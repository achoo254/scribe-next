//! Lệnh quản trị chạy trên server (cùng binary, cấu hình nhúng lúc build):
//!
//! ```text
//! scribe-be admin block <email> [--reason <lý do>]
//! scribe-be admin unblock <email>
//! scribe-be admin notify-test
//! scribe-be admin shorten-retention [--apply]
//! scribe-be admin busy
//! ```
//!
//! Chạy bằng user của service (`sudo -u <user> <binary> admin …`). Không khởi worker, sweeper hay
//! session key: chỉ nối Mongo, làm việc, in JSON, ghi `audit_events`. Chủ dự án chốt 02/11/2026: bỏ `usage` và
//! `elevenlabs-quota` (sổ của key AI server) cùng key AI của server, bỏ `set-plan` cùng gói free/plus.

use crate::config::Config;
use crate::error::AppError;
use crate::models::{file, session as sessions, storage, transcript, user};
use crate::tenant::PRIMARY_ID;
use mongodb::bson::{doc, DateTime, Document};
use mongodb::Database;
use serde_json::{json, Value};

/// Mã thoát của `admin busy` khi còn việc đang chạy (0 = rảnh, 2 = lỗi như mọi lệnh admin).
pub const EXIT_BUSY: i32 = 3;

/// Kết quả một lệnh admin: JSON in ra stdout và mã thoát của tiến trình.
pub struct Outcome {
    pub out: Value,
    pub exit_code: i32,
}

impl From<Value> for Outcome {
    fn from(out: Value) -> Self {
        Outcome { out, exit_code: 0 }
    }
}

pub async fn run(config: &Config, db: &Database, args: &[String]) -> Result<Outcome, String> {
    let (command, rest) = args.split_first().ok_or_else(usage_text)?;
    match command.as_str() {
        "block" => block(db, rest, true).await.map(Outcome::from),
        "unblock" => block(db, rest, false).await.map(Outcome::from),
        "shorten-retention" => shorten_retention(db, args.iter().any(|a| a == "--apply")).await.map(Outcome::from),
        "notify-test" => {
            let http = crate::notify::bot_client(reqwest::Client::builder(), config)?;
            crate::notify::deliver(&http, config, "Thử kênh báo động của Scribe.").await?;
            Ok(json!({ "sent": true }).into())
        }
        "busy" => busy(db, &config.data_dir, doc! {}).await,
        _ => Err(usage_text()),
    }
}

fn usage_text() -> String {
    "dùng: scribe-be admin block <email> [--reason <lý do>] | scribe-be admin unblock <email> | \
     scribe-be admin notify-test | scribe-be admin shorten-retention [--apply] | scribe-be admin busy"
        .into()
}

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str)
}

fn err(e: AppError) -> String {
    format!("{e:?}")
}

async fn one_user(db: &Database, email: &str) -> Result<String, String> {
    let ids = user::find_ids_by_email(db, email).await.map_err(err)?;
    match ids.as_slice() {
        [id] => Ok(id.clone()),
        [] => Err(format!("không có user nào với email {email}")),
        _ => Err(format!("{} user trùng email {email}, không đổi gì", ids.len())),
    }
}

async fn audit(db: &Database, user_id: Option<&str>, action: &str, details: Document) {
    let mut event = doc! { "at": DateTime::now(), "action": action, "userId": user_id, "ip": "admin-cli" };
    event.extend(details);
    if let Err(e) = db.collection::<Document>("audit_events").insert_one(event).await {
        tracing::warn!("ghi audit lỗi: {e}");
    }
}

