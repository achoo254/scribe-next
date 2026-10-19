//! Xoá tài khoản Scribe ngay trong app (`DELETE /api/me`, App Store guideline 5.1.1(v)).
//!
//! Tài khoản hoandat (auth.hoandat.com) là của IdP, không xoá ở đây: Scribe xoá mọi dữ liệu của chính nó.
//!
//! **Xoá hẳn**: tệp trên đĩa (cả thư mục `{data_dir}/<userId>`) và doc `files`; lượt chép (`transcripts`, lượt
//! còn chạy được quyết toán như khi xoá file); `transcript_docs` (tiêu đề, câu sửa, biên bản, dấu mốc, tên người
//! nói); bản dịch; index tìm kiếm; job cắt; phiên tải lên/ghi dở; token chép trực tiếp; Key riêng và lựa chọn;
//! token liên kết chat; phiên đăng nhập và thiết bị (grant ở IdP được thu hồi trước); sổ dùng riêng của user
//! (`u:`/`d:`/`b:` ở `transcribe_usage`, lượt ngày và chi phí tháng ở `summary_usage`, dấu đếm lần chạm giới
//! hạn ngày); doc `users` (email, tên, gói, liên kết chat).
//!
//! **Giữ, không còn gắn với người**: sổ chi tiết `transcribe_runs`, `summary_runs`, `ask_runs`, `translate_runs`
//! đổi `userId` thành [`DELETED_USER`] và bỏ id file/phiên — chi phí đã chi vẫn nằm trong trần tổng tháng và lệnh
//! đối soát (`admin usage --recompute`) vẫn cộng đúng. Sổ chung (`free:`, `all:`, `el:`) không có gì của user.
//! `audit_events` giữ tới hết hạn TTL 180 ngày (nhật ký bảo mật, chống lạm dụng): `userId` chỉ còn là một id
//! không trỏ tới ai, bỏ các trường có nội dung của người dùng (tên tệp, email, tiêu đề).
//!
//! **Việc đang chạy**: đang ghi trên trình duyệt thì từ chối (409 `RECORDING_OPEN`). Lượt chép, tóm tắt, dịch,
//! dựng index hay job cắt đang chạy có thể ghi lại vài doc sau khi xoá (vd lượt chép xong đặt tên người nói bằng
//! upsert). Vì vậy trước khi xoá, user được ghi vào `deleted_users` (chỉ id, TTL [`TOMBSTONE_DAYS`] ngày); vòng
//! retention mỗi giờ chạy lại [`purge_user`] cho các id này. Phiên bị thu hồi trước khi xoá nên không request
//! mới nào của user tới được nữa.

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
use mongodb::bson::{doc, oid::ObjectId, DateTime, Document};
use mongodb::options::IndexOptions;
use mongodb::{Database, IndexModel};
use serde::Deserialize;
use serde_json::{json, Value};
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
const BY_USER: [&str; 13] = [
    "transcript_docs",
    "transcript_translations",
    "transcript_sentences",
    "jobs",
    "upload_sessions",
    "recording_sessions",
    "realtime_tokens",
    "user_keys",
    "chat_link_tokens",
    "sessions",
    "devices",
    // Phần còn sót sau `file::delete_doc` (doc file không còn trên đĩa vẫn bị xoá ở bước trước).
    "files",
    "transcripts",
];

/// Sổ chi tiết: giữ số tiền, bỏ dấu vết người dùng.
const RUN_LEDGERS: [&str; 4] = ["transcribe_runs", "summary_runs", "ask_runs", "translate_runs"];

/// Trường của audit có nội dung do người dùng đặt (tên tệp, tiêu đề) hay định danh (email).
const AUDIT_PERSONAL_FIELDS: [&str; 3] = ["name", "email", "title"];

/// TTL của `deleted_users`. Gọi lúc khởi động.
pub async fn ensure_indexes(db: &Database) -> Result<(), String> {
    db.collection::<Document>(TOMBSTONES)
        .create_index(
            IndexModel::builder()
                .keys(doc! { "at": 1 })
                .options(IndexOptions::builder().expire_after(Duration::from_secs(TOMBSTONE_DAYS * 86_400)).build())
                .build(),
        )
        .await
        .map(|_| ())
        .map_err(|e| format!("index deleted_users.at (TTL): {e}"))
}

/// Chữ xác nhận người dùng gõ có đúng "XOÁ" không (có dấu hay không, hoa hay thường).
pub fn confirm_matches(text: &str) -> bool {
    crate::search::norm::normalize(text) == CONFIRM_WORD
}

