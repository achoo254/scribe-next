//! Lệnh quản trị chạy trên server (cùng binary, cấu hình nhúng lúc build):
//!
//! ```text
//! scribe-be admin set-plan <email> <free|plus> [--until YYYY-MM-DD]
//! scribe-be admin usage [--email <email>] [--month YYYY-MM] [--recompute [--apply]]
//! ```
//!
//! Chạy bằng user của service (`sudo -u <user> <binary> admin …`). Không khởi worker, sweeper hay
//! session key: chỉ nối Mongo, làm việc, in JSON, ghi `audit_events`.

use crate::config::Config;
use crate::error::AppError;
use crate::models::{plan, transcript, usage, user};
use chrono::{NaiveDate, TimeZone, Utc};
use futures_util::StreamExt;
use mongodb::bson::{doc, DateTime, Document};
use mongodb::Database;
use serde_json::{json, Value};
use std::collections::HashMap;

pub async fn run(config: &Config, db: &Database, args: &[String]) -> Result<Value, String> {
    let (command, rest) = args.split_first().ok_or_else(usage_text)?;
    match command.as_str() {
        "set-plan" => set_plan(db, rest).await,
        "usage" => show_usage(config, db, rest).await,
        _ => Err(usage_text()),
    }
}

fn usage_text() -> String {
    "dùng: scribe-be admin set-plan <email> <free|plus> [--until YYYY-MM-DD] | \
     scribe-be admin usage [--email <email>] [--month YYYY-MM] [--recompute [--apply]]"
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

/// Hết ngày `YYYY-MM-DD` theo giờ Việt Nam.
fn end_of_day(date: &str) -> Result<DateTime, String> {
    let day = NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|_| format!("ngày không hợp lệ: {date}"))?;
    let vn = chrono::FixedOffset::east_opt(7 * 3600).expect("+07:00");
    let end = day.and_hms_opt(23, 59, 59).expect("giờ hợp lệ");
    let at = vn.from_local_datetime(&end).single().ok_or("ngày không hợp lệ")?.with_timezone(&Utc);
    Ok(DateTime::from_millis(at.timestamp_millis()))
}

async fn audit(db: &Database, user_id: Option<&str>, action: &str, details: Document) {
    let mut event = doc! { "at": DateTime::now(), "action": action, "userId": user_id, "ip": "admin-cli" };
    event.extend(details);
    if let Err(e) = db.collection::<Document>("audit_events").insert_one(event).await {
        tracing::warn!("ghi audit lỗi: {e}");
    }
}

async fn set_plan(db: &Database, args: &[String]) -> Result<Value, String> {
    let (email, name) = match args {
        [email, name, ..] => (email.as_str(), name.as_str()),
        _ => return Err(usage_text()),
    };
    let chosen = plan::Plan::parse(name).ok_or_else(|| format!("gói không hợp lệ: {name}"))?;
    let until = flag(args, "--until").map(end_of_day).transpose()?;
    if chosen == plan::Plan::Free && until.is_some() {
        return Err("gói free không có hạn".into());
    }
    if until.is_some_and(|u| u < DateTime::now()) {
        return Err("hạn đã qua".into());
    }
    let user_id = one_user(db, email).await?;
    user::set_plan(db, &user_id, chosen.name(), until).await.map_err(err)?;
    let until_text = until.and_then(|u| u.try_to_rfc3339_string().ok());
    audit(db, Some(&user_id), "admin.set_plan", doc! { "plan": chosen.name(), "until": until_text.clone() }).await;
    Ok(json!({ "userId": user_id, "plan": chosen.name(), "until": until_text }))
}

async fn show_usage(config: &Config, db: &Database, args: &[String]) -> Result<Value, String> {
    let month = flag(args, "--month").map(str::to_string).unwrap_or_else(|| usage::month_key(Utc::now()));
    let user_id = match flag(args, "--email") {
        Some(email) => Some(one_user(db, email).await?),
        None => None,
    };
    let limits = config.transcribe_limits;
    let current = usage::month_usage(db, user_id.as_deref().unwrap_or("-"), &month).await.map_err(err)?;
    let mut out = json!({
        "month": month,
        "freePool": { "usedMinutes": current.free_pool_seconds / 60, "limitMinutes": limits.free_pool_seconds / 60 },
        "total": { "usedMinutes": current.total_seconds / 60, "limitMinutes": limits.monthly_cap_seconds / 60 },
    });
    if let Some(id) = &user_id {
        out["user"] = json!({ "userId": id, "usedMinutes": current.user_seconds / 60 });
    }
    if args.iter().any(|a| a == "--recompute") {
        out["recompute"] = recompute(db, &month, args.iter().any(|a| a == "--apply")).await?;
    }
    Ok(out)
}

