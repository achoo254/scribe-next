//! Lệnh quản trị chạy trên server (cùng binary, cấu hình nhúng lúc build):
//!
//! ```text
//! scribe-be admin set-plan <email> <free|plus> [--until YYYY-MM-DD]
//! scribe-be admin usage [--email <email>] [--month YYYY-MM] [--recompute [--apply]]
//! scribe-be admin elevenlabs-quota [--month YYYY-MM] [--clear]
//! scribe-be admin block <email> [--reason <lý do>]
//! scribe-be admin unblock <email>
//! scribe-be admin notify-test
//! scribe-be admin shorten-free-retention [--apply]
//! ```
//!
//! Chạy bằng user của service (`sudo -u <user> <binary> admin …`). Không khởi worker, sweeper hay
//! session key: chỉ nối Mongo, làm việc, in JSON, ghi `audit_events`.

use crate::config::{Config, FREE_RETENTION_DAYS};
use crate::error::AppError;
use crate::models::{file, plan, session as sessions, transcript, usage, user};
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
        "elevenlabs-quota" => elevenlabs_quota(config, db, rest).await,
        "block" => block(db, rest, true).await,
        "unblock" => block(db, rest, false).await,
        "shorten-free-retention" => shorten_free_retention(db, args.iter().any(|a| a == "--apply")).await,
        "notify-test" => {
            crate::notify::deliver(&reqwest::Client::new(), config, "Thử kênh báo động của Scribe.").await?;
            Ok(json!({ "sent": true }))
        }
        _ => Err(usage_text()),
    }
}

