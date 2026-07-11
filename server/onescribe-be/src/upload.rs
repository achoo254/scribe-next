//! Upload chunked — append thẳng disk (KHÔNG buffer RAM cả file). sessionId server-gen + bind user.
//!
//! Red-team: #8 sessionId UUID server-sinh + verify userId + validate UUID (chống IDOR/traversal);
//! #7 resumable stateless (độ dài `.part` trên disk = nguồn sự thật); #14 size cap 2GB; #3 quota tăng dần.

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

const SESSIONS: &str = "upload_sessions";

#[derive(Deserialize)]
pub struct InitReq {
    name: String,
    size: i64,
}
#[derive(Serialize)]
pub struct InitResp {
    #[serde(rename = "sessionId")]
    session_id: String,
}

/// `POST /api/upload/init` — reject sớm nếu vượt size cap / quota; sinh sessionId UUID.
pub async fn init(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Json(req): Json<InitReq>,
) -> Result<Json<InitResp>, AppError> {
    if req.size <= 0 {
        return Err(AppError::BadRequest("size không hợp lệ".into()));
    }
    let size = req.size as u64;
    if size > MAX_FILE_SIZE_BYTES {
        return Err(AppError::TooLarge("file vượt giới hạn 2GB".into()));
    }
    let usage = file::user_usage_bytes(&state.db, &user.0).await?;
    let inflight = user_inflight_bytes(&state, &user.0).await;
    if usage + inflight + size > QUOTA_BYTES {
        return Err(AppError::TooLarge("vượt quota 5GB (gồm cả upload đang dở)".into()));
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
            "expectedSize": req.size,
            "createdAt": DateTime::now(),
        })
        .await
        .map_err(|e| AppError::Internal(format!("tạo session: {e}")))?;
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

/// `POST /api/upload/chunk?session=&offset=` body = raw bytes → append tại offset (seek+write).
pub async fn chunk(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Query(q): Query<ChunkQuery>,
    body: Bytes,
) -> Result<Json<WrittenResp>, AppError> {
    load_session(&state, &q.session, &user.0).await?;
    let part = part_path(&state, &user.0, &q.session)?;

    // Chống gap/sparse-file: chỉ cho append liền mạch (offset ≤ độ dài hiện tại) → không tạo lỗ hổng.
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
    // Size cap + quota tăng dần — abort sớm, dọn .part để không ăn disk (box shared).
    if written > MAX_FILE_SIZE_BYTES {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(AppError::TooLarge("file vượt giới hạn 2GB".into()));
    }
    // Quota = committed + TẤT CẢ .part đang dở (gồm .part vừa ghi) → chặn lấp disk qua nhiều session bỏ dở.
    let usage = file::user_usage_bytes(&state.db, &user.0).await?;
    let inflight = user_inflight_bytes(&state, &user.0).await;
    if usage + inflight > QUOTA_BYTES {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(AppError::TooLarge("vượt quota 5GB (gồm cả upload đang dở)".into()));
    }
    Ok(Json(WrittenResp { written }))
}

#[derive(Deserialize)]
pub struct StatusQuery {
    session: String,
}

/// `GET /api/upload/status?session=` → byte `.part` hiện tại (resume từ offset này).
pub async fn status(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Query(q): Query<StatusQuery>,
) -> Result<Json<WrittenResp>, AppError> {
    load_session(&state, &q.session, &user.0).await?;
    let part = part_path(&state, &user.0, &q.session)?;
    let written = tokio::fs::metadata(&part).await.map(|m| m.len()).unwrap_or(0);
    Ok(Json(WrittenResp { written }))
}

#[derive(Deserialize)]
pub struct FinalizeReq {
    session: String,
}
#[derive(Serialize)]
pub struct FinalizeResp {
    #[serde(rename = "fileId")]
    file_id: String,
    name: String,
    #[serde(rename = "durationS")]
    duration_s: f64,
}

/// `POST /api/upload/finalize` — validate size + magic-byte → rename vào uploads → tạo file doc + probe.
pub async fn finalize(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Json(req): Json<FinalizeReq>,
) -> Result<Json<FinalizeResp>, AppError> {
    let session = load_session(&state, &req.session, &user.0).await?;
    let part = part_path(&state, &user.0, &req.session)?;
    let size = tokio::fs::metadata(&part)
        .await
        .map_err(|_| AppError::BadRequest("chưa có dữ liệu upload".into()))?
        .len();
    if size == 0 || size > MAX_FILE_SIZE_BYTES {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(AppError::BadRequest("kích thước file không hợp lệ".into()));
    }
    // Magic-byte: chỉ nhận audio/video (soát header — không chặn CVE demuxer, xem red-team #15).
    let detected = infer::get_from_path(&part)
        .map_err(|e| AppError::Internal(format!("đọc header: {e}")))?
        .ok_or_else(|| AppError::BadRequest("không nhận dạng được định dạng file".into()))?;
    let mt = detected.matcher_type();
    if mt != infer::MatcherType::Audio && mt != infer::MatcherType::Video {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(AppError::BadRequest("chỉ nhận file audio hoặc video".into()));
    }

    let uploads_dir = format!("{}/{}/uploads", state.config.data_dir, user.0);
    tokio::fs::create_dir_all(&uploads_dir)
        .await
        .map_err(|e| AppError::Internal(format!("tạo uploads dir: {e}")))?;
    let id = ObjectId::new();
    let final_path = format!("{}/{}.{}", uploads_dir, id.to_hex(), detected.extension());
    tokio::fs::rename(&part, &final_path)
        .await
        .map_err(|e| AppError::Internal(format!("rename upload: {e}")))?;

    let duration_s = core_split::probe_file(&state.config.ffprobe_path, &final_path)
        .await
        .map(|i| i.duration_s)
        .unwrap_or(0.0);

    let name = session.get_str("name").unwrap_or("upload").to_string();
    let file_id = file::create(
        &state.db,
        FileDoc::new_upload(&user.0, &name, &final_path, size as i64, id),
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

/// Tổng byte các `.part` đang dở của user — quota tính cả in-flight (red-team #3: chống lấp disk
/// bằng nhiều session upload bỏ dở, vì `.part` chưa finalize không nằm trong `files`).
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

/// Tra session theo _id rồi verify ownership (red-team #8: KHÔNG tin sessionId client bịa).
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
        .ok_or_else(|| AppError::NotFound("session không tồn tại".into()))?;
    if s.get_str("userId").unwrap_or("") != user_id {
        return Err(AppError::Forbidden("session không thuộc bạn".into()));
    }
    Ok(s)
}

/// Path `.part` — session PHẢI là UUID hợp lệ (validate trước khi dựng path → chống path-traversal).
fn part_path(state: &AppState, user_id: &str, session_id: &str) -> Result<String, AppError> {
    Uuid::parse_str(session_id).map_err(|_| AppError::BadRequest("session id không hợp lệ".into()))?;
    Ok(format!("{}/{}.part", tmp_dir(state, user_id), session_id))
}