/// Tính lại sổ tháng: lượt đã quyết toán (`transcribe_runs.sentS`) + lượt còn mở (phần đang giữ,
/// `chargedS` — quyết toán sau sẽ cộng chênh lệch). In chênh lệch; `apply` cộng chênh lệch bằng `$inc` và
/// chỉ chạy khi tháng không còn lượt nào đang mở (giữ chỗ đang dở sẽ làm số sai).
async fn recompute(db: &Database, month: &str, apply: bool) -> Result<Value, String> {
    let mut per_user: HashMap<String, i64> = HashMap::new();
    let (mut free_pool, mut total) = (0_i64, 0_i64);
    let mut add = |user: &str, free: bool, seconds: i64| {
        *per_user.entry(user.to_string()).or_default() += seconds;
        total += seconds;
        if free {
            free_pool += seconds;
        }
    };
    let read = |e: mongodb::error::Error| format!("đọc sổ: {e}");
    let mut runs = db.collection::<Document>(usage::RUNS_COLL).find(doc! { "month": month }).await.map_err(read)?;
    while let Some(r) = runs.next().await {
        let r = r.map_err(read)?;
        let seconds = r.get_i64("sentS").unwrap_or(0);
        add(r.get_str("userId").unwrap_or("-"), r.get_bool("freePool").unwrap_or(false), seconds);
    }
    let mut open_runs = 0;
    let mut open = db
        .collection::<transcript::TranscriptDoc>("transcripts")
        .find(doc! { "month": month, "$or": [{ "open": true }, { "settled": false }] })
        .await
        .map_err(read)?;
    while let Some(t) = open.next().await {
        let t = t.map_err(read)?;
        open_runs += 1;
        add(&t.user_id, t.free_pool, t.charged_s);
    }
    if apply && open_runs > 0 {
        return Err(format!("còn {open_runs} lượt đang chạy trong tháng {month}: chờ hàng đợi rỗng rồi chạy lại --apply"));
    }
    for user_id in usage::user_ids_with_ledger(db, month).await.map_err(err)? {
        per_user.entry(user_id).or_default();
    }
    let mut diffs = Vec::new();
    for (user_id, seconds) in &per_user {
        let ledger = usage::month_usage(db, user_id, month).await.map_err(err)?.user_seconds;
        if ledger != *seconds {
            diffs.push(json!({ "userId": user_id, "ledgerSeconds": ledger, "recomputedSeconds": seconds }));
            if apply {
                let (id, _, _) = usage::doc_ids(Some(user_id), month);
                usage::adjust(db, &id.unwrap_or_default(), seconds - ledger).await.map_err(err)?;
            }
        }
    }
    let current = usage::month_usage(db, "-", month).await.map_err(err)?;
    if apply {
        let (_, free_id, total_id) = usage::doc_ids(None, month);
        usage::adjust(db, &free_id, free_pool - current.free_pool_seconds).await.map_err(err)?;
        usage::adjust(db, &total_id, total - current.total_seconds).await.map_err(err)?;
        audit(db, None, "admin.usage_recompute", doc! { "month": month, "users": diffs.len() as i64 }).await;
    }
    Ok(json!({
        "applied": apply,
        "openRuns": open_runs,
        "userDiffs": diffs,
        "freePool": { "ledgerSeconds": current.free_pool_seconds, "recomputedSeconds": free_pool },
        "total": { "ledgerSeconds": current.total_seconds, "recomputedSeconds": total },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_end_dates_are_vietnam_end_of_day() {
        let at = end_of_day("2026-12-31").expect("ngày");
        assert_eq!(at.try_to_rfc3339_string().expect("rfc3339"), "2026-12-31T16:59:59Z");
        assert!(end_of_day("31/12/2026").is_err());
    }
}