/// `_id` của các sổ dùng riêng của user (regex, `user_id` là 24 hex nên ghép thẳng được).
fn ledger_patterns(user_id: &str) -> [(&'static str, String); 3] {
    [
        // Sổ phút tháng, sổ ngày gói free, sổ phút bằng key riêng.
        ("transcribe_usage", format!("^(u|d|b):{user_id}:")),
        // Lượt ngày tóm tắt / hỏi đáp / dịch, chi phí tháng.
        ("summary_usage", format!("^((ask|tr|cost):)?{user_id}:")),
        // Đếm lần chạm giới hạn ngày.
        ("transcribe_marks", format!("^hit:daily:{user_id}:")),
    ]
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
    for f in &files {
        match tokio::fs::remove_file(&f.path).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                tracing::warn!(path = %f.path, "xoá tài khoản: không xoá được tệp, giữ doc để thử lại: {e}");
                purged.failed += 1;
                continue;
            }
        }
        file::delete_doc(db, f).await?;
        purged.files += 1;
        purged.bytes += f.size.max(0) as u64;
    }
    // Lượt chép không còn file (hiếm): quyết toán rồi xoá.
    purged.docs += transcript::delete_user(db, user_id).await?;

    // 2. Mọi collection theo `userId`. Tệp không xoá được trên đĩa thì giữ doc `files` cho vòng sau.
    for coll in BY_USER {
        let filter = if coll == "files" && purged.failed > 0 {
            doc! { "userId": user_id, "_id": { "$nin": files.iter().filter_map(|f| f.id).collect::<Vec<_>>() } }
        } else {
            doc! { "userId": user_id }
        };
        purged.docs += db.collection::<Document>(coll).delete_many(filter).await.map_err(fail(coll))?.deleted_count;
    }
    purged.docs += db
        .collection::<Document>("user_key_prefs")
        .delete_many(doc! { "_id": user_id })
        .await
        .map_err(fail("user_key_prefs"))?
        .deleted_count;
    for (coll, pattern) in ledger_patterns(user_id) {
        purged.docs += db
            .collection::<Document>(coll)
            .delete_many(doc! { "_id": { "$regex": pattern } })
            .await
            .map_err(fail(coll))?
            .deleted_count;
    }

    // 3. Sổ chi tiết và audit: giữ, bỏ dấu vết người dùng.
    for coll in RUN_LEDGERS {
        db.collection::<Document>(coll)
            .update_many(
                doc! { "userId": user_id },
                doc! { "$set": { "userId": DELETED_USER }, "$unset": { "fileId": "", "key": "", "sessionId": "", "tokenId": "" } },
            )
            .await
            .map_err(fail(coll))?;
    }
    let unset: Document = AUDIT_PERSONAL_FIELDS.iter().map(|f| (f.to_string(), "".into())).collect();
    db.collection::<Document>("audit_events")
        .update_many(doc! { "userId": user_id }, doc! { "$unset": unset })
        .await
        .map_err(fail("audit_events"))?;

    // 4. Thư mục của user (tmp, zip, đoạn cắt còn sót) khi mọi tệp đã xoá được.
    if purged.failed == 0 {
        match tokio::fs::remove_dir_all(format!("{data_dir}/{user_id}")).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                tracing::warn!("xoá tài khoản: không xoá được thư mục của user: {e}");
                purged.failed += 1;
            }
        }
    }

    // 5. Cuối cùng là user (email, tên, gói, liên kết chat): còn doc này thì lần chạy lại vẫn biết đây là ai.
    purged.docs += db.collection::<Document>("users").delete_one(doc! { "_id": oid }).await.map_err(fail("users"))?.deleted_count;
    Ok(purged)
}

/// Ghi id vào `deleted_users` (vòng retention dọn lại trong [`TOMBSTONE_DAYS`] ngày).
async fn add_tombstone(db: &Database, user_id: &str) -> Result<(), AppError> {
    db.collection::<Document>(TOMBSTONES)
        .update_one(doc! { "_id": user_id }, doc! { "$set": { "at": DateTime::now() } })
        .upsert(true)
        .await
        .map(|_| ())
        .map_err(|e| AppError::Internal(format!("ghi tài khoản đã xoá: {e}")))
}