/// Khoá: chặn đăng nhập và mọi việc AI, gỡ liên kết chat, xoá mọi phiên (server đang chạy còn nhớ phiên tối đa 30 giây; app
/// iPhone mất thiết bị ở lần làm mới kế tiếp). Mở khoá chỉ xoá dấu khoá.
async fn block(db: &Database, args: &[String], on: bool) -> Result<Value, String> {
    let email = args.first().ok_or_else(usage_text)?;
    let user_id = one_user(db, email).await?;
    let reason = flag(args, "--reason").unwrap_or("không ghi lý do");
    user::set_blocked(db, &user_id, on.then_some(reason)).await.map_err(err)?;
    if !on {
        audit(db, Some(&user_id), "admin.unblock", doc! {}).await;
        return Ok(json!({ "userId": user_id, "blocked": false }));
    }
    let removed = sessions::delete_by_user(db, &user_id, None).await.map_err(err)?;
    // Tài khoản bị khoá không được nhận gì qua chat nữa; mở khoá thì người dùng tự liên kết lại.
    let unlinked = crate::models::chat_link::unlink(db, &user_id, None).await.map_err(err)?;
    let details = doc! { "reason": reason, "sessions": removed.len() as i64, "chatUnlinked": unlinked };
    audit(db, Some(&user_id), "admin.block", details).await;
    if unlinked {
        audit(db, Some(&user_id), "chat.unlink", doc! { "reason": "blocked" }).await;
    }
    Ok(json!({ "userId": user_id, "blocked": true, "sessionsRemoved": removed.len(), "chatUnlinked": unlinked }))
}

/// Tệp của mọi user còn hạn dài hơn hạn chung (7 ngày; vd tệp tạo thời còn gói plus, 14 ngày): rút `expiresAt` về
/// `min(expiresAt, createdAt + 7 ngày)`. Không tự chạy lúc deploy: mặc định chỉ đếm; `--apply` mới ghi. Tệp đã quá 7
/// ngày bị sweep retention xoá ở vòng kế tiếp (tối đa một giờ), nên sao lưu trước khi `--apply`.
async fn shorten_retention(db: &Database, apply: bool) -> Result<Value, String> {
    let days = storage::LIMITS.retention_days;
    let everyone = || doc! { "$exists": true };
    let (files, users, due_now) = file::count_longer_than(db, everyone(), days).await.map_err(err)?;
    let mut out = json!({
        "applied": apply,
        "retentionDays": days,
        "users": users,
        "files": files,
        "expireOnNextSweep": due_now,
    });
    if apply {
        let changed = file::shorten_expiry(db, everyone(), days).await.map_err(err)?;
        out["changed"] = json!(changed);
        let details = doc! { "files": changed as i64, "users": users as i64, "days": days };
        audit(db, None, "admin.shorten_retention", details).await;
    }
    Ok(out)
}

