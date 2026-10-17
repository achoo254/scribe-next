//! Upload chunked — append thẳng disk (KHÔNG buffer RAM cả file). sessionId server-gen + bind user.
//!
//! Red-team: #8 sessionId UUID server-sinh + verify userId + validate UUID (chống IDOR/traversal);
//! #7 resumable stateless (độ dài `.part` trên disk = nguồn sự thật); #14 size cap; #3 quota tăng dần. Cỡ tệp
//! tối đa và quota theo gói của user (`plan::Plan::storage`).

use crate::audit;
use crate::auth::UserId;
use crate::error::AppError;
use crate::models::file::{self, FileDoc, Owner};
use crate::models::user;
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
const MAX_NAME_CHARS: usize = 200;

/// Nguồn của lượt tải lên. Không gửi = Cắt file (tệp không vào "Lịch sử ghi"); giá trị lạ bị từ chối.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Purpose {
    /// "Tải tệp lên" ở trang Ghi âm: tệp vào "Lịch sử ghi" và chép lời được.
    Transcribe,
}

#[derive(Deserialize)]
pub struct InitReq {
    name: String,
    size: i64,
    #[serde(default)]
    purpose: Option<Purpose>,
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
    let storage = user::effective_plan(&state.db, &user.0).await?.storage();
    if size > storage.max_file_bytes {
        return Err(AppError::TooLarge(storage.file_too_large_message()));
    }
    let usage = file::user_usage_bytes(&state.db, &user.0).await?;
    let inflight = user_inflight_bytes(&state, &user.0).await;
    if usage + inflight + size > storage.quota_bytes {
        return Err(AppError::TooLarge(storage.quota_message()));
    }
    let name = display_name(&req.name);
    let session_id = Uuid::new_v4().to_string();
    tokio::fs::create_dir_all(tmp_dir(&state, &user.0))
        .await
        .map_err(|e| AppError::Internal(format!("tạo tmp dir: {e}")))?;
    let mut session = doc! {
        "_id": &session_id,
        "userId": &user.0,
        "name": &name,
        "expectedSize": req.size,
        "createdAt": DateTime::now(),
    };
    let mut details = doc! { "session": &session_id, "name": &name, "size": req.size };
    if req.purpose == Some(Purpose::Transcribe) {
        session.insert("purpose", file::PURPOSE_TRANSCRIBE);
        details.insert("purpose", file::PURPOSE_TRANSCRIBE);
    }
    state
        .db
        .collection::<Document>(SESSIONS)
        .insert_one(session)
        .await
        .map_err(|e| AppError::Internal(format!("tạo session: {e}")))?;
    audit::record(&state, Some(&user.0), "upload.start", details);
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
        .truncate(false)
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
    let storage = user::effective_plan(&state.db, &user.0).await?.storage();
    if written > storage.max_file_bytes {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(AppError::TooLarge(storage.file_too_large_message()));
    }
    // Quota = committed + TẤT CẢ .part đang dở (gồm .part vừa ghi) → chặn lấp disk qua nhiều session bỏ dở.
    let usage = file::user_usage_bytes(&state.db, &user.0).await?;
    let inflight = user_inflight_bytes(&state, &user.0).await;
    if usage + inflight > storage.quota_bytes {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(AppError::TooLarge(storage.quota_message()));
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
    if !crate::recordings::finalizable_size(size) {
        return Err(AppError::BadRequest("kích thước file không hợp lệ".into()));
    }
    let plan = user::effective_plan(&state.db, &user.0).await?;
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

    let info = core_split::probe_file(&state.config.ffprobe_path, &final_path).await.ok();
    let duration_s = info.as_ref().map_or(0.0, |i| i.duration_s);
    let has_video = info.as_ref().is_some_and(|i| !i.vcodec.is_empty());

    let name = session.get_str("name").unwrap_or("upload").to_string();
    let owner = Owner { user_id: &user.0, plan };
    let mut upload = FileDoc::new_upload(owner, &name, &final_path, size as i64, duration_s, has_video, id);
    if session.get_str("purpose") == Ok(file::PURPOSE_TRANSCRIBE) {
        upload = upload.for_transcription();
    }
    let file_id = file::create(&state.db, upload).await?;

    let _ = state
        .db
        .collection::<Document>(SESSIONS)
        .delete_one(doc! { "_id": &req.session })
        .await;

    let details = doc! {
        "fileId": &file_id,
        "name": &name,
        "size": size as i64,
        "durationS": duration_s,
    };
    audit::record(&state, Some(&user.0), "upload.finish", details);
    Ok(Json(FinalizeResp { file_id, name, duration_s }))
}

/// Tên tệp do client gửi, hiện ở "Lịch sử ghi": bỏ khoảng trắng hai đầu, tối đa [`MAX_NAME_CHARS`] ký tự.
fn display_name(raw: &str) -> String {
    let name: String = raw.trim().chars().take(MAX_NAME_CHARS).collect();
    if name.is_empty() { "Tệp tải lên".into() } else { name }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_accepts_only_known_purposes() {
        let parse = |body: &str| serde_json::from_str::<InitReq>(body).map(|r| r.purpose);
        assert_eq!(parse(r#"{"name":"a","size":1}"#).expect("Cắt file"), None);
        assert_eq!(parse(r#"{"name":"a","size":1,"purpose":"transcribe"}"#).expect("Ghi âm"), Some(Purpose::Transcribe));
        assert!(parse(r#"{"name":"a","size":1,"purpose":"history"}"#).is_err());
    }

    #[test]
    fn display_names_are_trimmed_and_capped() {
        assert_eq!(display_name("  họp.mp3 "), "họp.mp3");
        assert_eq!(display_name("   "), "Tệp tải lên");
        assert_eq!(display_name(&"á".repeat(300)).chars().count(), MAX_NAME_CHARS);
    }
}
