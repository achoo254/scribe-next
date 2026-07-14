//! Files: lịch sử ghi (list) + phát lại (stream + HTTP Range cho player tua). Ownership mọi route.

use crate::auth::UserId;
use crate::config::{QUOTA_BYTES, RETENTION_DAYS};
use crate::error::AppError;
use crate::models::file;
use crate::state::AppState;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;
use axum::{Extension, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::io::SeekFrom;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;
use uuid::Uuid;

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
    // retentionDays: FE tính hạn tự-xoá từng file = createdAt + retentionDays (khớp chính sách sweep).
    Ok(Json(json!({ "items": items, "retentionDays": RETENTION_DAYS })))
}

/// `GET /api/files/manage` — file KHÔNG phải bản ghi (upload Cắt file + đoạn output) để UI quản lý
/// dung lượng. Xoá dùng chung `DELETE /api/files/:id`. Ownership qua list_non_recordings.
pub async fn manage_list(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
) -> Result<Json<Value>, AppError> {
    let files = file::list_non_recordings(&state.db, &user.0).await?;
    let items: Vec<Value> = files
        .iter()
        .map(|f| {
            json!({
                "id": f.id.map(|o| o.to_hex()),
                "name": f.name,
                "kind": f.kind,
                "size": f.size,
                "jobId": f.job_id,
                "createdAt": f.created_at.try_to_rfc3339_string().ok(),
            })
        })
        .collect();
    Ok(Json(json!({ "items": items, "retentionDays": RETENTION_DAYS })))
}

/// `GET /api/me/usage` — dung lượng đã dùng + quota của user (FE QuotaBar).
pub async fn usage(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
) -> Result<Json<Value>, AppError> {
    let used = file::user_usage_bytes(&state.db, &user.0).await?;
    Ok(Json(json!({ "usageBytes": used, "quotaBytes": QUOTA_BYTES })))
}

/// `DELETE /api/files/:id` — xoá file của user (recording). Ownership bắt buộc; xoá disk TRƯỚC → doc
/// (giống retention sweep — lỗi disk non-NotFound → 500, giữ doc để không mồ côi).
pub async fn delete(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let f = file::get_owned(&state.db, &id, &user.0)
        .await?
        .ok_or_else(|| AppError::NotFound("file không tồn tại".into()))?;
    match tokio::fs::remove_file(&f.path).await {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(AppError::Internal(format!("xoá file disk: {e}"))),
    }
    if let Some(oid) = f.id {
        file::delete_doc(&state.db, oid).await?;
    }
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct StreamQuery {
    /// Có mặt (bất kỳ giá trị) → tải về (Content-Disposition attachment) thay vì phát inline.
    download: Option<String>,
}

/// `GET /api/files/:id[?download=1]` — stream file cho player (hỗ trợ HTTP Range để tua) hoặc tải
/// về kèm tên file khi có `download`. Ownership bắt buộc.
pub async fn stream(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path(id): Path<String>,
    Query(q): Query<StreamQuery>,
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
    let want_download = q.download.is_some();

    let file_handle = tokio::fs::File::open(&f.path)
        .await
        .map_err(|e| AppError::Internal(format!("mở file: {e}")))?;

    // Tải về: bỏ qua Range → trả full 200 kèm Content-Disposition (tên file có dấu tiếng Việt).
    // Phát inline: Range bytes=start-end (player tua); không hợp lệ → full 200.
    let range = if want_download {
        None
    } else {
        headers
            .get(header::RANGE)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| parse_range(s, total))
    };

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
    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, ctype)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, total);
    if want_download {
        builder = builder.header(
            header::CONTENT_DISPOSITION,
            content_disposition_attachment(&file_download_name(&f.name, &f.path)),
        );
    }
    builder
        .body(body)
        .map_err(|e| AppError::Internal(format!("build response: {e}")))
}

#[derive(Deserialize)]
pub struct ArchiveQuery {
    /// Lọc theo createdAt (ms epoch): from ≤ t < to. Bỏ trống cả hai → tải TẤT CẢ bản ghi.
    from: Option<i64>,
    to: Option<i64>,
}