/// Việc mà restart sẽ cắt ngang: phiên ghi đang sống (`recording_sessions`, cùng định nghĩa với chặn xoá tài khoản:
/// `recordings::count_live`, vừa mở hoặc vừa nhận chunk), phiên tải lên còn mở (`upload_sessions`) và lượt chép lời của
/// tenant chính còn chờ hoặc đang chạy (`queued`, `running`, cả `held` thoáng qua lúc xếp một lượt nhiều tệp). Phiên ghi
/// bỏ dở (đóng tab giữa chừng) còn doc tới khi retention dọn sau một ngày nhưng không tính, để không chặn deploy cả ngày.
/// Bước kích hoạt bản mới chờ lệnh này trả 0 trước khi thay binary; còn việc thì mã thoát [`EXIT_BUSY`]. `scope` chỉ để
/// test giới hạn vào doc của mình.
async fn busy(db: &Database, data_dir: &str, scope: Document) -> Result<Outcome, String> {
    let count = |coll: &'static str, filter: Document| {
        let mut filter = filter;
        filter.extend(scope.clone());
        async move {
            db.collection::<Document>(coll)
                .count_documents(filter)
                .await
                .map_err(|e| format!("đếm {coll}: {e}"))
        }
    };
    let recording = crate::recordings::count_live(db, data_dir, scope.clone())
        .await
        .map_err(|e| format!("đếm recording_sessions: {e}"))?;
    let upload = count("upload_sessions", doc! {}).await?;
    let queue = count("transcripts", doc! { "status": { "$in": transcript::ACTIVE.to_vec() }, "tenantId": PRIMARY_ID }).await?;
    let busy = recording + upload + queue > 0;
    Ok(Outcome {
        out: json!({ "busy": busy, "recordingSessions": recording, "uploadSessions": upload, "transcriptsActive": queue }),
        exit_code: if busy { EXIT_BUSY } else { 0 },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Trên Mongo thật (DB dev hoặc mongo của CI): `TRANSCRIPT_TEST_MONGO_URI=… TRANSCRIPT_TEST_DB_NAME=… cargo test
    /// -p scribe-be admin -- --ignored`. Chỉ đếm doc của user test này, vì test khác chạy song song cũng xếp lượt chép.
    /// `.part` của phiên ghi nằm trong thư mục tạm riêng của test.
    #[tokio::test]
    #[ignore]
    async fn busy_counts_open_sessions_and_the_primary_queue_only() {
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        let user = format!("selftest-{}", uuid::Uuid::new_v4().simple());
        let scope = doc! { "userId": &user };
        let data_dir = std::env::temp_dir().join(format!("scribe-busy-{}", uuid::Uuid::new_v4())).to_string_lossy().into_owned();
        let check = |outcome: Outcome, rec: u64, up: u64, queue: u64| {
            assert_eq!(outcome.out["recordingSessions"], json!(rec));
            assert_eq!(outcome.out["uploadSessions"], json!(up));
            assert_eq!(outcome.out["transcriptsActive"], json!(queue));
            let busy = rec + up + queue > 0;
            assert_eq!((outcome.out["busy"].as_bool(), outcome.exit_code), (Some(busy), if busy { EXIT_BUSY } else { 0 }));
        };
        check(busy(&db, &data_dir, scope.clone()).await.expect("rảnh"), 0, 0, 0);

        let transcripts = db.collection::<Document>("transcripts");
        let row = |status: &str, tenant: &str| doc! { "userId": &user, "status": status, "tenantId": tenant, "updatedAt": DateTime::now() };
        transcripts
            .insert_many([row("done", PRIMARY_ID), row("failed", PRIMARY_ID), row("queued", "ten_other")])
            .await
            .expect("lượt không tính");
        check(busy(&db, &data_dir, scope.clone()).await.expect("lượt xong và tenant khác không tính"), 0, 0, 0);
        transcripts.insert_many([row("queued", PRIMARY_ID), row("running", PRIMARY_ID)]).await.expect("lượt chạy");
        check(busy(&db, &data_dir, scope.clone()).await.expect("hàng đợi"), 0, 0, 2);

        let session = || doc! { "_id": uuid::Uuid::new_v4().to_string(), "userId": &user, "createdAt": DateTime::now() };
        let recordings = db.collection::<Document>("recording_sessions");
        // Bỏ dở: mở từ một giờ trước, `.part` không nhận chunk nào nữa ⟹ không chặn deploy.
        let hour_ago = DateTime::from_millis(DateTime::now().timestamp_millis() - 3_600_000);
        let abandoned = uuid::Uuid::new_v4().to_string();
        recordings.insert_one(doc! { "_id": &abandoned, "userId": &user, "createdAt": hour_ago }).await.expect("phiên bỏ dở");
        let tmp = format!("{data_dir}/{user}/tmp");
        std::fs::create_dir_all(&tmp).expect("thư mục tmp");
        let part = std::fs::File::create(format!("{tmp}/{abandoned}.part")).expect("part");
        part.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(3_600)).expect("mtime cũ");
        check(busy(&db, &data_dir, scope.clone()).await.expect("phiên bỏ dở không tính"), 0, 0, 2);
        // Đang ghi: mở từ lâu nhưng `.part` vừa nhận chunk; và một phiên vừa mở chưa có chunk.
        let recording = uuid::Uuid::new_v4().to_string();
        recordings.insert_one(doc! { "_id": &recording, "userId": &user, "createdAt": hour_ago }).await.expect("phiên đang ghi");
        std::fs::write(format!("{tmp}/{recording}.part"), b"chunk").expect("chunk mới");
        recordings.insert_one(session()).await.expect("phiên vừa mở");
        db.collection::<Document>("upload_sessions").insert_many([session(), session()]).await.expect("phiên tải");
        check(busy(&db, &data_dir, scope.clone()).await.expect("đủ loại"), 2, 2, 2);

        for coll in ["transcripts", "recording_sessions", "upload_sessions"] {
            db.collection::<Document>(coll).delete_many(scope.clone()).await.expect("dọn");
        }
        check(busy(&db, &data_dir, scope).await.expect("dọn xong"), 0, 0, 0);
        let _ = std::fs::remove_dir_all(&data_dir);
    }
}
