//! Recording streaming (Mode A) — append thẳng disk khi ghi (timeslice 1s), open-ended (KHÔNG biết
//! tổng size trước). Mirror có chủ đích cơ chế session/append/quota của `upload.rs` NHƯNG tách riêng
//! để KHÔNG đụng live path Cắt file (Phase 3 đang chờ QA). Dùng chung tmp dir → quota tính cả 2 loại.
//!
//! Khác upload: init KHÔNG cần size (stream mở); finalize → `files{kind:recording}` + probe duration.
//! Red-team: #8 sessionId UUID server-sinh + verify userId (chống IDOR/traversal); #7 append-only
//! stateless (độ dài `.part` = nguồn sự thật, không cần totalChunks); #3 quota tăng dần; #14 size cap. Quota và
//! cỡ tệp tối đa theo gói của user (`plan::Plan::storage`). Chunk làm vượt giới hạn bị cắt bỏ (413) nhưng
//! phần đã nhận giữ nguyên: bản ghi vẫn finalize được tới lúc đầy.

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
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
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
    /// Id file của bản ghi khi finalize, giữ trước từ lúc bắt đầu: client biết khoá transcript
    /// `f:<fileId>` ngay khi đang ghi (để đánh dấu mốc).
    #[serde(rename = "fileId")]
    file_id: String,
}

/// `POST /api/recordings/init` — stream mở (không có size trước); chỉ chặn nếu ĐÃ vượt quota.
pub async fn init(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Json(req): Json<InitReq>,
) -> Result<Json<InitResp>, AppError> {
    let storage = user::effective_plan(&state.db, &user.0).await?.storage();
    let usage = file::user_usage_bytes(&state.db, &user.0).await?;
    let inflight = user_inflight_bytes(&state, &user.0).await;
    if usage + inflight >= storage.quota_bytes {
        return Err(AppError::TooLarge(storage.quota_message()));
    }
    let session_id = Uuid::new_v4().to_string();
    let file_id = ObjectId::new();
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
            "fileId": file_id,
            "createdAt": DateTime::now(),
        })
        .await
        .map_err(|e| AppError::Internal(format!("tạo session ghi: {e}")))?;
    let details = doc! { "session": &session_id, "name": &req.name };
    audit::record(&state, Some(&user.0), "recording.start", details);
    Ok(Json(InitResp { session_id, file_id: file_id.to_hex() }))
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
    let storage = user::effective_plan(&state.db, &user.0).await?.storage();
    let over = if written > storage.max_file_bytes {
        Some(storage.file_too_large_message())
    } else {
        let usage = file::user_usage_bytes(&state.db, &user.0).await?;
        let inflight = user_inflight_bytes(&state, &user.0).await;
        (usage + inflight > storage.quota_bytes).then(|| storage.quota_message())
    };
    if let Some(message) = over {
        // Bỏ đúng chunk vừa ghi, giữ phần client đã được xác nhận: bản ghi finalize được tới lúc đầy.
        if let Err(e) = f.set_len(current_len).await {
            tracing::warn!("cắt chunk vượt giới hạn lỗi ({e}) — bỏ cả .part");
            let _ = tokio::fs::remove_file(&part).await;
        }
        return Err(AppError::TooLarge(message));
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
    if !finalizable_size(size) {
        return Err(AppError::BadRequest("kích thước bản ghi không hợp lệ".into()));
    }
    let plan = user::effective_plan(&state.db, &user.0).await?;
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
    // Phiên tạo trước khi có `fileId` giữ trước thì sinh id mới.
    let id = session.get_object_id("fileId").unwrap_or_else(|_| ObjectId::new());
    // Recording MediaRecorder = webm streaming: thiếu Cues (SeekHead) + Duration → player KHÔNG tua
    // được + probe duration 0. Remux `-c copy -f webm` ghi output là FILE seekable → ffmpeg dựng lại
    // Cues + Duration (không re-encode) → tua được + probe đúng. Chỉ EBML (webm/mkv) mới remux; định
    // dạng khác (mp4 Safari, hiếm) giữ nguyên. Remux lỗi (hết đĩa/input hỏng) → fallback rename để
    // KHÔNG mất bản ghi (chỉ mất khả năng tua).
    let is_ebml = matches!(detected.extension(), "webm" | "mkv");
    let final_path = if is_ebml {
        let webm_path = format!("{}/{}.webm", rec_dir, id.to_hex());
        match core_split::remux_copy_to_webm(&state.config.ffmpeg_path, &part, &webm_path).await {
            Ok(()) => {
                let _ = tokio::fs::remove_file(&part).await; // remux tạo bản mới → dọn .part gốc
                webm_path
            }
            Err(e) => {
                tracing::warn!("remux recording webm lỗi ({e}) — giữ nguyên (bản ghi không tua được)");
                let _ = tokio::fs::remove_file(&webm_path).await; // dọn bản remux dở
                // Fallback giữ container gốc (ext theo magic-byte) — KHÔNG ép .webm để tránh gán sai
                // Content-Type nếu input thực sự là mkv (chỉ qua upload thủ công; MediaRecorder = webm).
                let orig_path = format!("{}/{}.{}", rec_dir, id.to_hex(), detected.extension());
                tokio::fs::rename(&part, &orig_path)
                    .await
                    .map_err(|e| AppError::Internal(format!("rename recording: {e}")))?;
                orig_path
            }
        }
    } else {
        let fp = format!("{}/{}.{}", rec_dir, id.to_hex(), detected.extension());
        tokio::fs::rename(&part, &fp)
            .await
            .map_err(|e| AppError::Internal(format!("rename recording: {e}")))?;
        fp
    };

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
        FileDoc::new_recording(Owner { user_id: &user.0, plan }, &name, &final_path, size as i64, duration_s, has_video, id),
    )
    .await?;
    // Đoạn iPhone tới muộn (sau các đoạn đứng sau nó): mốc giây theo cả phiên của các đoạn sau nó dời theo,
    // nên dựng lại index của phiên. Bản ghi một file mới chưa có câu nào để dựng.
    let key = file::session_key("recording", &name, id);
    if key.starts_with("s:") {
        crate::search::index::schedule(&state.db, &user.0, &key);
    }

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
        "hasVideo": has_video,
    };
    audit::record(&state, Some(&user.0), "recording.finish", details);
    Ok(Json(FinalizeResp { file_id, name, duration_s }))
}

