//! Xoá dữ liệu Scribe của một tài khoản khi IdP báo tài khoản đã xoá (`POST /api/idp/account-deleted`, auth-next
//! ADR 0018). Tài khoản chỉ xoá ở auth.hoandat.com/account: Scribe không có nút, màn hay API xoá; IdP gửi một SET ký
//! ES256 và Scribe xoá ngay mọi dữ liệu của chính nó. Scribe không có tài sản dùng chung với người khác (biên bản
//! chia sẻ bằng link là của chủ, xoá theo), nên không có gì phải chuyển chủ.
//!
//! **Xoá hẳn**: tệp trên đĩa (cả thư mục `{data_dir}/<userId>`) và doc `files`; lượt chép (`transcripts`, lượt
//! còn chạy được quyết toán như khi xoá file); `transcript_docs` (tiêu đề, câu sửa, biên bản, dấu mốc, tên người
//! nói); bản dịch; index tìm kiếm; job cắt; phiên tải lên/ghi dở; Key riêng và lựa chọn; token liên kết chat; phiên
//! đăng nhập và thiết bị (grant ở IdP được thu hồi trước); sổ dùng riêng của user (`u:`/`d:`/`b:` ở
//! `transcribe_usage`, lượt ngày và chi phí tháng ở `summary_usage`, dấu đếm lần chạm giới hạn ngày); doc `users`
//! (email, tên, gói, liên kết chat).
//!
//! **Giữ, không còn gắn với người**: sổ chi tiết `transcribe_runs`, `summary_runs`, `ask_runs`, `translate_runs` và
//! token chép trực tiếp `realtime_tokens` (token chưa quyết toán vẫn được lệnh đối soát tính trọn phần giữ chỗ) đổi
//! `userId` thành [`DELETED_USER`] và bỏ id file/phiên/token — chi phí đã chi vẫn nằm trong trần tổng tháng và
//! `admin usage --recompute` vẫn cộng đúng. Sổ chung (`free:`, `all:`, `el:`) không có gì của user. `audit_events`
//! giữ tới hết hạn TTL 180 ngày (nhật ký bảo mật, chống lạm dụng): `userId` chỉ còn là một id không trỏ tới ai, bỏ
//! các trường có nội dung của người dùng (tên tệp, email, tiêu đề, id chat, câu lỗi, lý do khoá do quản trị ghi).
//!
//! **Trình tự và việc đang chạy**: tài khoản đã xoá ở IdP nên không từ chối (cả khi đang ghi: phiên ghi bị xoá
//! cùng mọi thứ). (1) `users.deletingAt` + `deleted_users` (chỉ id, TTL [`TOMBSTONE_DAYS`] ngày) — từ đây đăng nhập (web, iPhone,
//! làm mới phiên) bị từ chối và tiếp tục việc xoá, middleware phiên coi tài khoản như không còn, worker bỏ qua lượt
//! của user; (2) thu hồi grant IdP của các thiết bị (lỗi thì giữ bản mã hoá trong `deleted_users` để thử lại),
//! xoá phiên và bỏ khỏi cache; (3) [`purge_user`]. Bước (1) là hàng đợi bền: ghi xong thì trả 202 (lỗi trước đó trả
//! 503 để IdP gửi lại). Bước (2)–(3) chạy trong task riêng: IdP ngắt kết nối không làm dừng việc xoá; lỗi ở đây
//! chỉ ghi log, vòng retention làm nốt. Việc nền ghi thêm sau khi xoá (vd
//! lượt chép xong đặt tên người nói bằng upsert) được vòng retention mỗi giờ dọn lại qua `deleted_users`.

use crate::audit;
use crate::error::AppError;
use crate::models::session as sessions;
use crate::models::{file, transcript, user};
use crate::oidc::SetError;
use crate::session::open;
use crate::state::AppState;
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use futures_util::StreamExt;
use mongodb::bson::{doc, oid::ObjectId, Bson, DateTime, Document, Regex};
use mongodb::options::IndexOptions;
use mongodb::{Database, IndexModel};
use std::path::{Component, Path};
use std::time::Duration;

const TOMBSTONES: &str = "deleted_users";
/// Vòng retention còn dọn lại dữ liệu của tài khoản đã xoá trong chừng này ngày.
const TOMBSTONE_DAYS: u64 = 7;
/// `userId` của sổ chi tiết sau khi tài khoản bị xoá.
pub const DELETED_USER: &str = "deleted";
/// Đường dẫn IdP gọi khi một tài khoản bị xoá; `aud` của SET là `{PUBLIC_URL}` + đường dẫn này.
pub const ACCOUNT_DELETED_PATH: &str = "/api/idp/account-deleted";
/// SET (RFC 8417) chỉ vài trăm byte.
pub const ACCOUNT_DELETED_BODY_LIMIT: usize = 8 * 1024;
const SET_MEDIA_TYPE: &str = "application/secevent+jwt";

/// Xoá thẳng theo `userId`.
const BY_USER: [&str; 12] = [
    "transcript_docs",
    "transcript_translations",
    "transcript_sentences",
    "jobs",
    "upload_sessions",
    "recording_sessions",
    "user_keys",
    "chat_link_tokens",
    "sessions",
    "devices",
    // Phần còn sót sau `file::delete_doc` (doc file không còn trên đĩa vẫn bị xoá ở bước trước).
    "files",
    "transcripts",
];

/// Sổ chi tiết: giữ số tiền, bỏ dấu vết người dùng.
const RUN_LEDGERS: [&str; 5] = ["transcribe_runs", "summary_runs", "ask_runs", "translate_runs", "realtime_tokens"];
/// Trường nối sổ chi tiết với tệp, phiên, token hay key của user.
const RUN_LINKS: [&str; 6] = ["fileId", "key", "sessionId", "tokenId", "runId", "keyNonce"];

