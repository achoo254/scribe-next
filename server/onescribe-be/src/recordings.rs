//! Recording streaming (Mode A) — append thẳng disk khi ghi (timeslice 1s), open-ended (KHÔNG biết
//! tổng size trước). Mirror có chủ đích cơ chế session/append/quota của `upload.rs` NHƯNG tách riêng
//! để KHÔNG đụng live path Cắt file (Phase 3 đang chờ QA). Dùng chung tmp dir → quota tính cả 2 loại.
//!
//! Khác upload: init KHÔNG cần size (stream mở); finalize → `files{kind:recording}` + probe duration.
//! Red-team: #8 sessionId UUID server-sinh + verify userId (chống IDOR/traversal); #7 append-only
//! stateless (độ dài `.part` = nguồn sự thật, không cần totalChunks); #3 quota tăng dần; #14 size cap.

use crate::auth::UserId;
use crate::config::{MAX_FILE_SIZE_BYTES, QUOTA_BYTES};
use crate::error::AppError;
use crate::models::file::{self, FileDoc};
use crate::state::AppState;
use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::{Extension, Json};
use mongodb::bson::{doc, oid::ObjectId, DateTime, Document};
use serde::{Deserialize, Serialize};
use std::io::SeekFrom;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use uuid::Uuid;

const SESSIONS: &str = "recording_sessions";

#[derive(Deserialize)]
pub struct InitReq {
    /// Tên hiển thị do FE đặt (vd "Ghi âm 2026-07-12 11:50").
    name: String,
}
#[derive(Serialize)]
pub struct InitResp {
    #[serde(rename = "sessionId")]
    session_id: String,
}

/// `POST /api/recordings/init` — stream mở (không có size trước); chỉ chặn nếu ĐÃ vượt quota.
pub async fn init(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Json(req): Json<InitReq>,
) -> Result<Json<InitResp>, AppError> {
    let usage = file::user_usage_bytes(&state.db, &user.0).await?;
    let inflight = user_inflight_bytes(&state, &user.0).await;
    if usage + inflight >= QUOTA_BYTES {
        return Err(AppError::TooLarge("đã đầy quota 5GB — xoá bớt để ghi tiếp".into()));
    }
    let session_id = Uuid::new_v4().to_string();
    tokio::fs::create_dir_all(tmp_dir(&state, &user.0))
        .await
        .map_err(|e| AppError::Internal(format!("tạo tmp dir: {e}")))?;
    state
        .db
        .collection::<Document>(SESSIONS)
        .insert_one(doc! {
            "_id": &session_id,
            "userId": &user.0,
            "name": &req.name,
            "createdAt": DateTime::now(),
        })
        .await
        .map_err(|e| AppError::Internal(format!("tạo session ghi: {e}")))?;
    Ok(Json(InitResp { session_id }))
}

#[derive(Deserialize)]
pub struct ChunkQuery {
    session: String,
    offset: u64,
}
#[derive(Serialize)]
pub struct WrittenResp {
    written: u64,
}

/// `POST /api/recordings/chunk?session=&offset=` body = raw bytes MediaRecorder → append tại offset.
pub async fn chunk(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Query(q): Query<ChunkQuery>,
    body: Bytes,
) -> Result<Json<WrittenResp>, AppError> {
    load_session(&state, &q.session, &user.0).await?;
    let part = part_path(&state, &user.0, &q.session)?;

    // Chỉ cho append liền mạch (offset ≤ độ dài hiện tại) → không tạo lỗ hổng sparse-file.
    let current_len = tokio::fs::metadata(&part).await.map(|m| m.len()).unwrap_or(0);
    if q.offset > current_len {
        return Err(AppError::BadRequest("offset không liền mạch".into()));
    }

    let mut f = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(&part)
        .await
        .map_err(|e| AppError::Internal(format!("mở .part: {e}")))?;
    f.seek(SeekFrom::Start(q.offset))
        .await
        .map_err(|e| AppError::Internal(format!("seek: {e}")))?;
    f.write_all(&body)
        .await
        .map_err(|e| AppError::Internal(format!("ghi chunk: {e}")))?;
    f.flush().await.ok();

    let written = tokio::fs::metadata(&part).await.map(|m| m.len()).unwrap_or(0);
    if written > MAX_FILE_SIZE_BYTES {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(AppError::TooLarge("bản ghi vượt giới hạn 2GB".into()));
    }
    let usage = file::user_usage_bytes(&state.db, &user.0).await?;
    let inflight = user_inflight_bytes(&state, &user.0).await;
    if usage + inflight > QUOTA_BYTES {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(AppError::TooLarge("vượt quota 5GB (gồm cả bản ghi đang dở)".into()));
    }
    Ok(Json(WrittenResp { written }))
}