/// `.part` finalize được khi có dữ liệu. Cỡ tệp tối đa theo gói KHÔNG xét lại ở đây: chunk đã chặn từng byte
/// lúc nhận theo gói khi đó, còn gói có thể đã hạ giữa chừng (hết hạn plus khi đang ghi). Xét lại sẽ từ chối
/// cả phần đã nhận và người dùng mất bản ghi. Dùng chung cho tệp tải lên.
pub(crate) fn finalizable_size(size: u64) -> bool {
    size > 0
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

/// User có phiên ghi đang mở giữ trước id file này không (dấu mốc trước khi bản ghi được finalize).
pub(crate) async fn owns_pending_file(state: &AppState, user_id: &str, file_id: ObjectId) -> Result<bool, AppError> {
    state
        .db
        .collection::<Document>(SESSIONS)
        .find_one(doc! { "fileId": file_id, "userId": user_id })
        .projection(doc! { "_id": 1 })
        .await
        .map(|d| d.is_some())
        .map_err(|e| AppError::Internal(format!("tra session ghi: {e}")))
}

/// Khoá phiên chép lời (`s:<phiên iPhone>`, `f:<id giữ trước>`) của các phiên ghi đang mở của user: "Xoá tất
/// cả" chừa các file này (phiên iPhone còn đang ghi, các đoạn trước đã lên server).
pub(crate) async fn active_session_keys(state: &AppState, user_id: &str) -> Result<HashSet<String>, AppError> {
    let read = |e: mongodb::error::Error| AppError::Internal(format!("tra phiên ghi đang mở: {e}"));
    let mut cur = state
        .db
        .collection::<Document>(SESSIONS)
        .find(doc! { "userId": user_id })
        .projection(doc! { "name": 1, "fileId": 1 })
        .await
        .map_err(read)?;
    let mut keys = HashSet::new();
    while let Some(d) = cur.next().await {
        let d = d.map_err(read)?;
        if let Ok(id) = d.get_object_id("fileId") {
            keys.insert(format!("f:{}", id.to_hex()));
        }
        if let Some((session, _)) = d.get_str("name").ok().and_then(file::parse_segment_name) {
            keys.insert(format!("s:{session}"));
        }
    }
    Ok(keys)
}

/// Một đoạn tải lên bắt đầu quá chừng này (phút) mà chưa finalize thì coi như bỏ dở (app bị gỡ, mất mạng hẳn).
const UPLOAD_STALE_MINUTES: i64 = 10;

/// Phiên iPhone `session` còn đoạn đang tải lên (mở trong [`UPLOAD_STALE_MINUTES`] phút qua): chép cả phiên nên chờ
/// đoạn đó lên xong.
pub(crate) async fn segment_uploading(state: &AppState, user_id: &str, session: &str) -> Result<bool, AppError> {
    let read = |e: mongodb::error::Error| AppError::Internal(format!("tra đoạn đang tải lên: {e}"));
    let since = DateTime::from_millis(DateTime::now().timestamp_millis() - UPLOAD_STALE_MINUTES * 60_000);
    let mut cur = state
        .db
        .collection::<Document>(SESSIONS)
        .find(doc! { "userId": user_id, "createdAt": { "$gt": since } })
        .projection(doc! { "name": 1 })
        .await
        .map_err(read)?;
    while let Some(d) = cur.next().await {
        let d = d.map_err(read)?;
        if d.get_str("name").ok().and_then(file::parse_segment_name).is_some_and(|(s, _)| s == session) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Path `.part` — session PHẢI là UUID hợp lệ (chống path-traversal).
fn part_path(state: &AppState, user_id: &str, session_id: &str) -> Result<String, AppError> {
    Uuid::parse_str(session_id).map_err(|_| AppError::BadRequest("session id không hợp lệ".into()))?;
    Ok(format!("{}/{}.part", tmp_dir(state, user_id), session_id))
}

#[cfg(test)]
mod tests {
    use super::finalizable_size;
    use crate::models::plan::Plan;

    #[test]
    fn finalize_keeps_what_was_received_after_a_plan_downgrade() {
        // Ghi 600 MB khi còn plus, gói hạ về free (200 MB/tệp) giữa chừng: phần đã nhận vẫn finalize được.
        let received = 600 * 1024 * 1024;
        assert!(received > Plan::Free.storage().max_file_bytes);
        assert!(finalizable_size(received));
        assert!(finalizable_size(Plan::Plus.storage().max_file_bytes + 1));
        assert!(!finalizable_size(0));
    }
}