/// Trường của audit có nội dung do người dùng đặt (tên tệp, tiêu đề, câu lỗi có tên tệp) hay định danh (email,
/// id chat). `reason` của `admin.block` là chữ quản trị gõ, bỏ riêng.
const AUDIT_PERSONAL_FIELDS: [&str; 5] = ["name", "email", "title", "chatId", "error"];

/// Collection [`purge_user`] lọc theo `userId` mà chưa có index bắt đầu bằng `userId`.
const USER_INDEXED: [&str; 12] = [
    "transcripts",
    "jobs",
    "upload_sessions",
    "recording_sessions",
    "chat_link_tokens",
    "sessions",
    "devices",
    "transcribe_runs",
    "summary_runs",
    "ask_runs",
    "translate_runs",
    "realtime_tokens",
];

/// TTL của `deleted_users` và index `userId` cho các bước xoá. Gọi lúc khởi động.
pub async fn ensure_indexes(db: &Database) -> Result<(), String> {
    db.collection::<Document>(TOMBSTONES)
        .create_index(IndexModel::builder().keys(doc! { "devices": 1 }).build())
        .await
        .map_err(|e| format!("index deleted_users.devices: {e}"))?;
    db.collection::<Document>(TOMBSTONES)
        .create_index(
            IndexModel::builder()
                .keys(doc! { "at": 1 })
                .options(IndexOptions::builder().expire_after(Duration::from_secs(TOMBSTONE_DAYS * 86_400)).build())
                .build(),
        )
        .await
        .map_err(|e| format!("index deleted_users.at (TTL): {e}"))?;
    for coll in USER_INDEXED {
        db.collection::<Document>(coll)
            .create_index(IndexModel::builder().keys(doc! { "userId": 1 }).build())
            .await
            .map_err(|e| format!("index {coll}.userId: {e}"))?;
    }
    Ok(())
}

/// Tiền tố `_id` của các sổ dùng riêng của user, mỗi tiền tố một regex `^…` (dùng được index `_id`).
fn ledger_prefixes(user_id: &str) -> [(&'static str, Vec<String>); 3] {
    [
        // Sổ phút tháng, sổ ngày gói free, sổ phút bằng key riêng.
        ("transcribe_usage", ["u", "d", "b"].iter().map(|p| format!("{p}:{user_id}:")).collect()),
        // Lượt ngày tóm tắt / hỏi đáp / dịch, chi phí tháng.
        (
            "summary_usage",
            ["", "ask:", "tr:", "cost:"].iter().map(|p| format!("{p}{user_id}:")).collect(),
        ),
        // Đếm lần chạm giới hạn ngày.
        ("transcribe_marks", vec![format!("hit:daily:{user_id}:")]),
    ]
}

/// Điều kiện `_id` bắt đầu bằng một trong `prefixes` (`user_id` là 24 hex nên không cần thoát ký tự).
fn prefix_filter(prefixes: &[String]) -> Document {
    let any: Vec<Bson> = prefixes
        .iter()
        .map(|p| Bson::RegularExpression(Regex { pattern: format!("^{p}"), options: String::new() }))
        .collect();
    doc! { "_id": { "$in": any } }
}

/// Tệp `path` nằm trong thư mục của user (`{data_dir}/<userId>/`) và không có `..`: chỉ những tệp này được xoá.
fn inside_user_dir(path: &str, data_dir: &str, user_id: &str) -> bool {
    let dir = Path::new(data_dir).join(user_id);
    let path = Path::new(path);
    path.starts_with(&dir) && path != dir && !path.components().any(|c| matches!(c, Component::ParentDir))
}

/// Số doc/tệp đã xoá của một lần [`purge_user`].
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Purged {
    pub files: u64,
    pub bytes: u64,
    pub docs: u64,
    /// Tệp không xoá được trên đĩa (doc giữ lại, vòng sau thử lại).
    pub failed: u64,
}

/// Xoá mọi dữ liệu của `user_id` (xem đầu tệp). Chạy lại được: lần sau chỉ dọn phần còn sót.
pub async fn purge_user(db: &Database, data_dir: &str, user_id: &str) -> Result<Purged, AppError> {
    // Id user là ObjectId: chặn mọi đường dẫn lạ trước khi đụng tới đĩa.
    let oid = ObjectId::parse_str(user_id).map_err(|_| AppError::BadRequest("user id không hợp lệ".into()))?;
    let fail = |what: &'static str| move |e: mongodb::error::Error| AppError::Internal(format!("xoá tài khoản, {what}: {e}"));
    let mut purged = Purged::default();

    // 1. Tệp: đĩa trước, doc sau (như xoá tay); `delete_doc` quyết toán lượt chép đang chạy và dọn bản chép lời.
    let mut cur = db.collection::<file::FileDoc>("files").find(doc! { "userId": user_id }).await.map_err(fail("đọc tệp"))?;
    let mut files = Vec::new();
    while let Some(f) = cur.next().await {
        files.push(f.map_err(fail("đọc tệp"))?);
    }
    let mut kept = Vec::new();
    for f in &files {
        if inside_user_dir(&f.path, data_dir, user_id) {
            match tokio::fs::remove_file(&f.path).await {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    tracing::warn!(path = %f.path, "xoá tài khoản: không xoá được tệp, giữ doc để thử lại: {e}");
                    purged.failed += 1;
                    kept.extend(f.id);
                    continue;
                }
            }
        } else {
            // Doc trỏ ra ngoài thư mục của user (không bao giờ do Scribe tạo): không đụng tới đĩa, chỉ bỏ doc.
            tracing::error!(path = %f.path, "xoá tài khoản: đường dẫn tệp ngoài thư mục của user, bỏ qua đĩa");
        }
        file::delete_doc(db, f).await?;
        purged.files += 1;
        purged.bytes += f.size.max(0) as u64;
    }
    // Lượt chép không còn file (hiếm): quyết toán rồi xoá.
    purged.docs += transcript::delete_user(db, user_id).await?;

    // 2. Mọi collection theo `userId`. Tệp không xoá được trên đĩa thì giữ doc `files` cho vòng sau.
    for coll in BY_USER {
        let mut filter = doc! { "userId": user_id };
        if coll == "files" && !kept.is_empty() {
            filter.insert("_id", doc! { "$nin": kept.clone() });
        }
        purged.docs += db.collection::<Document>(coll).delete_many(filter).await.map_err(fail(coll))?.deleted_count;
    }
    purged.docs += db
        .collection::<Document>("user_key_prefs")
        .delete_many(doc! { "_id": user_id })
        .await
        .map_err(fail("user_key_prefs"))?
        .deleted_count;
    for (coll, prefixes) in ledger_prefixes(user_id) {
        purged.docs += db
            .collection::<Document>(coll)
            .delete_many(prefix_filter(&prefixes))
            .await
            .map_err(fail(coll))?
            .deleted_count;
    }

    // 3. Sổ chi tiết và audit: giữ, bỏ dấu vết người dùng.
    let unlink: Document = RUN_LINKS.iter().map(|f| (f.to_string(), Bson::String(String::new()))).collect();
    for coll in RUN_LEDGERS {
        db.collection::<Document>(coll)
            .update_many(doc! { "userId": user_id }, doc! { "$set": { "userId": DELETED_USER }, "$unset": unlink.clone() })
            .await
            .map_err(fail(coll))?;
    }
    let audit = db.collection::<Document>("audit_events");
    let unset: Document = AUDIT_PERSONAL_FIELDS.iter().map(|f| (f.to_string(), Bson::String(String::new()))).collect();
    audit.update_many(doc! { "userId": user_id }, doc! { "$unset": unset }).await.map_err(fail("audit_events"))?;
    audit
        .update_many(doc! { "userId": user_id, "action": "admin.block" }, doc! { "$unset": { "reason": "" } })
        .await
        .map_err(fail("audit_events"))?;

    // 4. Thư mục của user (tmp, zip, đoạn cắt còn sót) khi mọi tệp đã xoá được.
    if purged.failed == 0 {
        match tokio::fs::remove_dir_all(Path::new(data_dir).join(user_id)).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                tracing::warn!("xoá tài khoản: không xoá được thư mục của user: {e}");
                purged.failed += 1;
            }
        }
    }

    // 5. Cuối cùng là user (email, tên, gói, liên kết chat): còn doc này (có `deletingAt`) thì đăng nhập vẫn bị chặn
    //    và lần chạy lại vẫn biết đây là ai.
    if purged.failed == 0 {
        purged.docs += db.collection::<Document>("users").delete_one(doc! { "_id": oid }).await.map_err(fail("users"))?.deleted_count;
    }
    Ok(purged)
}

