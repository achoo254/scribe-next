//! Files: lịch sử ghi (list) + phát lại (stream + HTTP Range cho player tua). Ownership mọi route.

use crate::audit;
use crate::auth::UserId;
use crate::error::AppError;
use crate::models::file::FileDoc;
use crate::models::{file, job, transcript, user};
use crate::recordings;
use crate::state::AppState;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;
use axum::{Extension, Json};
use serde::Deserialize;
use mongodb::bson::{doc, DateTime};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::io::SeekFrom;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;
use uuid::Uuid;

/// Mỗi phút một user được bấm "Xoá tất cả" tối đa chừng này lần.
const DELETE_ALL_PER_MINUTE: u32 = 3;

/// Phiên iPhone có đoạn mới lên trong khoảng này coi như còn đang ghi: giữa hai lượt tải đoạn (mỗi đoạn
/// khoảng 1 phút) máy chủ không có phiên tải nào mở, nên "Xoá tất cả" dựa vào đoạn mới nhất.
const LIVE_IPHONE_SESSION_MS: i64 = 10 * 60 * 1000;

/// `GET /api/files` — lịch sử ghi của user (bản ghi + tệp tải lên ở trang Ghi âm), mới nhất trước.
pub async fn list(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
) -> Result<Json<Value>, AppError> {
    let recs = file::list_history(&state.db, &user.0).await?;
    let retention_days = user::effective_plan(&state.db, &user.0).await?.storage().retention_days;
    let transcripts: HashMap<String, String> = transcript::statuses(&state.db, &user.0).await?.into_iter().collect();
    let items: Vec<Value> = recs
        .iter()
        .map(|f| {
            let id = f.id.map(|o| o.to_hex());
            let transcript = id.as_ref().and_then(|id| transcripts.get(id));
            json!({
                "id": id,
                "name": f.name,
                "kind": f.kind,
                "size": f.size,
                "durationS": f.duration_s,
                "transcript": transcript.map(|status| json!({ "status": status })),
                "hasVideo": f.has_video.unwrap_or(false),
                "createdAt": f.created_at.try_to_rfc3339_string().ok(),
                "expiresAt": f.expires_at.try_to_rfc3339_string().ok(),
            })
        })
        .collect();
    // `expiresAt` của từng file là mốc sweep xoá thật (theo gói lúc tạo); `retentionDays` theo gói hiện tại
    // chỉ cho câu giới thiệu.
    Ok(Json(json!({ "items": items, "retentionDays": retention_days })))
}

/// `GET /api/files/manage` — file của trang Cắt file (tệp tải lên ở đó + đoạn output) để UI quản lý
/// dung lượng. Xoá dùng chung `DELETE /api/files/:id`. Ownership qua list_split_files.
pub async fn manage_list(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
) -> Result<Json<Value>, AppError> {
    let files = file::list_split_files(&state.db, &user.0).await?;
    let retention_days = user::effective_plan(&state.db, &user.0).await?.storage().retention_days;
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
                "expiresAt": f.expires_at.try_to_rfc3339_string().ok(),
            })
        })
        .collect();
    Ok(Json(json!({ "items": items, "retentionDays": retention_days })))
}

/// `GET /api/me/usage` — dung lượng đã dùng và giới hạn lưu trữ theo gói của user.
pub async fn usage(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
) -> Result<Json<Value>, AppError> {
    let used = file::user_usage_bytes(&state.db, &user.0).await?;
    let plan = user::effective_plan(&state.db, &user.0).await?;
    let storage = plan.storage();
    Ok(Json(json!({
        "usageBytes": used,
        "quotaBytes": storage.quota_bytes,
        "maxFileBytes": storage.max_file_bytes,
        "retentionDays": storage.retention_days,
        "plan": plan.name(),
    })))
}

/// Chia lịch sử ghi thành file "Xoá tất cả" được xoá và file phải chừa: thuộc phiên ghi đang mở
/// (`active_keys`, khoá phiên) hoặc là đầu vào của job cắt đang chạy (`busy_inputs`, id hex).
fn split_for_delete_all<'a>(
    files: &'a [FileDoc],
    active_keys: &HashSet<String>,
    busy_inputs: &HashSet<String>,
) -> (Vec<&'a FileDoc>, Vec<&'a FileDoc>) {
    files.iter().partition(|f| {
        let busy = f.id.is_some_and(|id| busy_inputs.contains(&id.to_hex()));
        let active = f.session_key().is_some_and(|key| active_keys.contains(&key));
        !busy && !active
    })
}