/// `GET /api/files/archive[?from=&to=]` — zip toàn bộ bản ghi của user (hoặc trong khoảng thời gian
/// cho "tải theo ngày"). Ownership qua list_recordings. Zip dựng ở tmp rồi unlink ngay sau khi mở →
/// tự dọn khi stream xong (không để rác). from/to (ms) do FE tính theo ngày cục bộ → đúng múi giờ user.
pub async fn archive(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Query(q): Query<ArchiveQuery>,
) -> Result<Response, AppError> {
    let recs = file::list_recordings(&state.db, &user.0).await?;
    let selected: Vec<_> = recs
        .into_iter()
        .filter(|f| {
            let t = f.created_at.timestamp_millis();
            q.from.map_or(true, |from| t >= from) && q.to.map_or(true, |to| t < to)
        })
        .collect();
    if selected.is_empty() {
        return Err(AppError::NotFound("không có bản ghi để tải".into()));
    }

    // Tên trong zip: "<tên hiển thị>.<ext>", dedupe khi trùng (bản ghi có thể trùng tên).
    let mut used: HashSet<String> = HashSet::new();
    let entries: Vec<(String, String)> = selected
        .iter()
        .map(|f| {
            let ext = f.path.rsplit('.').next().unwrap_or("bin");
            let base = sanitize_filename(&f.name);
            let mut name = format!("{base}.{ext}");
            let mut i = 1;
            while !used.insert(name.clone()) {
                name = format!("{base} ({i}).{ext}");
                i += 1;
            }
            (name, f.path.clone())
        })
        .collect();

    let tmp_dir = format!("{}/{}/tmp", state.config.data_dir, user.0);
    tokio::fs::create_dir_all(&tmp_dir)
        .await
        .map_err(|e| AppError::Internal(format!("tạo tmp dir: {e}")))?;
    let zip_path = format!("{tmp_dir}/archive-{}.zip", Uuid::new_v4());
    let zp = zip_path.clone();
    tokio::task::spawn_blocking(move || build_zip(&zp, &entries))
        .await
        .map_err(|e| AppError::Internal(format!("zip task: {e}")))?
        .map_err(|e| AppError::Internal(format!("tạo zip: {e}")))?;

    let fh = tokio::fs::File::open(&zip_path)
        .await
        .map_err(|e| AppError::Internal(format!("mở zip: {e}")))?;
    // Unlink sau khi mở: handle vẫn đọc được, disk tự giải phóng khi stream xong → không rác tmp.
    let _ = tokio::fs::remove_file(&zip_path).await;
    let body = Body::from_stream(ReaderStream::new(fh));
    Response::builder()
        .header(header::CONTENT_TYPE, "application/zip")
        .header(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_static("attachment; filename=\"onescribe-recordings.zip\""),
        )
        .body(body)
        .map_err(|e| AppError::Internal(format!("build response: {e}")))
}

/// Dựng zip từ [(tên-trong-zip, path-đĩa)] — deflate thuần Rust (musl sạch). Chạy trong spawn_blocking.
fn build_zip(zip_path: &str, files: &[(String, String)]) -> std::io::Result<()> {
    use zip::write::{SimpleFileOptions, ZipWriter};
    let out = std::fs::File::create(zip_path)?;
    let mut zw = ZipWriter::new(out);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, path) in files {
        zw.start_file(name.as_str(), opts)?;
        let mut src = std::fs::File::open(path)?;
        std::io::copy(&mut src, &mut zw)?;
    }
    zw.finish()?;
    Ok(())
}

/// Tên file tải về = "<tên hiển thị an toàn>.<ext lấy từ path>".
fn file_download_name(name: &str, path: &str) -> String {
    let ext = path.rsplit('.').next().unwrap_or("bin");
    format!("{}.{ext}", sanitize_filename(name))
}

/// Bỏ ký tự không hợp lệ cho tên file (đặc biệt '/' tạo thư mục trong zip). Giữ dấu tiếng Việt.
fn sanitize_filename(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c => c,
        })
        .collect();
    let s = s.trim();
    if s.is_empty() { "ban-ghi".into() } else { s.into() }
}

/// Content-Disposition attachment với fallback ASCII + `filename*` UTF-8 (RFC 5987) cho tên có dấu.
fn content_disposition_attachment(filename: &str) -> HeaderValue {
    let ascii: String = filename
        .chars()
        .map(|c| if c.is_ascii() && c != '"' && c != '\\' { c } else { '_' })
        .collect();
    let value = format!("attachment; filename=\"{ascii}\"; filename*=UTF-8''{}", pct_encode(filename));
    HeaderValue::from_str(&value).unwrap_or_else(|_| HeaderValue::from_static("attachment"))
}

/// Percent-encode cho RFC 5987 (giữ nguyên unreserved; còn lại → %XX theo byte UTF-8).
fn pct_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push('%');
            out.push_str(&format!("{b:02X}"));
        }
    }
    out
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