/// Bước (1): dấu xoá trên user và id trong `deleted_users`.
async fn mark(db: &Database, user_id: &str) -> Result<(), AppError> {
    db.collection::<Document>(TOMBSTONES)
        .update_one(doc! { "_id": user_id }, doc! { "$set": { "at": DateTime::now() } })
        .upsert(true)
        .await
        .map_err(|e| AppError::Internal(format!("ghi tài khoản đã xoá: {e}")))?;
    user::mark_deleting(db, user_id).await
}

/// Thu hồi grant IdP của các refresh token (bản mã hoá); trả những token chưa thu hồi được.
async fn revoke_all(state: &AppState, sealed: Vec<String>) -> Vec<String> {
    let mut left = Vec::new();
    for s in sealed {
        // Không giải mã được (khoá phiên đã đổi): không còn gì để thu hồi.
        let Some(refresh) = open(&state.session_key, &s) else { continue };
        if !state.oidc.revoke(&state.http, &state.config.oidc_mobile_client_id, &refresh).await {
            left.push(s);
        }
    }
    left
}

/// Bước (2)–(3): thu hồi thiết bị, xoá phiên, xoá dữ liệu.
async fn finish(state: &AppState, user_id: &str) -> Result<Purged, AppError> {
    let devices = sessions::user_devices(&state.db, user_id).await?;
    let ids: Vec<String> = devices.iter().map(|(id, _)| id.clone()).collect();
    let left = revoke_all(state, devices.into_iter().map(|(_, sealed)| sealed).collect()).await;
    // Id thiết bị (băm token): lần làm mới sau của iPhone nhận 410 và xoá đoạn còn trên máy. Bản mã hoá của grant
    // chưa thu hồi được: vòng retention thử lại (thiết bị bị xoá ngay sau đây).
    state
        .db
        .collection::<Document>(TOMBSTONES)
        .update_one(doc! { "_id": user_id }, doc! { "$addToSet": { "devices": { "$each": ids }, "revokes": { "$each": left } } })
        .await
        .map_err(|e| AppError::Internal(format!("ghi thiết bị của tài khoản đã xoá: {e}")))?;
    let removed = sessions::delete_by_user(&state.db, user_id, None).await?;
    state.session_cache.forget(&removed);
    purge_user(&state.db, &state.config.data_dir, user_id).await
}

/// Thiết bị `device_id` (băm token thiết bị) thuộc một tài khoản vừa bị xoá (trong [`TOMBSTONE_DAYS`] ngày).
pub async fn device_of_deleted_account(db: &Database, device_id: &str) -> Result<bool, AppError> {
    db.collection::<Document>(TOMBSTONES)
        .count_documents(doc! { "devices": device_id })
        .await
        .map(|n| n > 0)
        .map_err(|e| AppError::Internal(format!("tra thiết bị của tài khoản đã xoá: {e}")))
}