/// Vòng retention: dọn lại phần việc nền ghi thêm sau khi tài khoản bị xoá. Trả số doc/tệp đã dọn.
pub async fn sweep_deleted(db: &Database, data_dir: &str) -> u64 {
    let ids: Vec<String> = match db.collection::<Document>(TOMBSTONES).find(doc! {}).projection(doc! { "_id": 1 }).await {
        Ok(cur) => cur
            .filter_map(|d| async move { d.ok().and_then(|d| d.get_str("_id").ok().map(str::to_string)) })
            .collect()
            .await,
        Err(e) => {
            tracing::warn!("đọc tài khoản đã xoá lỗi: {e}");
            return 0;
        }
    };
    let mut cleaned = 0;
    for id in ids {
        match purge_user(db, data_dir, &id).await {
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

/// `DELETE /api/me` — xoá tài khoản Scribe và mọi dữ liệu của nó, thu hồi phiên và thiết bị, bỏ cookie.
/// Trả `{deleted, files, bytes, complete}`; `complete: false` khi còn phần lỗi (vòng retention làm nốt).
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
    add_tombstone(&state.db, &me.0).await?;

    // Thu hồi trước khi xoá: không request mới nào của user tới được nữa, máy biến khỏi /account của IdP.
    for sealed in sessions::device_refreshes(&state.db, &me.0).await? {
        if let Some(refresh) = open(&state.session_key, &sealed) {
            state.oidc.revoke(&state.http, &state.config.oidc_mobile_client_id, &refresh).await;
        }
    }
    let removed = sessions::delete_by_user(&state.db, &me.0, None).await?;
    state.session_cache.forget(&removed);

    let (purged, complete) = match purge_user(&state.db, &state.config.data_dir, &me.0).await {
        Ok(p) => {
            let complete = p.failed == 0;
            (p, complete)
        }
        Err(e) => {
            tracing::error!("xoá tài khoản dở dang (vòng retention làm nốt): {e:?}");
            (Purged::default(), false)
        }
    };
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
    fn ledger_patterns_only_reach_the_users_own_docs() {
        let user = "65f0a1b2c3d4e5f60718293a";
        let other = "65f0a1b2c3d4e5f60718293b";
        let matches = |coll: &str, id: &str| {
            let (_, pattern) = ledger_patterns(user).into_iter().find(|(c, _)| *c == coll).expect("collection");
            // Regex của Mongo cho các mẫu này chỉ dùng `^`, nhóm và `|`: so bằng cách thử từng nhánh.
            let body = pattern.trim_start_matches('^');
            let (head, rest) = body.split_once(user).expect("có id user");
            let prefixes: Vec<String> = match head {
                "(u|d|b):" => vec!["u:".into(), "d:".into(), "b:".into()],
                "((ask|tr|cost):)?" => vec!["".into(), "ask:".into(), "tr:".into(), "cost:".into()],
                plain => vec![plain.to_string()],
            };
            prefixes.iter().any(|p| id.starts_with(&format!("{p}{user}{rest}")))
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
        assert!(!matches("summary_usage", &format!("cost:{other}:2026-10")));
        assert!(matches("transcribe_marks", &format!("hit:daily:{user}:2026-10-19")));
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
        for id in leftovers.iter().filter_map(|b| b.as_object_id()) {
            purge_user(&db, &data_dir, &id.to_hex()).await.expect("dọn user giả cũ");
        }
        db.collection::<Document>("audit_events")
            .delete_many(doc! { "action": "file.delete", "userId": { "$regex": "^[0-9a-f]{24}$" }, "name": { "$exists": false }, "fileId": { "$exists": false } })
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
            insert("realtime_tokens", doc! { "_id": uuid::Uuid::new_v4().to_string(), "userId": id, "month": month }).await;
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
        }

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
            for (coll, pattern) in ledger_patterns(id) {
                let expected = if coll == "transcribe_marks" { left } else { 2 * left };
                assert_eq!(count(coll, doc! { "_id": { "$regex": pattern } }).await, expected, "{coll} của {id}");
            }
            for coll in RUN_LEDGERS.iter().take(2) {
                assert_eq!(count(coll, doc! { "userId": id }).await, left, "{coll} của {id}");
            }
            assert_eq!(tokio::fs::metadata(format!("{data_dir}/{id}")).await.is_ok(), left == 1, "thư mục {id}");
        }
        assert_eq!(count("users", doc! { "_id": a }).await, 0);
        assert_eq!(count("users", doc! { "_id": b }).await, 1);
        // Sổ chi tiết của A còn số tiền, không còn id file hay phiên.
        assert_eq!(count("transcribe_runs", doc! { "userId": DELETED_USER, "month": month, "fileId": { "$exists": false } }).await, 1);
        assert_eq!(count("summary_runs", doc! { "userId": DELETED_USER, "month": month, "key": { "$exists": false } }).await, 1);
        // Audit của A còn hành động, không còn tên tệp; của B nguyên.
        assert_eq!(count("audit_events", doc! { "userId": &a_id, "name": { "$exists": false } }).await, 1);
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
