//! `files` collection: bản ghi, tệp tải lên, đoạn cắt. Ownership theo `userId`.

use super::plan::Plan;
use crate::config::RETENTION_DAYS;
use crate::error::AppError;
use futures_util::StreamExt;
use mongodb::bson::{doc, oid::ObjectId, DateTime, Document};
use mongodb::{Database, IndexModel};
use serde::{Deserialize, Serialize};

const COLL: &str = "files";

const DAY_MS: i64 = 86_400_000;

/// Loại file hiện ở "Lịch sử ghi" và chép lời được: bản ghi và tệp tải lên (đoạn cắt `output` thì không).
pub const HISTORY_KINDS: [&str; 2] = ["recording", "upload"];

pub fn is_history_kind(kind: &str) -> bool {
    HISTORY_KINDS.contains(&kind)
}

/// `expiresAt` = now + `days`. Sweep retention (retention.rs) là tiến trình DUY NHẤT xoá.
fn expiry_after(days: i64) -> DateTime {
    DateTime::from_millis(DateTime::now().timestamp_millis() + days * DAY_MS)
}

/// Mặc định serde cho doc cũ thiếu `expiresAt`: hạn dài nhất.
fn expiry() -> DateTime {
    expiry_after(RETENTION_DAYS)
}

/// Chủ của file mới và gói của họ lúc tạo: hạn giữ tệp (`expiresAt`) tính theo gói.
#[derive(Debug, Clone, Copy)]
pub struct Owner<'a> {
    pub user_id: &'a str,
    pub plan: Plan,
}

impl Owner<'_> {
    fn expiry(&self) -> DateTime {
        expiry_after(self.plan.storage().retention_days)
    }
}