/// Làm tiếp việc xoá đang dở (đăng nhập vào tài khoản có `deletingAt`), trong task riêng.
pub fn resume(state: &AppState, user_id: &str) {
    let (state, user_id) = (state.clone(), user_id.to_string());
    tokio::spawn(async move {
        if let Err(e) = finish(&state, &user_id).await {
            tracing::warn!("làm tiếp việc xoá tài khoản lỗi (vòng retention thử lại): {e:?}");
        }
    });
}

/// Vòng retention: thu hồi lại grant IdP còn sót và dọn phần việc nền ghi thêm sau khi tài khoản bị xoá. Trả số
/// doc/tệp đã dọn.
pub async fn sweep_deleted(state: &AppState) -> u64 {
    let coll = state.db.collection::<Document>(TOMBSTONES);
    let tombstones: Vec<Document> = match coll.find(doc! {}).await {
        Ok(cur) => cur.filter_map(|d| async move { d.ok() }).collect().await,
        Err(e) => {
            tracing::warn!("đọc tài khoản đã xoá lỗi: {e}");
            return 0;
        }
    };
    let mut cleaned = 0;
    for t in tombstones {
        let Ok(id) = t.get_str("_id") else { continue };
        let sealed: Vec<String> =
            t.get_array("revokes").map(|a| a.iter().filter_map(|b| b.as_str().map(str::to_string)).collect()).unwrap_or_default();
        if !sealed.is_empty() {
            let left = revoke_all(state, sealed).await;
            if let Err(e) = coll.update_one(doc! { "_id": id }, doc! { "$set": { "revokes": left } }).await {
                tracing::warn!("ghi thiết bị chưa thu hồi lỗi: {e}");
            }
        }
        match purge_user(&state.db, &state.config.data_dir, id).await {
            Ok(p) => cleaned += p.files + p.docs,
            Err(e) => tracing::warn!("dọn lại tài khoản đã xoá lỗi (thử lại vòng sau): {e:?}"),
        }
    }
    cleaned
}

/// Trả lời `status` không body, không cache.
fn no_store(status: StatusCode) -> Response {
    let mut res = status.into_response();
    res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

/// Body có khai `Content-Type: application/secevent+jwt` không (bỏ qua tham số và hoa thường).
fn is_set_media_type(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case(SET_MEDIA_TYPE))
}