/// Khoá `s:` của các phiên iPhone có đoạn tạo trong [`LIVE_IPHONE_SESSION_MS`] trước `now`.
fn live_iphone_sessions(files: &[FileDoc], now: DateTime) -> HashSet<String> {
    let since = now.timestamp_millis() - LIVE_IPHONE_SESSION_MS;
    files
        .iter()
        .filter(|f| f.kind == "recording" && f.created_at.timestamp_millis() > since)
        .filter_map(FileDoc::session_key)
        .filter(|key| key.starts_with("s:"))
        .collect()
}

/// Xoá đĩa rồi doc (như xoá tay). Lỗi đĩa khác NotFound → giữ doc (không mồ côi file), trả lỗi.
async fn remove_file_and_doc(db: &mongodb::Database, f: &FileDoc) -> Result<(), AppError> {
    match tokio::fs::remove_file(&f.path).await {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(AppError::Internal(format!("xoá file disk: {e}"))),
    }
    file::delete_doc(db, f).await
}

/// `DELETE /api/files` — "Xoá tất cả" trong Lịch sử ghi: đúng những file `GET /api/files` liệt kê (bản ghi và
/// tệp tải lên ở trang Ghi âm), kèm bản chép lời, biên bản, dấu mốc và câu trong index (qua `file::delete_doc`).
/// Chừa file của phiên ghi đang mở, của phiên iPhone có đoạn mới trong 10 phút qua (còn đang ghi dù lúc này
/// không có đoạn nào đang tải) và đầu vào job cắt đang chạy. Tệp tải lên và đoạn cắt của trang Cắt file
/// quản lý ở trang đó, không xoá ở đây.
pub async fn delete_all(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
) -> Result<Json<Value>, AppError> {
    if !state.rate.allow(&format!("delete_all:{}", user.0), DELETE_ALL_PER_MINUTE) {
        return Err(AppError::TooManyRequests("thao tác quá nhanh, thử lại sau một phút".into()));
    }
    let files = file::list_history(&state.db, &user.0).await?;
    let mut active_keys = recordings::active_session_keys(&state, &user.0).await?;
    active_keys.extend(live_iphone_sessions(&files, DateTime::now()));
    let busy_inputs: HashSet<String> = job::busy_inputs(&state.db, &user.0).await?.into_iter().collect();
    let (targets, kept) = split_for_delete_all(&files, &active_keys, &busy_inputs);
    let (mut deleted, mut failed, mut bytes) = (0_i64, 0_i64, 0_i64);
    for f in targets {
        match remove_file_and_doc(&state.db, f).await {
            Ok(()) => {
                deleted += 1;
                bytes += f.size.max(0);
            }
            Err(e) => {
                failed += 1;
                tracing::warn!(path = %f.path, "xoá tất cả: bỏ qua file lỗi: {e:?}");
            }
        }
    }
    let skipped = kept.len() as i64;
    let details = doc! { "deleted": deleted, "skipped": skipped, "failed": failed, "bytes": bytes };
    audit::record(&state, Some(&user.0), "file.delete_all", details);
    if deleted == 0 && failed > 0 {
        return Err(AppError::Internal("xoá tất cả: không xoá được file nào".into()));
    }
    Ok(Json(json!({ "deleted": deleted, "skipped": skipped, "failed": failed, "bytes": bytes })))
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
    remove_file_and_doc(&state.db, &f).await?;
    let details = doc! { "fileId": &id, "name": &f.name, "kind": &f.kind, "size": f.size };
    audit::record(&state, Some(&user.0), "file.delete", details);
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
    // Phát: chỉ ghi lần mở đầu (không Range hoặc từ byte 0), không ghi mỗi lần tua.
    let opening = headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .is_none_or(|r| r.trim().starts_with("bytes=0-"));
    if want_download || opening {
        let action = if want_download { "file.download" } else { "file.play" };
        let details = doc! { "fileId": &id, "name": &f.name, "kind": &f.kind };
        audit::record(&state, Some(&user.0), action, details);
    }

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
/// cho "tải theo ngày"). Ownership qua list_history. Zip dựng ở tmp rồi unlink ngay sau khi mở →
/// tự dọn khi stream xong (không để rác). from/to (ms) do FE tính theo ngày cục bộ → đúng múi giờ user.
pub async fn archive(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Query(q): Query<ArchiveQuery>,
) -> Result<Response, AppError> {
    let recs = file::list_history(&state.db, &user.0).await?;
    let selected: Vec<_> = recs
        .into_iter()
        .filter(|f| {
            let t = f.created_at.timestamp_millis();
            q.from.is_none_or(|from| t >= from) && q.to.is_none_or(|to| t < to)
        })
        .collect();
    if selected.is_empty() {
        return Err(AppError::NotFound("không có bản ghi để tải".into()));
    }
    let details = doc! { "count": selected.len() as i64, "from": q.from, "to": q.to };
    audit::record(&state, Some(&user.0), "recordings.archive", details);

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
            HeaderValue::from_static("attachment; filename=\"scribe-recordings.zip\""),
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
pub(crate) fn sanitize_filename(name: &str) -> String {
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
pub(crate) fn content_disposition_attachment(filename: &str) -> HeaderValue {
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
    use super::{live_iphone_sessions, parse_range, split_for_delete_all, LIVE_IPHONE_SESSION_MS};
    use crate::models::file::{FileDoc, Owner};
    use crate::models::plan::Plan;
    use mongodb::bson::{oid::ObjectId, DateTime};
    use std::collections::HashSet;

    #[test]
    fn delete_all_spares_open_recording_sessions_and_busy_split_inputs() {
        let owner = Owner { user_id: "u", plan: Plan::Free };
        let rec = |name: &str| FileDoc::new_recording(owner, name, "/r", 10, 1.0, false, ObjectId::new());
        let files = vec![
            rec("Họp sáng"),
            rec("aaaaaaaa-1111#0"),
            rec("aaaaaaaa-1111#1"),
            rec("bbbbbbbb-2222#0"),
            FileDoc::new_upload(owner, "aaaaaaaa-1111#2", "/u.mp3", 10, 1.0, false, ObjectId::new()).for_transcription(),
            rec("Đang cắt"),
        ];
        let id = |i: usize| files[i].id.expect("id").to_hex();
        let active: HashSet<String> = ["s:aaaaaaaa-1111".to_string()].into();
        let busy: HashSet<String> = [id(5)].into();
        let (targets, kept) = split_for_delete_all(&files, &active, &busy);
        let names = |v: Vec<&FileDoc>| v.into_iter().map(|f| f.name.clone()).collect::<Vec<_>>();
        // Tệp tải lên trùng tên đoạn iPhone vẫn là phiên riêng nên bị xoá.
        assert_eq!(names(targets), ["Họp sáng", "bbbbbbbb-2222#0", "aaaaaaaa-1111#2"]);
        assert_eq!(names(kept), ["aaaaaaaa-1111#0", "aaaaaaaa-1111#1", "Đang cắt"]);
        let (all, none) = split_for_delete_all(&files, &HashSet::new(), &HashSet::new());
        assert_eq!((all.len(), none.len()), (6, 0));
    }

    #[test]
    fn delete_all_spares_iphone_sessions_with_a_recent_segment() {
        let owner = Owner { user_id: "u", plan: Plan::Free };
        let now = DateTime::now();
        let at = |name: &str, ago_ms: i64| FileDoc {
            created_at: DateTime::from_millis(now.timestamp_millis() - ago_ms),
            ..FileDoc::new_recording(owner, name, "/r", 10, 60.0, false, ObjectId::new())
        };
        let files = vec![
            // Đang ghi: đoạn #0 cũ, đoạn #1 vừa lên.
            at("aaaaaaaa-1111#0", 20 * 60_000),
            at("aaaaaaaa-1111#1", 60_000),
            // Đã xong từ lâu.
            at("bbbbbbbb-2222#0", LIVE_IPHONE_SESSION_MS + 1),
            // Bản ghi trình duyệt và tệp tải lên mới: không phải phiên iPhone.
            at("Họp sáng", 0),
            FileDoc::new_upload(owner, "cccccccc-3333#0", "/u.mp3", 10, 1.0, false, ObjectId::new()).for_transcription(),
        ];
        let live = live_iphone_sessions(&files, now);
        assert_eq!(live, ["s:aaaaaaaa-1111".to_string()].into());
        let (targets, kept) = split_for_delete_all(&files, &live, &HashSet::new());
        let names = |v: Vec<&FileDoc>| v.into_iter().map(|f| f.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(kept), ["aaaaaaaa-1111#0", "aaaaaaaa-1111#1"]);
        assert_eq!(names(targets), ["bbbbbbbb-2222#0", "Họp sáng", "cccccccc-3333#0"]);
    }

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