/// Index `userId` (ownership+quota) + `expiresAt` THƯỜNG (sweep quét quá hạn — KHÔNG `expire_after`
/// TTL native, red-team #1: tránh Mongo xoá doc out-of-band làm mồ côi file disk).
pub async fn ensure_indexes(db: &Database) -> Result<(), String> {
    let coll = db.collection::<FileDoc>(COLL);
    coll.create_index(IndexModel::builder().keys(doc! { "userId": 1 }).build())
        .await
        .map_err(|e| format!("index files.userId: {e}"))?;
    coll.create_index(IndexModel::builder().keys(doc! { "expiresAt": 1 }).build())
        .await
        .map_err(|e| format!("index files.expiresAt: {e}"))?;
    // Backfill doc cũ (Phase 3/4) chưa có expiresAt → tính từ createdAt (đúng chính sách N-ngày-từ-tạo,
    // không phải deploy-time). Pipeline update `$add`. Idempotent (chỉ chạm doc thiếu field).
    let _ = db
        .collection::<Document>(COLL)
        .update_many(
            doc! { "expiresAt": { "$exists": false } },
            vec![doc! { "$set": { "expiresAt": { "$add": ["$createdAt", RETENTION_DAYS * DAY_MS] } } }],
        )
        .await;
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileDoc {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    #[serde(rename = "userId")]
    pub user_id: String,
    pub kind: String, // "upload" | "output" | "recording"
    pub name: String,
    pub path: String,
    pub size: i64,
    /// Thời lượng (giây) — set cho recording/upload đã probe; None với đoạn output.
    #[serde(rename = "durationS", skip_serializing_if = "Option::is_none")]
    pub duration_s: Option<f64>,
    /// Có video stream không (recording): true = màn hình (player <video>), false = mic (<audio>).
    #[serde(rename = "hasVideo", skip_serializing_if = "Option::is_none")]
    pub has_video: Option<bool>,
    #[serde(rename = "jobId", skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime,
    /// Mốc hết hạn — sweep retention xoá file (disk trước, doc sau) khi `expiresAt < now`.
    /// `default`: doc cũ (Phase 3/4) chưa có field → deserialize không vỡ (gán now+retention).
    #[serde(rename = "expiresAt", default = "expiry")]
    pub expires_at: DateTime,
}

impl FileDoc {
    /// Tệp audio/video người dùng tải lên (Cắt file hoặc "Tải tệp lên" ở trang Ghi âm): hiện ở "Lịch sử ghi"
    /// và chép lời được như bản ghi một file (khoá `f:<id>`).
    pub fn new_upload(
        owner: Owner<'_>,
        name: &str,
        path: &str,
        size: i64,
        duration_s: f64,
        has_video: bool,
        id: ObjectId,
    ) -> Self {
        FileDoc {
            kind: "upload".into(),
            ..FileDoc::new_recording(owner, name, path, size, duration_s, has_video, id)
        }
    }
    /// File ghi (Mode A) — kind "recording", có thời lượng + cờ video đã probe. Phát qua `/api/files/:id`.
    pub fn new_recording(
        owner: Owner<'_>,
        name: &str,
        path: &str,
        size: i64,
        duration_s: f64,
        has_video: bool,
        id: ObjectId,
    ) -> Self {
        FileDoc {
            id: Some(id),
            user_id: owner.user_id.into(),
            kind: "recording".into(),
            name: name.into(),
            path: path.into(),
            size,
            duration_s: Some(duration_s),
            has_video: Some(has_video),
            job_id: None,
            created_at: DateTime::now(),
            expires_at: owner.expiry(),
        }
    }
    pub fn new_output(owner: Owner<'_>, job_id: &str, name: &str, path: &str, size: i64) -> Self {
        FileDoc {
            id: None,
            user_id: owner.user_id.into(),
            kind: "output".into(),
            name: name.into(),
            path: path.into(),
            size,
            duration_s: None,
            has_video: None,
            job_id: Some(job_id.into()),
            created_at: DateTime::now(),
            expires_at: owner.expiry(),
        }
    }

    /// Khoá phiên chép lời của file ([`session_key`]); `None` khi chưa có id.
    pub fn session_key(&self) -> Option<String> {
        self.id.map(|id| session_key(&self.kind, &self.name, id))
    }

    /// Thứ tự trong phiên: số đoạn của đoạn iPhone, còn lại là 0.
    pub fn segment_index(&self) -> u32 {
        if self.kind != "recording" {
            return 0;
        }
        parse_segment_name(&self.name).map_or(0, |(_, index)| index)
    }
}

/// Insert file doc → trả `_id` hex.
pub async fn create(db: &Database, doc_: FileDoc) -> Result<String, AppError> {
    let res = db
        .collection::<FileDoc>(COLL)
        .insert_one(doc_)
        .await
        .map_err(|e| AppError::Internal(format!("insert file: {e}")))?;
    res.inserted_id
        .as_object_id()
        .map(|o| o.to_hex())
        .ok_or_else(|| AppError::Internal("file _id không phải ObjectId".into()))
}

/// Lấy file theo id + userId (ownership). None = không tồn tại hoặc không thuộc user.
pub async fn get_owned(
    db: &Database,
    file_id: &str,
    user_id: &str,
) -> Result<Option<FileDoc>, AppError> {
    let oid = ObjectId::parse_str(file_id)
        .map_err(|_| AppError::BadRequest("file id không hợp lệ".into()))?;
    db.collection::<FileDoc>(COLL)
        .find_one(doc! { "_id": oid, "userId": user_id })
        .await
        .map_err(|e| AppError::Internal(format!("find file: {e}")))
}

/// Các file của user trong `ids` (bỏ qua id không thuộc user), một truy vấn.
pub async fn list_owned_by_ids(db: &Database, user_id: &str, ids: &[ObjectId]) -> Result<Vec<FileDoc>, AppError> {
    let mut cur = db
        .collection::<FileDoc>(COLL)
        .find(doc! { "_id": { "$in": ids }, "userId": user_id })
        .await
        .map_err(|e| AppError::Internal(format!("find files: {e}")))?;
    let mut out = Vec::new();
    while let Some(r) = cur.next().await {
        out.push(r.map_err(|e| AppError::Internal(format!("đọc file: {e}")))?);
    }
    Ok(out)
}

/// Các đoạn output của 1 job (ownership).
pub async fn list_outputs_for_job(
    db: &Database,
    job_id: &str,
    user_id: &str,
) -> Result<Vec<FileDoc>, AppError> {
    let mut cur = db
        .collection::<FileDoc>(COLL)
        .find(doc! { "jobId": job_id, "userId": user_id, "kind": "output" })
        .await
        .map_err(|e| AppError::Internal(format!("find outputs: {e}")))?;
    let mut out = Vec::new();
    while let Some(r) = cur.next().await {
        out.push(r.map_err(|e| AppError::Internal(format!("đọc output: {e}")))?);
    }
    Ok(out)
}

/// Lịch sử ghi của user (bản ghi + tệp tải lên), mới nhất trước.
pub async fn list_history(db: &Database, user_id: &str) -> Result<Vec<FileDoc>, AppError> {
    let mut cur = db
        .collection::<FileDoc>(COLL)
        .find(doc! { "userId": user_id, "kind": { "$in": HISTORY_KINDS.to_vec() } })
        .sort(doc! { "createdAt": -1 })
        .await
        .map_err(|e| AppError::Internal(format!("find recordings: {e}")))?;
    let mut out = Vec::new();
    while let Some(r) = cur.next().await {
        out.push(r.map_err(|e| AppError::Internal(format!("đọc recording: {e}")))?);
    }
    Ok(out)
}

/// File KHÔNG phải bản ghi (upload nguồn Cắt file + đoạn output), mới nhất trước — để UI quản lý
/// dung lượng (những file này tính quota nhưng không hiện ở "Lịch sử ghi").
pub async fn list_non_recordings(db: &Database, user_id: &str) -> Result<Vec<FileDoc>, AppError> {
    let mut cur = db
        .collection::<FileDoc>(COLL)
        .find(doc! { "userId": user_id, "kind": { "$in": ["upload", "output"] } })
        .sort(doc! { "createdAt": -1 })
        .await
        .map_err(|e| AppError::Internal(format!("find non-recordings: {e}")))?;
    let mut out = Vec::new();
    while let Some(r) = cur.next().await {
        out.push(r.map_err(|e| AppError::Internal(format!("đọc file: {e}")))?);
    }
    Ok(out)
}

/// Các file đã quá hạn (`expiresAt < now`) — sweep retention xoá.
pub async fn find_expired(db: &Database) -> Result<Vec<FileDoc>, AppError> {
    let mut cur = db
        .collection::<FileDoc>(COLL)
        .find(doc! { "expiresAt": { "$lt": DateTime::now() } })
        .await
        .map_err(|e| AppError::Internal(format!("find expired: {e}")))?;
    let mut out = Vec::new();
    while let Some(r) = cur.next().await {
        out.push(r.map_err(|e| AppError::Internal(format!("đọc expired: {e}")))?);
    }
    Ok(out)
}

/// Xoá file doc (gọi SAU khi đã xoá file disk — thứ tự cố định, red-team #1).
///
/// Bản chép lời đi theo file: đây là nơi duy nhất xoá doc file (xoá tay, retention, cắt bản ghi).
/// Câu sửa của đoạn và biên bản của phiên trong `transcript_docs` dọn theo (phiên hết file thì xoá cả
/// doc), lỗi chỉ `warn` (đĩa đã xoá, không báo lỗi ngược); sweep mồ côi trong retention là cơ chế đảm bảo.
/// Câu của file bị xoá khỏi index tìm kiếm ngay (đồng bộ), rồi index của phiên được dựng lại (nền) để mốc
/// giây của các đoạn còn lại dời theo; quét câu mồ côi trong retention và TTL theo hạn file là cơ chế đảm
/// bảo của index.
pub async fn delete_doc(db: &Database, f: &FileDoc) -> Result<(), AppError> {
    let Some(id) = f.id else { return Ok(()) };
    db.collection::<FileDoc>(COLL)
        .delete_one(doc! { "_id": id })
        .await
        .map_err(|e| AppError::Internal(format!("xoá file doc: {e}")))?;
    super::transcript::delete(db, id).await?;
    if is_history_kind(&f.kind) {
        let key = session_key(&f.kind, &f.name, id);
        if let Err(e) = super::transcript_doc::forget_segment(db, &f.user_id, &key, id).await {
            tracing::warn!(file = %id, "dọn câu sửa và biên bản của phiên lỗi (sweep mồ côi sẽ dọn): {e:?}");
        }
        if let Err(e) = crate::search::index::forget_file(db, &f.user_id, &key, id).await {
            tracing::warn!(file = %id, "xoá câu của file khỏi index lỗi (sweep mồ côi sẽ dọn): {e:?}");
        }
        crate::search::index::schedule(db, &f.user_id, &key);
    }
    Ok(())
}

/// `<sessionId>#<index>` — tên đoạn do app iPhone đặt (`UploadQueue.swift`). Cùng luật với
/// `parseSegmentName` của web (`recording-groups.ts`), thêm trần 64 ký tự cho id phiên.
pub fn parse_segment_name(name: &str) -> Option<(&str, u32)> {
    let (session, index) = name.split_once('#')?;
    let session_ok = (8..=64).contains(&session.len())
        && session.bytes().all(|b| b.is_ascii_digit() || b.is_ascii_lowercase() || b == b'-');
    // Tối đa 9 chữ số (cùng luật với web): luôn vừa u32.
    let index_ok = (1..=9).contains(&index.len()) && index.bytes().all(|b| b.is_ascii_digit());
    if !session_ok || !index_ok {
        return None;
    }
    Some((session, index.parse().ok()?))
}

/// Khoá phiên của một file: `s:<sessionId>` cho đoạn iPhone, `f:<id>` cho bản ghi một file và tệp tải lên
/// (tên tệp tải lên do người dùng đặt nên không bao giờ được coi là đoạn iPhone). Tiền tố để tên do client
/// đặt không trùng được khoá của file khác.
pub fn session_key(kind: &str, name: &str, id: ObjectId) -> String {
    match parse_segment_name(name).filter(|_| kind == "recording") {
        Some((session, _)) => format!("s:{session}"),
        None => format!("f:{}", id.to_hex()),
    }
}

/// Khoá có đúng một trong hai dạng của [`session_key`] không (kiểm trước khi đưa vào truy vấn).
pub fn is_valid_session_key(key: &str) -> bool {
    if let Some(session) = key.strip_prefix("s:") {
        return parse_segment_name(&format!("{session}#0")).is_some();
    }
    key.strip_prefix("f:")
        .is_some_and(|hex| hex.len() == 24 && hex.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
}

/// Các file của user thuộc phiên `key`, theo thứ tự ghi (index đoạn; bản ghi một file và tệp tải lên là 0).
/// `key` phải hợp lệ ([`is_valid_session_key`]); khoá sai dạng trả rỗng.
pub async fn list_session(db: &Database, user_id: &str, key: &str) -> Result<Vec<(u32, FileDoc)>, AppError> {
    if !is_valid_session_key(key) {
        return Ok(Vec::new());
    }
    let filter = if let Some(session) = key.strip_prefix("s:") {
        // `session` chỉ gồm [0-9a-z-] (đã kiểm) nên đặt thẳng vào regex được.
        doc! { "userId": user_id, "kind": "recording", "name": { "$regex": format!("^{session}#[0-9]+$") } }
    } else if let Some(oid) = key.strip_prefix("f:").and_then(|hex| ObjectId::parse_str(hex).ok()) {
        doc! { "_id": oid, "userId": user_id, "kind": { "$in": HISTORY_KINDS.to_vec() } }
    } else {
        return Ok(Vec::new());
    };
    let mut cur = db
        .collection::<FileDoc>(COLL)
        .find(filter)
        .await
        .map_err(|e| AppError::Internal(format!("find phiên ghi: {e}")))?;
    let mut out = Vec::new();
    while let Some(r) = cur.next().await {
        let f = r.map_err(|e| AppError::Internal(format!("đọc phiên ghi: {e}")))?;
        // Bản ghi trình duyệt tình cờ đặt tên giống đoạn iPhone thì thuộc phiên `s:`, không phải `f:`.
        if f.session_key().as_deref() != Some(key) {
            continue;
        }
        out.push((f.segment_index(), f));
    }
    out.sort_by_key(|(index, _)| *index);
    Ok(out)
}

/// Tên và lúc tạo của các bản ghi thuộc các phiên `keys` (khoá sai dạng bị bỏ qua), một truy vấn chỉ đọc
/// `_id`, `name`, `createdAt`. Trả `(khoá phiên, tên, lúc tạo)` mỗi bản ghi.
pub async fn session_heads(db: &Database, user_id: &str, keys: &[String]) -> Result<Vec<(String, String, DateTime)>, AppError> {
    let wanted: std::collections::HashSet<&str> = keys.iter().map(String::as_str).collect();
    let mut ids = Vec::new();
    let mut sessions = Vec::new();
    for key in keys.iter().filter(|k| is_valid_session_key(k)) {
        if let Some(session) = key.strip_prefix("s:") {
            sessions.push(session.to_string());
        } else if let Some(oid) = key.strip_prefix("f:").and_then(|hex| ObjectId::parse_str(hex).ok()) {
            ids.push(oid);
        }
    }
    let mut any = Vec::new();
    if !ids.is_empty() {
        any.push(doc! { "_id": { "$in": ids }, "kind": { "$in": HISTORY_KINDS.to_vec() } });
    }
    if !sessions.is_empty() {
        // Id phiên chỉ gồm [0-9a-z-] (đã kiểm) nên ghép thẳng vào regex được.
        any.push(doc! { "kind": "recording", "name": { "$regex": format!("^({})#[0-9]+$", sessions.join("|")) } });
    }
    if any.is_empty() {
        return Ok(Vec::new());
    }
    let read = |e: mongodb::error::Error| AppError::Internal(format!("đọc bản ghi của phiên: {e}"));
    let mut cur = db
        .collection::<Document>(COLL)
        .find(doc! { "userId": user_id, "$or": any })
        .projection(doc! { "_id": 1, "kind": 1, "name": 1, "createdAt": 1 })
        .await
        .map_err(read)?;
    let mut out = Vec::new();
    while let Some(r) = cur.next().await {
        let d = r.map_err(read)?;
        let (Ok(id), Ok(kind), Ok(name), Ok(created_at)) =
            (d.get_object_id("_id"), d.get_str("kind"), d.get_str("name"), d.get_datetime("createdAt"))
        else {
            continue;
        };
        let key = session_key(kind, name, id);
        if wanted.contains(key.as_str()) {
            out.push((key, name.to_string(), *created_at));
        }
    }
    Ok(out)
}

/// File có `userId` khớp `users` (điều kiện Mongo, vd `{"$nin": [...]}`) mà `expiresAt` muộn hơn
/// `createdAt + days` (lọc của lệnh admin rút hạn tệp gói free).
fn longer_than_filter(users: Document, days: i64) -> Document {
    doc! {
        "userId": users,
        "$expr": { "$gt": ["$expiresAt", { "$add": ["$createdAt", days * DAY_MS] }] },
    }
}

/// Đếm file sẽ bị rút hạn: `(số file, số user, số file hết hạn ngay khi rút)`.
pub async fn count_longer_than(db: &Database, users: Document, days: i64) -> Result<(u64, u64, u64), AppError> {
    let read = |e: mongodb::error::Error| AppError::Internal(format!("đếm file theo hạn: {e}"));
    let filter = longer_than_filter(users, days);
    let coll = db.collection::<Document>(COLL);
    let files = coll.count_documents(filter.clone()).await.map_err(read)?;
    let users = coll.distinct("userId", filter.clone()).await.map_err(read)?.len() as u64;
    let cutoff = DateTime::from_millis(DateTime::now().timestamp_millis() - days * DAY_MS);
    let mut due = filter;
    due.insert("createdAt", doc! { "$lte": cutoff });
    let due_now = coll.count_documents(due).await.map_err(read)?;
    Ok((files, users, due_now))
}

/// Rút `expiresAt` về `createdAt + days` cho file có `userId` khớp `users` (chỉ file có hạn muộn hơn). File đã quá hạn mới bị sweep retention xoá ở vòng kế tiếp. Trả số file đã đổi.
pub async fn shorten_expiry(db: &Database, users: Document, days: i64) -> Result<u64, AppError> {
    db.collection::<Document>(COLL)
        .update_many(
            longer_than_filter(users, days),
            vec![doc! { "$set": { "expiresAt": { "$add": ["$createdAt", days * DAY_MS] } } }],
        )
        .await
        .map(|r| r.modified_count)
        .map_err(|e| AppError::Internal(format!("rút hạn file: {e}")))
}

/// Tổng byte đã dùng của user (quota — red-team #3).
pub async fn user_usage_bytes(db: &Database, user_id: &str) -> Result<u64, AppError> {
    let pipeline = vec![
        doc! { "$match": { "userId": user_id } },
        doc! { "$group": { "_id": null, "total": { "$sum": "$size" } } },
    ];
    let mut cur = db
        .collection::<Document>(COLL)
        .aggregate(pipeline)
        .await
        .map_err(|e| AppError::Internal(format!("aggregate usage: {e}")))?;
    if let Some(r) = cur.next().await {
        let d = r.map_err(|e| AppError::Internal(format!("đọc usage: {e}")))?;
        let total = d
            .get_i64("total")
            .or_else(|_| d.get_i32("total").map(i64::from))
            .unwrap_or(0);
        return Ok(total.max(0) as u64);
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_key_groups_ios_segments_and_keeps_other_recordings_alone() {
        let id = ObjectId::parse_str("65f0a1b2c3d4e5f60718293a").expect("oid");
        let session_key = |name: &str, id| super::session_key("recording", name, id);
        let file_key = "f:65f0a1b2c3d4e5f60718293a";
        // Cùng ví dụ với `recording-groups.test.ts` của web.
        assert_eq!(session_key("6ad09d56-a5e5#12", id), "s:6ad09d56-a5e5");
        assert_eq!(session_key("aaaaaaaa-1111#0", id), "s:aaaaaaaa-1111");
        assert_eq!(session_key("Bản ghi #3", id), file_key);
        assert_eq!(session_key("abc#1", id), file_key);
        assert_eq!(session_key("Bản ghi 16:31:09", id), file_key);
        // Phiên iPhone thật (UUID chữ thường) và phiên selftest của RecorderPlugin.
        assert_eq!(
            session_key("0b6c3f9e-2d4a-4f8e-9a51-7c2b1e0d3f44#48", id),
            "s:0b6c3f9e-2d4a-4f8e-9a51-7c2b1e0d3f44"
        );
        assert_eq!(session_key("selftest-1760590000#0", id), "s:selftest-1760590000");
        // Chữ hoa, index rỗng/không phải số, id phiên quá dài: không phải đoạn iPhone.
        assert_eq!(session_key("ABCDEFGH-1111#0", id), file_key);
        assert_eq!(session_key("aaaaaaaa-1111#", id), file_key);
        assert_eq!(session_key("aaaaaaaa-1111#1a", id), file_key);
        assert_eq!(session_key("aaaaaaaa-1111#1234567890", id), file_key);
        assert_eq!(session_key(&format!("{}#0", "a".repeat(65)), id), file_key);
        assert_eq!(parse_segment_name("6ad09d56-a5e5#12"), Some(("6ad09d56-a5e5", 12)));
    }

    #[test]
    fn uploads_are_single_file_sessions_whatever_their_name() {
        let id = ObjectId::parse_str("65f0a1b2c3d4e5f60718293a").expect("oid");
        let owner = Owner { user_id: "u", plan: Plan::Free };
        let upload = FileDoc::new_upload(owner, "6ad09d56-a5e5#12", "/x.mp3", 1, 30.0, false, id);
        assert_eq!(upload.kind, "upload");
        assert_eq!(upload.session_key().as_deref(), Some("f:65f0a1b2c3d4e5f60718293a"));
        assert_eq!(upload.segment_index(), 0);
        let segment = FileDoc::new_recording(owner, "6ad09d56-a5e5#12", "/x.m4a", 1, 60.0, false, id);
        assert_eq!(segment.session_key().as_deref(), Some("s:6ad09d56-a5e5"));
        assert_eq!(segment.segment_index(), 12);
        assert!(is_history_kind("upload") && is_history_kind("recording") && !is_history_kind("output"));
    }

    #[test]
    fn new_files_expire_after_the_owner_plan_retention() {
        let days = |plan| {
            let f = FileDoc::new_recording(Owner { user_id: "u", plan }, "a", "/a", 1, 1.0, false, ObjectId::new());
            (f.expires_at.timestamp_millis() - f.created_at.timestamp_millis() + DAY_MS / 2) / DAY_MS
        };
        assert_eq!(days(Plan::Free), 7);
        assert_eq!(days(Plan::Plus), 14);
        let output = FileDoc::new_output(Owner { user_id: "u", plan: Plan::Free }, "j", "p1", "/p1", 1);
        assert_eq!((output.expires_at.timestamp_millis() - output.created_at.timestamp_millis() + DAY_MS / 2) / DAY_MS, 7);
    }

    #[test]
    fn session_keys_are_validated_before_queries() {
        assert!(is_valid_session_key("s:6ad09d56-a5e5"));
        assert!(is_valid_session_key("f:65f0a1b2c3d4e5f60718293a"));
        assert!(!is_valid_session_key("s:abc"));
        assert!(!is_valid_session_key("s:6ad09d56-a5e5#1"));
        assert!(!is_valid_session_key("s:6ad09d56.a5e5"));
        assert!(!is_valid_session_key("f:65F0A1B2C3D4E5F60718293A"));
        assert!(!is_valid_session_key("f:65f0a1b2"));
        assert!(!is_valid_session_key("x:6ad09d56-a5e5"));
        assert!(!is_valid_session_key(""));
    }

    /// Trên Mongo thật (DB dev): `TRANSCRIPT_TEST_MONGO_URI=… TRANSCRIPT_TEST_DB_NAME=… cargo test
    /// shorten_expiry_on_real_mongo -- --ignored`. User giả riêng, tự dọn doc đã tạo.
    #[tokio::test]
    #[ignore]
    async fn shorten_expiry_on_real_mongo() {
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        let user = format!("selftest-{}", uuid::Uuid::new_v4());
        let owner = Owner { user_id: &user, plan: Plan::Plus };
        let mut old = FileDoc::new_recording(owner, "cũ", "/nonexistent", 1, 1.0, false, ObjectId::new());
        old.created_at = DateTime::from_millis(old.created_at.timestamp_millis() - 10 * DAY_MS);
        old.expires_at = DateTime::from_millis(old.created_at.timestamp_millis() + 14 * DAY_MS);
        let fresh = FileDoc::new_recording(owner, "mới", "/nonexistent", 1, 1.0, false, ObjectId::new());
        let free = FileDoc::new_upload(Owner { user_id: &user, plan: Plan::Free }, "free", "/n", 1, 1.0, false, ObjectId::new());
        let (old_id, fresh_id, free_id) = (old.id, fresh.id, free.id);
        for f in [old, fresh, free] {
            create(&db, f).await.expect("tạo file");
        }
        let scope = || doc! { "$eq": &user };
        assert_eq!(count_longer_than(&db, scope(), 7).await.expect("đếm"), (2, 1, 1));
        assert_eq!(shorten_expiry(&db, scope(), 7).await.expect("rút"), 2);
        assert_eq!(count_longer_than(&db, scope(), 7).await.expect("đếm lại"), (0, 0, 0));
        let coll = db.collection::<FileDoc>(COLL);
        let get = |id: Option<ObjectId>| coll.find_one(doc! { "_id": id });
        let old = get(old_id).await.expect("đọc").expect("cũ");
        assert_eq!(old.expires_at.timestamp_millis() - old.created_at.timestamp_millis(), 7 * DAY_MS);
        assert!(find_expired(&db).await.expect("quá hạn").iter().any(|f| f.id == old_id));
        let fresh = get(fresh_id).await.expect("đọc").expect("mới");
        assert_eq!(fresh.expires_at.timestamp_millis() - fresh.created_at.timestamp_millis(), 7 * DAY_MS);
        let untouched = get(free_id).await.expect("đọc").expect("free");
        assert!(untouched.expires_at.timestamp_millis() - untouched.created_at.timestamp_millis() <= 7 * DAY_MS);
        coll.delete_many(doc! { "userId": &user }).await.expect("dọn");
    }

    /// Tệp tải lên là phiên một file `f:<id>` kể cả khi tên giống đoạn iPhone (DB dev, như test trên).
    #[tokio::test]
    #[ignore]
    async fn uploads_are_single_file_sessions_on_real_mongo() {
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        let user = format!("selftest-{}", uuid::Uuid::new_v4());
        let session = format!("selftest-{}", &uuid::Uuid::new_v4().simple().to_string()[..12]);
        let owner = Owner { user_id: &user, plan: Plan::Free };
        let upload = FileDoc::new_upload(owner, &format!("{session}#0"), "/nonexistent", 5, 9.0, false, ObjectId::new());
        let segment = FileDoc::new_recording(owner, &format!("{session}#1"), "/nonexistent", 7, 60.0, false, ObjectId::new());
        let upload_key = upload.session_key().expect("khoá");
        for f in [upload, segment] {
            create(&db, f).await.expect("tạo file");
        }
        let names = |v: Vec<(u32, FileDoc)>| v.into_iter().map(|(i, f)| (i, f.kind)).collect::<Vec<_>>();
        assert_eq!(names(list_session(&db, &user, &upload_key).await.expect("f:")), [(0, "upload".to_string())]);
        assert_eq!(names(list_session(&db, &user, &format!("s:{session}")).await.expect("s:")), [(1, "recording".to_string())]);
        let heads = session_heads(&db, &user, std::slice::from_ref(&upload_key)).await.expect("heads");
        assert_eq!(heads.len(), 1);
        assert_eq!(heads[0].0, upload_key);
        assert_eq!(list_history(&db, &user).await.expect("lịch sử").len(), 2);
        assert_eq!(user_usage_bytes(&db, &user).await.expect("dung lượng"), 12);
        for f in list_history(&db, &user).await.expect("lịch sử") {
            delete_doc(&db, &f).await.expect("xoá");
        }
        assert!(list_history(&db, &user).await.expect("lịch sử").is_empty());
    }
}