#[derive(Deserialize)]
pub struct FinalizeReq {
    session: String,
    /// Thời lượng client đo bằng timer (webm MediaRecorder thường thiếu duration trong container →
    /// probe ra 0). Dùng làm fallback khi probe không có duration.
    #[serde(rename = "durationS", default)]
    duration_s: Option<f64>,
}
#[derive(Serialize)]
pub struct FinalizeResp {
    #[serde(rename = "fileId")]
    file_id: String,
    name: String,
    #[serde(rename = "durationS")]
    duration_s: f64,
}

/// `POST /api/recordings/finalize` — validate size + magic-byte → rename vào recordings → file doc + probe.
pub async fn finalize(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Json(req): Json<FinalizeReq>,
) -> Result<Json<FinalizeResp>, AppError> {
    let session = load_session(&state, &req.session, &user.0).await?;
    let part = part_path(&state, &user.0, &req.session)?;
    let size = tokio::fs::metadata(&part)
        .await
        .map_err(|_| AppError::BadRequest("chưa có dữ liệu ghi".into()))?
        .len();
    if size == 0 || size > MAX_FILE_SIZE_BYTES {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(AppError::BadRequest("kích thước bản ghi không hợp lệ".into()));
    }
    // Magic-byte: MediaRecorder xuất webm (matroska) → infer nhận diện Video. Chỉ nhận audio/video.
    let detected = infer::get_from_path(&part)
        .map_err(|e| AppError::Internal(format!("đọc header: {e}")))?
        .ok_or_else(|| AppError::BadRequest("không nhận dạng được định dạng bản ghi".into()))?;
    let mt = detected.matcher_type();
    if mt != infer::MatcherType::Audio && mt != infer::MatcherType::Video {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(AppError::BadRequest("bản ghi không phải audio/video hợp lệ".into()));
    }

    let rec_dir = format!("{}/{}/recordings", state.config.data_dir, user.0);
    tokio::fs::create_dir_all(&rec_dir)
        .await
        .map_err(|e| AppError::Internal(format!("tạo recordings dir: {e}")))?;
    let id = ObjectId::new();
    let final_path = format!("{}/{}.{}", rec_dir, id.to_hex(), detected.extension());
    tokio::fs::rename(&part, &final_path)
        .await
        .map_err(|e| AppError::Internal(format!("rename recording: {e}")))?;

    let info = core_split::probe_file(&state.config.ffprobe_path, &final_path)
        .await
        .ok();
    let probed = info.as_ref().map(|i| i.duration_s).unwrap_or(0.0);
    // Probe webm MediaRecorder thường = 0 (thiếu duration metadata) → fallback timer client.
    let duration_s = if probed > 0.0 { probed } else { req.duration_s.unwrap_or(0.0).max(0.0) };
    let has_video = info.as_ref().map(|i| !i.vcodec.is_empty()).unwrap_or(false);

    let name = session.get_str("name").unwrap_or("Bản ghi").to_string();
    let file_id = file::create(
        &state.db,
        FileDoc::new_recording(&user.0, &name, &final_path, size as i64, duration_s, has_video, id),
    )
    .await?;

    let _ = state
        .db
        .collection::<Document>(SESSIONS)
        .delete_one(doc! { "_id": &req.session })
        .await;

    Ok(Json(FinalizeResp { file_id, name, duration_s }))
}

fn tmp_dir(state: &AppState, user_id: &str) -> String {
    format!("{}/{}/tmp", state.config.data_dir, user_id)
}

/// Tổng byte các `.part` đang dở (chung tmp với upload → quota tính cả upload lẫn recording dở).
async fn user_inflight_bytes(state: &AppState, user_id: &str) -> u64 {
    let mut total = 0u64;
    if let Ok(mut rd) = tokio::fs::read_dir(tmp_dir(state, user_id)).await {
        while let Ok(Some(entry)) = rd.next_entry().await {
            if entry.file_name().to_string_lossy().ends_with(".part") {
                if let Ok(m) = entry.metadata().await {
                    total += m.len();
                }
            }
        }
    }
    total
}

/// Tra session rồi verify ownership (red-team #8: KHÔNG tin sessionId client bịa).
async fn load_session(
    state: &AppState,
    session_id: &str,
    user_id: &str,
) -> Result<Document, AppError> {
    Uuid::parse_str(session_id).map_err(|_| AppError::BadRequest("session id không hợp lệ".into()))?;
    let s = state
        .db
        .collection::<Document>(SESSIONS)
        .find_one(doc! { "_id": session_id })
        .await
        .map_err(|e| AppError::Internal(format!("tra session: {e}")))?
        .ok_or_else(|| AppError::NotFound("session ghi không tồn tại".into()))?;
    if s.get_str("userId").unwrap_or("") != user_id {
        return Err(AppError::Forbidden("session không thuộc bạn".into()));
    }
    Ok(s)
}

/// Path `.part` — session PHẢI là UUID hợp lệ (chống path-traversal).
fn part_path(state: &AppState, user_id: &str, session_id: &str) -> Result<String, AppError> {
    Uuid::parse_str(session_id).map_err(|_| AppError::BadRequest("session id không hợp lệ".into()))?;
    Ok(format!("{}/{}.part", tmp_dir(state, user_id), session_id))
}
