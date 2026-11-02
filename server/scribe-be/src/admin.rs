//! Lệnh quản trị chạy trên server (cùng binary, cấu hình nhúng lúc build):
//!
//! ```text
//! scribe-be admin block <email> [--reason <lý do>]
//! scribe-be admin unblock <email>
//! scribe-be admin notify-test
//! scribe-be admin shorten-retention [--apply]
//! ```
//!
//! Chạy bằng user của service (`sudo -u <user> <binary> admin …`). Không khởi worker, sweeper hay
//! session key: chỉ nối Mongo, làm việc, in JSON, ghi `audit_events`. Chủ dự án chốt 02/11/2026: bỏ `usage` và
//! `elevenlabs-quota` (sổ của key AI server) cùng key AI của server, bỏ `set-plan` cùng gói free/plus.

use crate::config::Config;
use crate::error::AppError;
use crate::models::{file, session as sessions, storage, user};
use mongodb::bson::{doc, DateTime, Document};
use mongodb::Database;
use serde_json::{json, Value};

pub async fn run(config: &Config, db: &Database, args: &[String]) -> Result<Value, String> {
    let (command, rest) = args.split_first().ok_or_else(usage_text)?;
    match command.as_str() {
        "block" => block(db, rest, true).await,
        "unblock" => block(db, rest, false).await,
        "shorten-retention" => shorten_retention(db, args.iter().any(|a| a == "--apply")).await,
        "notify-test" => {
            let http = crate::notify::bot_client(reqwest::Client::builder(), config)?;
            crate::notify::deliver(&http, config, "Thử kênh báo động của Scribe.").await?;
            Ok(json!({ "sent": true }))
        }
        _ => Err(usage_text()),
    }
}

fn usage_text() -> String {
    "dùng: scribe-be admin block <email> [--reason <lý do>] | scribe-be admin unblock <email> | \
     scribe-be admin notify-test | scribe-be admin shorten-retention [--apply]"
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
