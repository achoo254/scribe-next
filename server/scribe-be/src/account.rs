//! Xoá tài khoản Scribe ngay trong app (`POST /api/me/delete`, cũng nhận `DELETE /api/me`; App Store guideline
//! 5.1.1(v)).
//!
//! Tài khoản hoandat (auth.hoandat.com) là của IdP, không xoá ở đây: Scribe xoá mọi dữ liệu của chính nó.
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
//! **Trình tự và việc đang chạy**: đang ghi trên trình duyệt thì từ chối (409 `RECORDING_OPEN`). Còn lại:
//! (1) `users.deletingAt` + `deleted_users` (chỉ id, TTL [`TOMBSTONE_DAYS`] ngày) — từ đây đăng nhập (web, iPhone,
//! làm mới phiên) bị từ chối và tiếp tục việc xoá, middleware phiên coi tài khoản như không còn, worker bỏ qua lượt
//! của user; (2) thu hồi grant IdP của các thiết bị (lỗi thì giữ bản mã hoá trong `deleted_users` để thử lại),
//! xoá phiên và bỏ khỏi cache; (3) [`purge_user`]. Bước (2)–(3) chạy trong task riêng: client ngắt kết nối không
//! làm dừng việc xoá; lỗi sau bước (1) trả 200 `complete: false` ("đang xoá"). Việc nền ghi thêm sau khi xoá (vd
//! lượt chép xong đặt tên người nói bằng upsert) được vòng retention mỗi giờ dọn lại qua `deleted_users`.

use crate::audit;
use crate::auth::{without_session_cookie, UserId};
use crate::error::AppError;
use crate::models::session as sessions;
use crate::models::{file, transcript, user};
use crate::session::open;
use crate::state::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::{Extension, Json};
use axum_extra::extract::cookie::CookieJar;
use futures_util::StreamExt;
use mongodb::bson::{doc, oid::ObjectId, Bson, DateTime, Document, Regex};
use mongodb::options::IndexOptions;
use mongodb::{Database, IndexModel};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Component, Path};
use std::time::Duration;

const TOMBSTONES: &str = "deleted_users";
/// Vòng retention còn dọn lại dữ liệu của tài khoản đã xoá trong chừng này ngày.
const TOMBSTONE_DAYS: u64 = 7;
/// `userId` của sổ chi tiết sau khi tài khoản bị xoá.
pub const DELETED_USER: &str = "deleted";
/// Mỗi phút một user gọi xoá tài khoản tối đa chừng này lần.
const DELETE_PER_MINUTE: u32 = 3;
/// Chữ người dùng gõ để xác nhận (so sau khi chuẩn hoá: "XOÁ", "xóa", "xoa" đều được).
const CONFIRM_WORD: &str = "xoa";

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

/// Chữ xác nhận người dùng gõ có đúng "XOÁ" không (có dấu hay không, hoa hay thường).
pub fn confirm_matches(text: &str) -> bool {
    crate::search::norm::normalize(text) == CONFIRM_WORD
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

#[derive(Deserialize)]
pub struct DeleteBody {
    /// Chữ người dùng gõ vào ô xác nhận ("XOÁ").
    confirm: String,
}

/// `POST /api/me/delete` (và `DELETE /api/me`) — xoá tài khoản Scribe và mọi dữ liệu của nó, thu hồi phiên và
/// thiết bị, bỏ cookie. Trả `{deleted, files, bytes, complete}`; `complete: false` khi phần còn lại đang chạy tiếp
/// hoặc lỗi (vòng retention làm nốt).
pub async fn delete_me(
    State(state): State<AppState>,
    Extension(me): Extension<UserId>,
    jar: CookieJar,
    Json(body): Json<DeleteBody>,
) -> Result<(CookieJar, Json<Value>), AppError> {
    if !state.rate.allow(&format!("account_delete:{}", me.0), DELETE_PER_MINUTE) {
        return Err(AppError::TooManyRequests("thao tác quá nhanh, thử lại sau một phút".into()));
    }
    if !confirm_matches(&body.confirm) {
        return Err(AppError::Coded {
            status: StatusCode::BAD_REQUEST,
            code: "CONFIRM_REQUIRED",
            message: "Gõ XOÁ để xác nhận xoá tài khoản.".into(),
        });
    }
    if crate::recordings::recording_live(&state, &me.0).await? {
        return Err(AppError::Coded {
            status: StatusCode::CONFLICT,
            code: "RECORDING_OPEN",
            message: "Đang có bản ghi chạy. Dừng ghi rồi thử lại.".into(),
        });
    }
    // Đã xoá ở lần gọi trước (bấm hai lần): phiên này cũng phải hết.
    if user::find_profile(&state.db, &me.0).await?.is_none() {
        return Err(AppError::Unauthorized("tài khoản không còn".into()));
    }
    mark(&state.db, &me.0).await?;

    // Từ đây tài khoản coi như đã xoá: lỗi chỉ làm `complete: false`. Task riêng: client ngắt kết nối không làm
    // dừng việc xoá.
    let task = {
        let (state, user_id) = (state.clone(), me.0.clone());
        tokio::spawn(async move { finish(&state, &user_id).await })
    };
    let purged = match task.await {
        Ok(Ok(p)) => Some(p),
        Ok(Err(e)) => {
            tracing::error!("xoá tài khoản dở dang (vòng retention làm nốt): {e:?}");
            None
        }
        Err(e) => {
            tracing::error!("task xoá tài khoản dừng bất thường (vòng retention làm nốt): {e}");
            None
        }
    };
    let complete = purged.as_ref().is_some_and(|p| p.failed == 0);
    let purged = purged.unwrap_or_default();
    let details = doc! {
        "files": purged.files as i64,
        "bytes": purged.bytes as i64,
        "docs": purged.docs as i64,
        "failed": purged.failed as i64,
        "complete": complete,
    };
    audit::record(&state, Some(&me.0), "account.delete", details);
    let body = json!({ "deleted": true, "files": purged.files, "bytes": purged.bytes, "complete": complete });
    Ok((without_session_cookie(&state, jar), Json(body)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::file::{FileDoc, Owner};
    use crate::models::plan::Plan;

    #[test]
    fn confirm_word_accepts_any_vietnamese_spelling() {
        for ok in ["XOÁ", "xoá", "Xóa", "xoa", " XOA "] {
            assert!(confirm_matches(ok), "{ok}");
        }
        for bad in ["", "xo", "xoá tài khoản", "delete", "XOAA"] {
            assert!(!confirm_matches(bad), "{bad}");
        }
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
            let owner = Owner { user_id: id, plan: Plan::Free };
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