/// `POST /api/idp/account-deleted` — IdP báo tài khoản `sub` đã xoá (body là SET ký ES256, server gọi server, không
/// phiên). Trả 202 khi đã ghi việc xoá vào hàng đợi bền (bước (1), rồi làm luôn (2)–(3)) hoặc không có tài khoản
/// nào mang `sub` đó (chưa từng đăng nhập, đã xoá: IdP gửi lại thì không làm gì); 400 khi body không phải SET; 401
/// khi SET không hợp lệ cho URL này (alg, chữ ký, iss, aud, exp, sự kiện); 503 khi chưa tải được khoá IdP hoặc ghi
/// việc xoá lỗi, để IdP thử lại. Log không có SET hay `sub`, chỉ id user và kết quả.
pub async fn idp_account_deleted(State(state): State<AppState>, headers: HeaderMap, body: String) -> Response {
    if !is_set_media_type(&headers) {
        return no_store(StatusCode::BAD_REQUEST);
    }
    let audience = format!("{}{ACCOUNT_DELETED_PATH}", state.config.public_url);
    let sub = match state.oidc.verify_account_deleted(&state.http, body.trim(), &audience).await {
        Ok(sub) => sub,
        Err(SetError::Malformed) => return no_store(StatusCode::BAD_REQUEST),
        Err(SetError::Invalid) => return no_store(StatusCode::UNAUTHORIZED),
        Err(SetError::Unavailable) => {
            tracing::warn!("thông báo xoá tài khoản: chưa tải được khoá IdP");
            return no_store(StatusCode::SERVICE_UNAVAILABLE);
        }
    };
    let user_id = match user::find_id_by_sub(&state.db, &sub).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            tracing::info!("thông báo xoá tài khoản: không có tài khoản ở Scribe");
            return no_store(StatusCode::ACCEPTED);
        }
        Err(e) => {
            tracing::error!("thông báo xoá tài khoản: tra tài khoản lỗi: {e:?}");
            return no_store(StatusCode::SERVICE_UNAVAILABLE);
        }
    };
    if let Err(e) = mark(&state.db, &user_id).await {
        tracing::error!(user = %user_id, "thông báo xoá tài khoản: ghi dấu xoá lỗi: {e:?}");
        return no_store(StatusCode::SERVICE_UNAVAILABLE);
    }

    // Từ đây việc xoá đã nằm trong hàng đợi bền (`deleted_users`): lỗi chỉ để vòng retention làm nốt. Task riêng:
    // IdP ngắt kết nối không làm dừng việc xoá.
    let task = {
        let (state, user_id) = (state.clone(), user_id.clone());
        tokio::spawn(async move { finish(&state, &user_id).await })
    };
    let purged = match task.await {
        Ok(Ok(p)) => Some(p),
        Ok(Err(e)) => {
            tracing::error!(user = %user_id, "xoá tài khoản dở dang (vòng retention làm nốt): {e:?}");
            None
        }
        Err(e) => {
            tracing::error!(user = %user_id, "task xoá tài khoản dừng bất thường (vòng retention làm nốt): {e}");
            None
        }
    };
    let complete = purged.as_ref().is_some_and(|p| p.failed == 0);
    let purged = purged.unwrap_or_default();
    let details = doc! {
        "source": "idp",
        "files": purged.files as i64,
        "bytes": purged.bytes as i64,
        "docs": purged.docs as i64,
        "failed": purged.failed as i64,
        "complete": complete,
    };
    audit::record(&state, Some(&user_id), "account.delete", details);
    tracing::info!(user = %user_id, complete, "thông báo xoá tài khoản: đã xoá");
    no_store(StatusCode::ACCEPTED)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::file::{FileDoc, Owner};
    use serde_json::{json, Value};

    /// Server thật (router đầy đủ) với IdP giả; trả `(base, idp)`.
    async fn notice_server(db: Database, issuer: Option<&str>) -> (String, crate::oidc::tests::FakeIdp) {
        let idp = crate::oidc::tests::fake_idp().await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base = format!("http://{}", listener.local_addr().expect("addr"));
        let config = crate::config::Config::for_tests(&base, issuer.unwrap_or(&idp.issuer));
        let app = crate::routes::build_router(AppState::for_tests(config, db));
        tokio::spawn(async move { axum::serve(listener, app).await });
        (base, idp)
    }

    /// Database không kết nối được (chọn server hỏng nhanh).
    async fn unreachable_db() -> Database {
        mongodb::Client::with_uri_str("mongodb://127.0.0.1:9/?serverSelectionTimeoutMS=200&connectTimeoutMS=200")
            .await
            .expect("uri")
            .database("scribe_account_test")
    }

    /// Claim hợp lệ của SET cho receiver ở `base`, ghi đè bằng `extra`.
    fn set_claims(issuer: &str, base: &str, sub: &str, extra: Value) -> Value {
        let now = crate::oidc::tests::now();
        let mut c = json!({
            "iss": issuer, "aud": format!("{base}{ACCOUNT_DELETED_PATH}"), "sub": sub, "jti": "j1", "iat": now,
            "exp": now + 600, "toe": now, "events": { crate::oidc::ACCOUNT_DELETED_EVENT: {} },
        });
        if let (Some(c), Some(extra)) = (c.as_object_mut(), extra.as_object()) {
            c.extend(extra.clone());
        }
        c
    }

    async fn post_notice(base: &str, body: &str, content_type: &str) -> reqwest::Response {
        reqwest::Client::new()
            .post(format!("{base}{ACCOUNT_DELETED_PATH}"))
            .header("content-type", content_type)
            .body(body.to_string())
            .send()
            .await
            .expect("gửi")
    }

    #[tokio::test]
    async fn notices_that_are_not_a_valid_set_for_this_url_are_refused() {
        use crate::oidc::tests::{now, sign};
        let (base, idp) = notice_server(unreachable_db().await, None).await;
        let set = |extra: Value| sign(&idp, Some("secevent+jwt"), "k1", set_claims(&idp.issuer, &base, "acc_1", extra));
        let status = |body: String, ct: &'static str| {
            let base = base.clone();
            async move { post_notice(&base, &body, ct).await.status().as_u16() }
        };
        // Không phải SET: 400.
        assert_eq!(status(set(json!({})), "application/json").await, 400, "sai media type");
        assert_eq!(status("không phải token".into(), SET_MEDIA_TYPE).await, 400, "không phải JWS");
        let logout = sign(&idp, Some("logout+jwt"), "k1", set_claims(&idp.issuer, &base, "acc_1", json!({})));
        assert_eq!(status(logout, SET_MEDIA_TYPE).await, 400, "typ khác");
        // SET không hợp lệ cho URL này: 401.
        let mut hs = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
        hs.typ = Some("secevent+jwt".into());
        hs.kid = Some("k1".into());
        let hmac = jsonwebtoken::encode(&hs, &set_claims(&idp.issuer, &base, "acc_1", json!({})), &jsonwebtoken::EncodingKey::from_secret(b"s"))
            .expect("ký");
        assert_eq!(status(hmac, SET_MEDIA_TYPE).await, 401, "thuật toán khác");
        let unknown_kid = sign(&idp, Some("secevent+jwt"), "k9", set_claims(&idp.issuer, &base, "acc_1", json!({})));
        assert_eq!(status(unknown_kid, SET_MEDIA_TYPE).await, 401, "khoá lạ");
        let cases = [
            ("iss khác", json!({ "iss": "https://evil.example" })),
            ("aud của app khác", json!({ "aud": "https://kb.hoandat.com/api/idp/account-deleted" })),
            ("aud là mảng", json!({ "aud": [format!("{base}{ACCOUNT_DELETED_PATH}")] })),
            ("hết hạn", json!({ "exp": now() - 120 })),
            ("thiếu exp", json!({ "exp": null })),
            ("sự kiện khác", json!({ "events": { "http://schemas.openid.net/event/backchannel-logout": {} } })),
            ("sub rỗng", json!({ "sub": "" })),
            ("iat ở tương lai", json!({ "iat": now() + 3600 })),
        ];
        for (name, extra) in cases {
            assert_eq!(status(set(extra), SET_MEDIA_TYPE).await, 401, "{name}");
        }
        // SET hợp lệ nhưng không đọc được database: 503 để IdP gửi lại.
        let res = post_notice(&base, &set(json!({})), "application/secevent+jwt; charset=utf-8").await;
        assert_eq!(res.status(), 503);
        assert_eq!(res.headers().get("cache-control").and_then(|v| v.to_str().ok()), Some("no-store"));
    }

    #[tokio::test]
    async fn notices_wait_for_the_idp_keys() {
        // IdP không trả lời: không kiểm được chữ ký, 503 (IdP thử lại), không phải 401.
        let (base, idp) = notice_server(unreachable_db().await, Some("http://127.0.0.1:9")).await;
        let token = crate::oidc::tests::sign(&idp, Some("secevent+jwt"), "k1", set_claims("http://127.0.0.1:9", &base, "acc_1", json!({})));
        assert_eq!(post_notice(&base, &token, SET_MEDIA_TYPE).await.status(), 503);
    }

    /// Trên Mongo thật: `TRANSCRIPT_TEST_MONGO_URI=… TRANSCRIPT_TEST_DB_NAME=… cargo test
    /// account_deleted_notice_on_real_mongo -- --ignored`. SET hợp lệ cho một user: 202, user và dữ liệu không còn;
    /// gửi lại và `sub` lạ cũng 202.
    #[tokio::test]
    #[ignore]
    async fn account_deleted_notice_on_real_mongo() {
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        let (base, idp) = notice_server(db.clone(), None).await;
        let (a, b) = (ObjectId::new(), ObjectId::new());
        let (sub_a, sub_b) = (format!("selftest-purge-{}", a.to_hex()), format!("selftest-purge-{}", b.to_hex()));
        for (oid, sub) in [(a, &sub_a), (b, &sub_b)] {
            db.collection::<Document>("users")
                .insert_one(doc! { "_id": oid, "tenantId": crate::tenant::PRIMARY_ID, "idpSub": sub, "email": format!("{sub}@example.com") })
                .await
                .expect("user");
            db.collection::<Document>("sessions")
                .insert_one(doc! { "_id": uuid::Uuid::new_v4().to_string(), "userId": oid.to_hex(), "client": "web" })
                .await
                .expect("phiên");
        }
        let token = crate::oidc::tests::sign(&idp, Some("secevent+jwt"), "k1", set_claims(&idp.issuer, &base, &sub_a, json!({})));
        assert_eq!(post_notice(&base, &token, SET_MEDIA_TYPE).await.status(), 202);
        let count = |coll: &'static str, filter: Document| {
            let db = db.clone();
            async move { db.collection::<Document>(coll).count_documents(filter).await.expect(coll) }
        };
        assert_eq!(count("users", doc! { "_id": a }).await, 0, "user A đã xoá");
        assert_eq!(count("sessions", doc! { "userId": a.to_hex() }).await, 0, "phiên của A đã xoá");
        assert_eq!(count(TOMBSTONES, doc! { "_id": a.to_hex() }).await, 1, "A nằm trong hàng đợi dọn lại");
        assert_eq!(count("users", doc! { "_id": b }).await, 1, "B nguyên");
        assert_eq!(count("sessions", doc! { "userId": b.to_hex() }).await, 1, "phiên của B nguyên");
        // Gửi lại (IdP thử lại) và `sub` không có ở Scribe: 202, không làm gì.
        assert_eq!(post_notice(&base, &token, SET_MEDIA_TYPE).await.status(), 202);
        let stranger = crate::oidc::tests::sign(&idp, Some("secevent+jwt"), "k1", set_claims(&idp.issuer, &base, "sub-chua-tung-dang-nhap", json!({})));
        assert_eq!(post_notice(&base, &stranger, SET_MEDIA_TYPE).await.status(), 202);

        // Dọn.
        purge_user(&db, "/nonexistent", &b.to_hex()).await.expect("dọn B");
        db.collection::<Document>(TOMBSTONES).delete_one(doc! { "_id": a.to_hex() }).await.expect("dọn dấu xoá");
        db.collection::<Document>("audit_events")
            .delete_many(doc! { "userId": { "$in": [a.to_hex(), b.to_hex()] } })
            .await
            .expect("dọn audit");
    }

    #[test]
    fn ledger_prefixes_only_reach_the_users_own_docs() {
        let user = "65f0a1b2c3d4e5f60718293a";
        let other = "65f0a1b2c3d4e5f60718293b";
        let matches = |coll: &str, id: &str| {
            let (_, prefixes) = ledger_prefixes(user).into_iter().find(|(c, _)| *c == coll).expect("collection");
            prefixes.iter().any(|p| id.starts_with(p.as_str()))
        };
        assert!(matches("transcribe_usage", &format!("u:{user}:2026-10")));
        assert!(matches("transcribe_usage", &format!("d:{user}:2026-10-19")));
        assert!(matches("transcribe_usage", &format!("b:{user}:2026-10")));
        assert!(!matches("transcribe_usage", "free:2026-10"));
        assert!(!matches("transcribe_usage", "all:2026-10"));
        assert!(!matches("transcribe_usage", &format!("u:{other}:2026-10")));
        assert!(matches("summary_usage", &format!("{user}:2026-10-19")));
        assert!(matches("summary_usage", &format!("cost:{user}:2026-10")));
        assert!(matches("summary_usage", &format!("ask:{user}:2026-10-19")));
        assert!(matches("summary_usage", &format!("tr:{user}:2026-10-19")));
        assert!(!matches("summary_usage", &format!("cost:{other}:2026-10")));
        assert!(matches("transcribe_marks", &format!("hit:daily:{user}:2026-10-19")));
        // Mỗi tiền tố thành một regex `^…` riêng (dùng được index `_id`).
        let filter = prefix_filter(&[format!("u:{user}:"), format!("d:{user}:")]);
        let any = filter.get_document("_id").expect("_id").get_array("$in").expect("$in");
        let patterns: Vec<String> = any
            .iter()
            .map(|b| match b {
                Bson::RegularExpression(r) => r.pattern.clone(),
                other => panic!("không phải regex: {other:?}"),
            })
            .collect();
        assert_eq!(patterns, [format!("^u:{user}:"), format!("^d:{user}:")]);
    }

    #[test]
    fn only_files_inside_the_users_folder_are_removed() {
        let user = "65f0a1b2c3d4e5f60718293a";
        let inside = |p: &str| inside_user_dir(p, "/data", user);
        assert!(inside(&format!("/data/{user}/recordings/a.m4a")));
        assert!(inside(&format!("/data/{user}/outputs/j1/p1.webm")));
        assert!(!inside(&format!("/data/{user}")));
        assert!(!inside(&format!("/data/{user}/../other/a.m4a")));
        assert!(!inside("/data/65f0a1b2c3d4e5f60718293b/recordings/a.m4a"));
        assert!(!inside(&format!("/data/{user}x/a.m4a")));
        assert!(!inside("/etc/passwd"));
    }

    #[tokio::test]
    async fn purge_refuses_an_id_that_is_not_an_object_id() {
        // Không cần Mongo: id bị từ chối trước mọi truy vấn và mọi thao tác đĩa.
        let client = mongodb::Client::with_uri_str("mongodb://127.0.0.1:1/?serverSelectionTimeoutMS=50").await.expect("client");
        let db = client.database("unused");
        for bad in ["../etc", "", "deleted", "65f0a1b2c3d4e5f60718293a/.."] {
            assert!(matches!(purge_user(&db, "/nonexistent", bad).await, Err(AppError::BadRequest(_))), "{bad}");
        }
    }

    /// Trên Mongo thật (DB dev): `TRANSCRIPT_TEST_MONGO_URI=… TRANSCRIPT_TEST_DB_NAME=… cargo test
    /// purge_user_on_real_mongo -- --ignored`. Hai user giả với dữ liệu ở mọi collection: xoá user A thì A không
    /// còn gì (tệp trên đĩa, doc, sổ riêng), sổ chi tiết và audit của A không còn dấu vết; user B nguyên vẹn.
    #[tokio::test]
    #[ignore]
    async fn purge_user_on_real_mongo() {
        // Câu giả của transcript "running" bị đối soát index coi là thừa và xoá nếu chạy cùng lúc.
        let _sweep = crate::db::WHOLE_DB_SWEEP.lock().await;
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        let data_dir = std::env::temp_dir().join(format!("scribe-purge-{}", uuid::Uuid::new_v4()));
        let data_dir = data_dir.to_str().expect("utf-8").to_string();
        let month = "2099-01";
        // Lần chạy trước dừng giữa chừng: dọn user giả còn sót trước.
        let leftovers = db
            .collection::<Document>("users")
            .distinct("_id", doc! { "idpSub": { "$regex": "^(sub-|selftest-purge-)[0-9a-f]{24}$" } })
            .await
            .expect("tìm user giả cũ");
        let leftover_ids: Vec<String> = leftovers.iter().filter_map(|b| b.as_object_id()).map(|o| o.to_hex()).collect();
        for id in &leftover_ids {
            purge_user(&db, &data_dir, id).await.expect("dọn user giả cũ");
        }
        db.collection::<Document>("audit_events")
            .delete_many(doc! { "userId": { "$in": &leftover_ids } })
            .await
            .expect("dọn audit giả cũ");
        for coll in RUN_LEDGERS {
            db.collection::<Document>(coll).delete_many(doc! { "month": month }).await.expect("dọn sổ cũ");
        }
        let (a, b) = (ObjectId::new(), ObjectId::new());
        let (a_id, b_id) = (a.to_hex(), b.to_hex());
        let session = format!("selftest-{}", &uuid::Uuid::new_v4().simple().to_string()[..12]);

        for (oid, id) in [(a, &a_id), (b, &b_id)] {
            let dir = format!("{data_dir}/{id}/recordings");
            tokio::fs::create_dir_all(&dir).await.expect("thư mục");
            tokio::fs::create_dir_all(format!("{data_dir}/{id}/tmp")).await.expect("tmp");
            tokio::fs::write(format!("{data_dir}/{id}/tmp/x.part"), b"part").await.expect(".part");
            let owner = Owner { user_id: id };
            let path = format!("{dir}/seg.m4a");
            tokio::fs::write(&path, b"audio").await.expect("tệp");
            let rec = FileDoc::new_recording(owner, &format!("{session}#0"), &path, 5, 60.0, false, ObjectId::new());
            let file_id = rec.id.expect("id");
            file::create(&db, rec).await.expect("tạo file");
            let key = format!("s:{session}");
            let insert = |coll: &'static str, d: Document| {
                let db = db.clone();
                async move { db.collection::<Document>(coll).insert_one(d).await.expect(coll) }
            };
            db.collection::<Document>("users")
                .insert_one(doc! { "_id": oid, "idpSub": format!("selftest-purge-{id}"), "email": format!("{id}@example.com"), "name": "Thử" })
                .await
                .expect("user");
            insert("transcripts", doc! {
                "_id": file_id, "userId": id, "status": "running", "progress": 0.0, "runId": "r1", "model": "scribe_v2",
                "updatedAt": DateTime::now(), "open": false, "settled": true,
            }).await;
            insert("transcript_docs", doc! { "userId": id, "key": &key, "title": "Họp", "createdAt": DateTime::now() }).await;
            insert("transcript_translations", doc! { "userId": id, "key": &key, "lang": "en", "status": "done" }).await;
            insert("transcript_sentences", doc! { "userId": id, "key": &key, "fileId": file_id, "idx": 0, "text": "Chào", "norm": "chao" }).await;
            insert("jobs", doc! { "userId": id, "status": "queued", "createdAt": DateTime::now() }).await;
            insert("upload_sessions", doc! { "_id": uuid::Uuid::new_v4().to_string(), "userId": id, "createdAt": DateTime::now() }).await;
            insert("recording_sessions", doc! { "_id": uuid::Uuid::new_v4().to_string(), "userId": id, "createdAt": DateTime::now() }).await;
            insert("realtime_tokens", doc! {
                "_id": uuid::Uuid::new_v4().to_string(), "userId": id, "month": month, "sessionId": &session,
                "chargedS": 600_i64, "settled": false,
            }).await;
            insert("user_keys", doc! { "userId": id, "provider": "openai", "ciphertext": "x" }).await;
            insert("user_key_prefs", doc! { "_id": id, "text": "openai" }).await;
            insert("chat_link_tokens", doc! { "_id": uuid::Uuid::new_v4().to_string(), "userId": id }).await;
            insert("sessions", doc! { "_id": uuid::Uuid::new_v4().to_string(), "userId": id, "client": "web" }).await;
            insert("devices", doc! { "_id": uuid::Uuid::new_v4().to_string(), "userId": id, "refresh": "sealed" }).await;
            insert("transcribe_usage", doc! { "_id": format!("u:{id}:{month}"), "seconds": 60_i64 }).await;
            insert("transcribe_usage", doc! { "_id": format!("d:{id}:{month}-01"), "seconds": 60_i64 }).await;
            insert("summary_usage", doc! { "_id": format!("cost:{id}:{month}"), "seconds": 3_i64 }).await;
            insert("summary_usage", doc! { "_id": format!("ask:{id}:{month}-01"), "count": 1_i64 }).await;
            insert("transcribe_marks", doc! { "_id": format!("hit:daily:{id}:{month}-01"), "count": 1_i64 }).await;
            insert("transcribe_runs", doc! { "userId": id, "fileId": file_id, "month": month, "sentS": 60_i64 }).await;
            insert("summary_runs", doc! { "userId": id, "key": &key, "month": month, "costS": 3_i64 }).await;
            insert("audit_events", doc! { "userId": id, "action": "file.delete", "name": "Họp riêng.m4a", "at": DateTime::now() }).await;
            insert("audit_events", doc! { "userId": id, "action": "chat.link", "chatId": 42_i64, "at": DateTime::now() }).await;
            insert("audit_events", doc! { "userId": id, "action": "job.failed", "error": "lỗi ở Họp riêng.m4a", "at": DateTime::now() }).await;
        }

        // Dấu xoá: hồ sơ coi tài khoản như không còn (đăng nhập và phiên bị chặn), B không bị ảnh hưởng.
        mark(&db, &a_id).await.expect("đánh dấu A");
        assert!(user::find_profile(&db, &a_id).await.expect("đọc").expect("còn doc").deleting);
        assert!(!user::find_profile(&db, &b_id).await.expect("đọc").expect("còn doc").deleting);
        let purged = purge_user(&db, &data_dir, &a_id).await.expect("xoá A");
        assert_eq!((purged.files, purged.bytes, purged.failed), (1, 5, 0));

        let count = |coll: &'static str, filter: Document| {
            let db = db.clone();
            async move { db.collection::<Document>(coll).count_documents(filter).await.expect(coll) }
        };
        for (id, left) in [(&a_id, 0), (&b_id, 1)] {
            for coll in BY_USER {
                assert_eq!(count(coll, doc! { "userId": id }).await, left, "{coll} của {id}");
            }
            assert_eq!(count("user_key_prefs", doc! { "_id": id }).await, left, "prefs {id}");
            for (coll, prefixes) in ledger_prefixes(id) {
                let expected = if coll == "transcribe_marks" { left } else { 2 * left };
                assert_eq!(count(coll, prefix_filter(&prefixes)).await, expected, "{coll} của {id}");
            }
            for coll in RUN_LEDGERS.iter().take(2).chain(["realtime_tokens"].iter()) {
                assert_eq!(count(coll, doc! { "userId": id }).await, left, "{coll} của {id}");
            }
            assert_eq!(tokio::fs::metadata(format!("{data_dir}/{id}")).await.is_ok(), left == 1, "thư mục {id}");
        }
        assert_eq!(count("users", doc! { "_id": a }).await, 0);
        assert_eq!(count("users", doc! { "_id": b }).await, 1);
        // Sổ chi tiết của A còn số tiền, không còn id file hay phiên.
        assert_eq!(count("transcribe_runs", doc! { "userId": DELETED_USER, "month": month, "fileId": { "$exists": false } }).await, 1);
        assert_eq!(count("summary_runs", doc! { "userId": DELETED_USER, "month": month, "key": { "$exists": false } }).await, 1);
        // Token chép trực tiếp chưa quyết toán của A: còn (đối soát tính trọn phần giữ chỗ), không còn phiên.
        assert_eq!(
            count("realtime_tokens", doc! { "userId": DELETED_USER, "month": month, "settled": false, "sessionId": { "$exists": false } }).await,
            1
        );
        // Audit của A còn hành động, không còn tên tệp; của B nguyên.
        assert_eq!(count("audit_events", doc! { "userId": &a_id }).await, 3);
        for field in AUDIT_PERSONAL_FIELDS {
            assert_eq!(count("audit_events", doc! { "userId": &a_id, field: { "$exists": true } }).await, 0, "{field}");
        }
        assert_eq!(count("audit_events", doc! { "userId": &b_id, "chatId": 42_i64 }).await, 1);
        assert_eq!(count("audit_events", doc! { "userId": &b_id, "name": "Họp riêng.m4a" }).await, 1);
        // Chạy lại: không còn gì để xoá.
        assert_eq!(purge_user(&db, &data_dir, &a_id).await.expect("lần hai"), Purged::default());

        // Dọn.
        purge_user(&db, &data_dir, &b_id).await.expect("dọn B");
        for coll in RUN_LEDGERS {
            db.collection::<Document>(coll).delete_many(doc! { "month": month }).await.expect("dọn sổ");
        }
        db.collection::<Document>("audit_events")
            .delete_many(doc! { "userId": { "$in": [&a_id, &b_id] } })
            .await
            .expect("dọn audit");
        let _ = tokio::fs::remove_dir_all(&data_dir).await;
    }
}