fn usage_text() -> String {
    "dùng: scribe-be admin set-plan <email> <free|plus> [--until YYYY-MM-DD] | \
     scribe-be admin usage [--email <email>] [--month YYYY-MM] [--recompute [--apply]] | \
     scribe-be admin elevenlabs-quota [--month YYYY-MM] [--clear] | \
     scribe-be admin block <email> [--reason <lý do>] | scribe-be admin unblock <email> | \
     scribe-be admin notify-test | scribe-be admin shorten-free-retention [--apply]"
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

/// Khoá: chặn đăng nhập và chép lời, gỡ liên kết chat, xoá mọi phiên (server đang chạy còn nhớ phiên tối đa 30 giây; app
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

/// Tệp của user gói free (mọi user không có gói plus hiệu lực) đã tạo trước khi hạn free còn 7 ngày: rút
/// `expiresAt` về `min(expiresAt, createdAt + 7 ngày)`. Mặc định chỉ đếm; `--apply` mới ghi. Tệp đã quá 7
/// ngày bị sweep retention xoá ở vòng kế tiếp (tối đa một giờ), nên sao lưu trước khi `--apply`.
async fn shorten_free_retention(db: &Database, apply: bool) -> Result<Value, String> {
    let plus = user::plus_user_ids(db, DateTime::now()).await.map_err(err)?;
    let (files, users, due_now) = file::count_longer_than(db, doc! { "$nin": &plus }, FREE_RETENTION_DAYS).await.map_err(err)?;
    let mut out = json!({
        "applied": apply,
        "retentionDays": FREE_RETENTION_DAYS,
        "plusUsersSkipped": plus.len(),
        "freeUsers": users,
        "files": files,
        "expireOnNextSweep": due_now,
    });
    if apply {
        let changed = file::shorten_expiry(db, doc! { "$nin": &plus }, FREE_RETENTION_DAYS).await.map_err(err)?;
        out["changed"] = json!(changed);
        let details = doc! { "files": changed as i64, "users": users as i64, "days": FREE_RETENTION_DAYS };
        audit(db, None, "admin.shorten_free_retention", details).await;
    }
    Ok(out)
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
    // `--month` cũng là khoá kỳ sổ giờ; không có thì là kỳ ElevenLabs hiện tại (có thể lệch tháng dương lịch).
    let period = flag(args, "--month").map(str::to_string).unwrap_or_else(|| current_period(config));
    out["elevenlabs"] = provider_hours(config, db, &period).await?;
    if let Some(id) = &user_id {
        out["user"] = json!({ "userId": id, "usedMinutes": current.user_seconds / 60 });
    }
    if args.iter().any(|a| a == "--recompute") {
        out["recompute"] = recompute(db, &month, args.iter().any(|a| a == "--apply")).await?;
    }
    Ok(out)
}

/// Kỳ sổ giờ ElevenLabs đang chạy (từ ngày gia hạn `ELEVENLABS_RESET_DAY`).
fn current_period(config: &Config) -> String {
    usage::provider_period(Utc::now(), config.elevenlabs_reset_day)
}

/// Sổ giờ ElevenLabs của kỳ `month`: đã dùng, trần, ElevenLabs đã báo hết quota chưa và lúc kỳ sau bắt đầu.
async fn provider_hours(config: &Config, db: &Database, month: &str) -> Result<Value, String> {
    let used = usage::provider_seconds(db, month).await.map_err(err)?;
    let exhausted = usage::provider_exhausted(db, month).await.map_err(err)?;
    let hours = |s: i64| (s as f64 / 3600.0 * 100.0).round() / 100.0;
    Ok(json!({
        "usedMinutes": used / 60,
        "usedHours": hours(used),
        "limitHours": hours(config.elevenlabs_monthly_seconds),
        "quotaExhausted": exhausted,
        "period": month,
        "nextResetAt": usage::provider_next_reset(Utc::now(), config.elevenlabs_reset_day).to_rfc3339(),
    }))
}

/// Xem sổ giờ ElevenLabs của kỳ (`--month` = khoá kỳ, mặc định kỳ đang chạy); `--clear` gỡ dấu "hết quota" sau khi đã
/// nạp thêm credit.
async fn elevenlabs_quota(config: &Config, db: &Database, args: &[String]) -> Result<Value, String> {
    let month = flag(args, "--month").map(str::to_string).unwrap_or_else(|| current_period(config));
    let mut out = json!({ "month": month });
    if args.iter().any(|a| a == "--clear") {
        let cleared = usage::clear_provider_exhausted(db, &month).await.map_err(err)?;
        audit(db, None, "admin.elevenlabs_quota_clear", doc! { "month": &month, "cleared": cleared }).await;
        out["cleared"] = json!(cleared);
    }
    out["elevenlabs"] = provider_hours(config, db, &month).await?;
    Ok(out)
}

/// Tính lại sổ tháng: lượt đã quyết toán (`transcribe_runs.sentS`; sổ quy theo USD dùng `costS` khi có) + lượt còn mở (phần đang giữ,
/// `chargedS` — quyết toán sau sẽ cộng chênh lệch); trần tổng cộng thêm chi phí tóm tắt và hỏi đáp
/// (`summary_runs.costS`, `ask_runs.costS`). In chênh lệch; `apply` cộng chênh lệch bằng `$inc` và
/// chỉ chạy khi tháng không còn lượt nào đang mở (giữ chỗ đang dở sẽ làm số sai).
async fn recompute(db: &Database, month: &str, apply: bool) -> Result<Value, String> {
    let mut per_user: HashMap<String, i64> = HashMap::new();
    let (mut free_pool, mut total) = (0_i64, 0_i64);
    // `cost`: giây trên sổ quy theo USD (chép trực tiếp có giá khác chép lời theo tệp).
    let mut add = |user: &str, free: bool, seconds: i64, cost: i64| {
        *per_user.entry(user.to_string()).or_default() += seconds;
        total += cost;
        if free {
            free_pool += cost;
        }
    };
    let read = |e: mongodb::error::Error| format!("đọc sổ: {e}");
    // Lượt bằng key riêng của người dùng (`byok`) không thuộc sổ nào của Scribe.
    let mut runs = db.collection::<Document>(usage::RUNS_COLL).find(doc! { "month": month, "byok": null }).await.map_err(read)?;
    while let Some(r) = runs.next().await {
        let r = r.map_err(read)?;
        let seconds = r.get_i64("sentS").unwrap_or(0);
        let cost = r.get_i64("costS").unwrap_or(seconds);
        add(r.get_str("userId").unwrap_or("-"), r.get_bool("freePool").unwrap_or(false), seconds, cost);
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
        add(&t.user_id, t.free_pool, t.charged_s, t.charged_s);
    }
    // Token chép trực tiếp chưa quyết toán: sổ đang tính trọn phần giữ chỗ (đã quyết toán thì nằm trong
    // `transcribe_runs` ở trên). Không chặn `--apply`: báo cáo đến muộn chỉ cộng chênh lệch bằng `$inc`.
    for (user, free, charged, cost) in crate::models::realtime_token::unsettled(db, month).await.map_err(err)? {
        add(&user, free, charged, cost);
    }
    // Chi phí token của tóm tắt, hỏi đáp và dịch chỉ tính vào trần tổng (không vào sổ phút của user hay quỹ
    // free); lượt đang gọi model giữ chỗ theo ước lượng và quyết toán sau, nên cũng chặn `--apply` như
    // lượt chép.
    let (mut running_summaries, mut running_asks, mut running_translations) = (0, 0, 0);
    for feature in crate::models::summary::Feature::ALL {
        total += crate::models::summary::month_cost_seconds(db, feature, month).await.map_err(err)?;
        let running = crate::models::summary::running_count(db, feature, month).await.map_err(err)?;
        match feature {
            crate::models::summary::Feature::Summary => running_summaries = running,
            crate::models::summary::Feature::Ask => running_asks = running,
            crate::models::summary::Feature::Translate => running_translations = running,
        }
    }
    if apply && open_runs > 0 {
        return Err(format!("còn {open_runs} lượt đang chạy trong tháng {month}: chờ hàng đợi rỗng rồi chạy lại --apply"));
    }
    if apply && running_summaries + running_asks + running_translations > 0 {
        return Err(format!(
            "còn {running_summaries} lượt tóm tắt, {running_asks} lượt hỏi đáp và {running_translations} lượt dịch đang chạy trong tháng {month}: chờ ít phút rồi chạy lại --apply"
        ));
    }
    for user_id in usage::user_ids_with_ledger(db, month).await.map_err(err)? {
        per_user.entry(user_id).or_default();
    }
    // Lượt của tài khoản đã xoá vẫn tính vào quỹ free và trần tổng ở trên, nhưng không còn sổ riêng để đối soát.
    per_user.remove(crate::account::DELETED_USER);
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
        "runningSummaries": running_summaries,
        "runningAsks": running_asks,
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
