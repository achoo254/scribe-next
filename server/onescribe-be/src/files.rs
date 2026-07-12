//! Files: lịch sử ghi (list) + phát lại (stream + HTTP Range cho player tua). Ownership mọi route.

use crate::auth::UserId;
use crate::error::AppError;
use crate::models::file;
use crate::state::AppState;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::Response;
use axum::{Extension, Json};
use serde_json::{json, Value};
use std::io::SeekFrom;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

/// `GET /api/files` — lịch sử ghi của user (recording), mới nhất trước.
pub async fn list(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
) -> Result<Json<Value>, AppError> {
    let recs = file::list_recordings(&state.db, &user.0).await?;
    let items: Vec<Value> = recs
        .iter()
        .map(|f| {
            json!({
                "id": f.id.map(|o| o.to_hex()),
                "name": f.name,
                "kind": f.kind,
                "size": f.size,
                "durationS": f.duration_s,
                "hasVideo": f.has_video.unwrap_or(false),
                "createdAt": f.created_at.try_to_rfc3339_string().ok(),
            })
        })
        .collect();
    Ok(Json(json!({ "items": items })))
}

/// `GET /api/files/:id` — stream file cho player; hỗ trợ HTTP Range (tua). Ownership bắt buộc.
pub async fn stream(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let f = file::get_owned(&state.db, &id, &user.0)
        .await?
        .ok_or_else(|| AppError::NotFound("file không tồn tại".into()))?;
    let meta = tokio::fs::metadata(&f.path)
        .await
        .map_err(|_| AppError::NotFound("file trên đĩa không tồn tại".into()))?;
    let total = meta.len();
    let ctype = content_type_for(&f.path);

    let file_handle = tokio::fs::File::open(&f.path)
        .await
        .map_err(|e| AppError::Internal(format!("mở file: {e}")))?;

    // Range: bytes=start-end (player tua). Chỉ hỗ trợ single-range; không hợp lệ → trả full 200.
    let range = headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| parse_range(s, total));

    if let Some((start, end)) = range {
        let len = end - start + 1;
        let mut fh = file_handle;
        fh.seek(SeekFrom::Start(start))
            .await
            .map_err(|e| AppError::Internal(format!("seek: {e}")))?;
        let body = Body::from_stream(ReaderStream::new(fh.take(len)));
        return Response::builder()
            .status(StatusCode::PARTIAL_CONTENT)
            .header(header::CONTENT_TYPE, ctype)
            .header(header::ACCEPT_RANGES, "bytes")
            .header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{total}"))
            .header(header::CONTENT_LENGTH, len)
            .body(body)
            .map_err(|e| AppError::Internal(format!("build response: {e}")));
    }

    let body = Body::from_stream(ReaderStream::new(file_handle));
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, ctype)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, total)
        .body(body)
        .map_err(|e| AppError::Internal(format!("build response: {e}")))
}

/// Parse `Range: bytes=start-end` → (start,end) inclusive, clamp theo total. None nếu không hợp lệ.
fn parse_range(raw: &str, total: u64) -> Option<(u64, u64)> {
    if total == 0 {
        return None;
    }
    let spec = raw.strip_prefix("bytes=")?;
    // Chỉ nhận single-range (không có dấu ',').
    if spec.contains(',') {
        return None;
    }
    let (s, e) = spec.split_once('-')?;
    let last = total - 1;
    let (start, end) = match (s.trim(), e.trim()) {
        ("", "") => return None,
        // bytes=-N → N byte cuối.
        ("", suffix) => {
            let n: u64 = suffix.parse().ok()?;
            if n == 0 {
                return None;
            }
            (total.saturating_sub(n), last)
        }
        // bytes=start- → tới hết.
        (start, "") => (start.parse().ok()?, last),
        // bytes=start-end
        (start, end) => (start.parse().ok()?, end.parse::<u64>().ok()?.min(last)),
    };
    if start > end || start > last {
        return None;
    }
    Some((start, end))
}

/// Content-Type theo đuôi file (recording = webm; các loại media khác cho playback/tải).
fn content_type_for(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "webm" => "video/webm",
        "mp4" | "m4v" => "video/mp4",
        "mkv" => "video/x-matroska",
        "mov" => "video/quicktime",
        "m4a" => "audio/mp4",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" | "oga" => "audio/ogg",
        "flac" => "audio/flac",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::parse_range;

    #[test]
    fn range_parsing() {
        assert_eq!(parse_range("bytes=0-99", 1000), Some((0, 99)));
        assert_eq!(parse_range("bytes=100-", 1000), Some((100, 999)));
        assert_eq!(parse_range("bytes=-50", 1000), Some((950, 999)));
        assert_eq!(parse_range("bytes=500-99999", 1000), Some((500, 999))); // clamp end
        assert_eq!(parse_range("bytes=0-0", 1000), Some((0, 0)));
        assert_eq!(parse_range("bytes=-", 1000), None);
        assert_eq!(parse_range("bytes=999-500", 1000), None); // start > end
        assert_eq!(parse_range("bytes=0-99,200-299", 1000), None); // multi-range
        assert_eq!(parse_range("bytes=1000-", 1000), None); // start > last
        assert_eq!(parse_range("bytes=0-99", 0), None); // empty file
    }
}
