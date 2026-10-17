//! Chép trực tiếp trong lúc ghi trên iPhone: backend cấp token tạm thời của OpenAI Realtime (phiên
//! `transcription`), app stream PCM thẳng tới OpenAI bằng token đó. Key server không bao giờ rời server.
//!
//! - Chỉ email trong `REALTIME_EMAILS` được dùng (rỗng = không ai), độc lập với `TRANSCRIBE_EMAILS`.
//! - Tính phút chung sổ với chép lời cả file (`models::usage`): hạn mức tháng của gói, sổ ngày và quỹ
//!   chung của gói free, trần tổng. Mỗi token giữ chỗ tối đa [`RESERVE_STEPS`]`[0]` giây; còn ít hơn thì
//!   giữ bậc nhỏ hơn. App không stream quá `reservedSeconds` bằng một token, hết thì xin token mới.
//! - App báo số giây đã stream (`POST /api/realtime/usage`) khi đóng phiên hoặc đổi token; quyết toán
//!   theo số đó (cộng thêm nếu vượt phần giữ). Không báo trong [`REPORT_GRACE_S`] sau khi token hết hạn
//!   thì phần giữ chỗ tính trọn (`models::realtime_token`).
//! - Cấu hình phiên (model, tiếng Việt, VAD phía server) gắn vào token lúc cấp; app không cần gửi
//!   `session.update`.

use crate::audit;
use crate::auth::UserId;
use crate::error::AppError;
use crate::models::realtime_token::{self, TokenDoc};
use crate::models::usage::{self, Blocked, Charge};
use crate::models::{plan, user};
use crate::state::AppState;
use crate::transcribe::{self, openai};
use axum::extract::State;
use axum::http::StatusCode;
use axum::{Extension, Json};
use mongodb::bson::{doc, DateTime};
use mongodb::Database;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

const MINT_URL: &str = "https://api.openai.com/v1/realtime/client_secrets";
/// Hạn của token (giây): mở phiên mới phải trước mốc này; phiên đã mở vẫn chạy tiếp sau đó.
pub const TOKEN_TTL_S: i64 = 600;
/// Bậc giữ chỗ cho một token, lớn trước: hạn mức còn ít thì vẫn chép được phần còn lại.
pub const RESERVE_STEPS: [i64; 4] = [600, 300, 120, 60];
/// Sau khi token hết hạn, app còn chừng này giây để báo số giây đã stream.
pub const REPORT_GRACE_S: i64 = 7_200;
/// Im lặng bao lâu thì OpenAI chốt một câu.
const SILENCE_MS: i64 = 500;

/// Thông báo khi tài khoản chưa được dùng chép trực tiếp.
const NOT_ALLOWED: &str = "Tài khoản này chưa được dùng chép trực tiếp.";
const MINT_FAILED: &str = "Không mở được phiên chép trực tiếp, thử lại sau.";

#[derive(Deserialize)]
pub struct TokenReq {
    /// Phiên ghi trên iPhone (UUID chữ thường), để tra cứu.
    #[serde(rename = "sessionId", default)]
    session_id: Option<String>,
}

#[derive(Deserialize)]
pub struct UsageReq {
    #[serde(rename = "tokenId")]
    token_id: String,
    /// Giây audio đã stream bằng token này.
    seconds: f64,
}

/// Id phiên/token hợp lệ: 1–64 ký tự chữ số, chữ thường a–z hoặc `-`.
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_digit() || b.is_ascii_lowercase() || b == b'-')
}

/// Cấu hình phiên gắn vào token: PCM16 24 kHz mono, `gpt-4o-mini-transcribe`, tiếng Việt, VAD server.
fn mint_body() -> Value {
    json!({
        "expires_after": { "anchor": "created_at", "seconds": TOKEN_TTL_S },
        "session": {
            "type": "transcription",
            "audio": { "input": {
                "format": { "type": "audio/pcm", "rate": 24_000 },
                "transcription": { "model": openai::MODEL, "language": "vi" },
                "turn_detection": { "type": "server_vad", "silence_duration_ms": SILENCE_MS },
            } },
        },
    })
}

/// Xin token tạm thời của OpenAI. Trả (giá trị token, hạn theo giây Unix). Lỗi theo `openai::api_error`
/// (key hỏng ⟹ `openai::is_key_broken`).
async fn mint(http: &reqwest::Client, api_key: &str) -> Result<(String, i64), String> {
    let response = http
        .post(MINT_URL)
        .bearer_auth(api_key)
        .json(&mint_body())
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| if e.is_timeout() { "OpenAI: quá thời gian chờ phản hồi" } else { "OpenAI: không kết nối được" })?;
    let status = response.status().as_u16();
    let body = response.text().await.unwrap_or_default();
    if !(200..300).contains(&status) {
        return Err(openai::api_error(status, &body));
    }
    parse_mint(&body).ok_or_else(|| "OpenAI trả token không đọc được".to_string())
}

fn parse_mint(body: &str) -> Option<(String, i64)> {
    let v: Value = serde_json::from_str(body).ok()?;
    let token = v["value"].as_str().filter(|t| !t.is_empty())?.to_string();
    Some((token, v["expires_at"].as_i64()?))
}

/// Giữ chỗ theo bậc lớn nhất còn vừa. Bị chặn ở mọi bậc thì trả lý do của bậc nhỏ nhất.
async fn reserve_window(
    db: &Database,
    charge: &Charge,
    user_limit: i64,
    limits: &crate::config::TranscribeLimits,
) -> Result<Result<i64, Blocked>, AppError> {
    let mut last = Blocked::UserQuota;
    for need in RESERVE_STEPS {
        match usage::reserve(db, charge, user_limit, need, limits).await? {
            Ok(()) => return Ok(Ok(need)),
            Err(blocked) => last = blocked,
        }
    }
    Ok(Err(last))
}

/// `POST /api/realtime/token` → `{token, tokenId, expiresAt, model, reservedSeconds}`.
pub async fn token(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Json(req): Json<TokenReq>,
) -> Result<Json<Value>, AppError> {
    let session_id = req.session_id.filter(|s| !s.is_empty());
    if session_id.as_deref().is_some_and(|s| !valid_id(s)) {
        return Err(AppError::BadRequest("phiên ghi không hợp lệ".into()));
    }
    let profile = user::find_profile(&state.db, &user.0)
        .await?
        .ok_or_else(|| AppError::Unauthorized("user không tồn tại".into()))?;
    if profile.blocked {
        return Err(AppError::Forbidden("tài khoản đã bị khoá".into()));
    }
    if !state.config.realtime_allowed(&profile.email) {
        audit::record(&state, Some(&user.0), "realtime.blocked", doc! { "reason": "NotAllowed" });
        return Err(AppError::Coded { status: StatusCode::FORBIDDEN, code: "REALTIME_NOT_ALLOWED", message: NOT_ALLOWED.into() });
    }
    let Some(api_key) = state.transcriber.api_key() else {
        return Err(AppError::Unavailable(transcribe::PAUSED.into()));
    };

    let now = chrono::Utc::now();
    let plan = plan::effective(profile.plan.as_deref(), profile.plan_until, DateTime::now());
    let limits = state.config.transcribe_limits;
    if plan == plan::Plan::Free {
        if let Some(e) = transcribe::too_new(&profile, limits.free_delay_hours, now) {
            audit::record(&state, Some(&user.0), "realtime.blocked", doc! { "reason": "AccountTooNew" });
            return Err(e);
        }
    }
    let charge = Charge::new(&user.0, plan, now);
    let user_limit = plan.limit_seconds(&limits);
    let reserved = match reserve_window(&state.db, &charge, user_limit, &limits).await? {
        Ok(seconds) => seconds,
        Err(blocked) => {
            let used_s = usage::blocked_usage(&state.db, &charge, &blocked).await?;
            audit::record(&state, Some(&user.0), "realtime.blocked", doc! { "reason": format!("{blocked:?}") });
            transcribe::note_repeated_block(&state, &charge, &blocked, &profile.email).await;
            return Err(transcribe::blocked_error(&blocked, used_s, usage::limit_of(&blocked, user_limit, &limits), now));
        }
    };
    // Từ đây mọi đường lỗi phải hoàn phần vừa giữ.
    let refund = |why: &'static str| {
        let (db, charge) = (state.db.clone(), charge.clone());
        async move {
            if let Err(e) = usage::settle(&db, &charge, -reserved).await {
                tracing::error!(reserved, "hoàn phút chép trực tiếp ({why}) lỗi: {e:?}");
            }
        }
    };
    let (value, expires_at) = match mint(&state.http, api_key).await {
        Ok(minted) => minted,
        Err(e) => {
            refund("không cấp được token").await;
            if openai::is_key_broken(&e) {
                transcribe::report_key_broken(&state, &e);
                return Err(AppError::Unavailable(transcribe::PAUSED.into()));
            }
            tracing::warn!("cấp token chép trực tiếp lỗi: {e}");
            return Err(AppError::Unavailable(MINT_FAILED.into()));
        }
    };
    let created = DateTime::now();
    let doc = TokenDoc {
        id: uuid::Uuid::new_v4().to_string(),
        user_id: user.0.clone(),
        session_id: session_id.clone(),
        month: charge.month.clone(),
        free_pool: charge.free_pool,
        day: charge.day.clone(),
        charged_s: reserved,
        settled: false,
        created_at: created,
        report_by: DateTime::from_millis((expires_at + REPORT_GRACE_S) * 1000),
    };
    if let Err(e) = realtime_token::insert(&state.db, &doc).await {
        refund("không ghi được token").await;
        return Err(e);
    }
    transcribe::note_cap_levels(&state, &charge.month).await;
    let details = doc! { "tokenId": &doc.id, "sessionId": session_id, "seconds": reserved, "plan": plan.name() };
    audit::record(&state, Some(&user.0), "realtime.token", details);
    let expires = chrono::DateTime::from_timestamp(expires_at, 0).map(|t| t.to_rfc3339()).unwrap_or_default();
    Ok(Json(json!({
        "token": value,
        "tokenId": doc.id,
        "expiresAt": expires,
        "model": openai::MODEL,
        "reservedSeconds": reserved,
    })))
}

/// Kết quả một lần báo số giây.
#[derive(Debug, PartialEq)]
pub enum Report {
    /// Đã quyết toán: (giây đã giữ, giây tính thật).
    Settled { charged: i64, sent: i64 },
    /// Token đã quyết toán trước đó (app gửi lại): không đổi gì.
    Already,
    /// Quá hạn báo: phần giữ chỗ đã tính trọn.
    TooLate,
    NotFound,
}

/// Giây tính cho một báo cáo: làm tròn lên, không quá thời gian thật từ lúc cấp token (+5 giây lệch
/// đồng hồ), vì không thể stream nhanh hơn thời gian thực.
pub fn billable(seconds: f64, created: DateTime, now: DateTime) -> i64 {
    let wall = (now.timestamp_millis() - created.timestamp_millis()).max(0) / 1000 + 5;
    (seconds.max(0.0).ceil() as i64).min(wall)
}

/// Quyết toán token `token_id` của `user_id` theo `seconds` app báo.
pub async fn settle_report(db: &Database, user_id: &str, token_id: &str, seconds: f64, now: DateTime) -> Result<Report, AppError> {
    let Some(t) = realtime_token::find_owned(db, user_id, token_id).await? else { return Ok(Report::NotFound) };
    if t.settled {
        return Ok(Report::Already);
    }
    if t.report_by <= now {
        return Ok(Report::TooLate);
    }
    let sent = billable(seconds, t.created_at, now);
    if !realtime_token::close(db, user_id, token_id, sent, now).await? {
        // Một báo cáo khác vừa nhận quyền quyết toán (hoặc vừa quá hạn).
        let again = realtime_token::find_owned(db, user_id, token_id).await?;
        return Ok(if again.is_some_and(|t| t.settled) { Report::Already } else { Report::TooLate });
    }
    let charge = t.charge();
    if let Err(e) = usage::settle(db, &charge, sent - t.charged_s).await {
        tracing::error!(token = %t.id, "quyết toán phút chép trực tiếp lỗi: {e:?}");
    }
    // Sổ chi tiết chỉ thêm, chung với lượt chép file: nguồn của lệnh đối soát.
    let run = doc! {
        "kind": "realtime",
        "tokenId": &t.id,
        "userId": &t.user_id,
        "month": &charge.month,
        "freePool": charge.free_pool,
        "day": &charge.day,
        "chargedS": t.charged_s,
        "sentS": sent,
        "at": now,
    };
    if let Err(e) = db.collection::<mongodb::bson::Document>(usage::RUNS_COLL).insert_one(run).await {
        tracing::error!(token = %t.id, "ghi sổ lượt chép trực tiếp lỗi: {e}");
    }
    Ok(Report::Settled { charged: t.charged_s, sent })
}

/// `POST /api/realtime/usage {tokenId, seconds}` — app báo số giây đã stream bằng một token.
pub async fn report_usage(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Json(req): Json<UsageReq>,
) -> Result<Json<Value>, AppError> {
    if !valid_id(&req.token_id) {
        return Err(AppError::BadRequest("token không hợp lệ".into()));
    }
    if !req.seconds.is_finite() || req.seconds < 0.0 || req.seconds > 86_400.0 {
        return Err(AppError::BadRequest("số giây không hợp lệ".into()));
    }
    match settle_report(&state.db, &user.0, &req.token_id, req.seconds, DateTime::now()).await? {
        Report::Settled { charged, sent } => Ok(Json(json!({ "settled": true, "chargedSeconds": charged, "seconds": sent }))),
        Report::Already => Ok(Json(json!({ "settled": true, "already": true }))),
        Report::TooLate => Err(AppError::Coded {
            status: StatusCode::CONFLICT,
            code: "REPORT_TOO_LATE",
            message: "Đã quá hạn báo số giây của token này; phần giữ chỗ được tính trọn.".into(),
        }),
        Report::NotFound => Err(AppError::NotFound("token không tồn tại".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mint_body_locks_model_language_and_vad() {
        let b = mint_body();
        assert_eq!(b["expires_after"]["seconds"], 600);
        assert_eq!(b["session"]["type"], "transcription");
        let input = &b["session"]["audio"]["input"];
        assert_eq!(input["format"]["rate"], 24_000);
        assert_eq!(input["transcription"]["model"], "gpt-4o-mini-transcribe");
        assert_eq!(input["transcription"]["language"], "vi");
        assert_eq!(input["turn_detection"]["type"], "server_vad");
        assert_eq!(input["turn_detection"]["silence_duration_ms"], 500);
    }

    #[test]
    fn parses_client_secret_reply() {
        assert_eq!(parse_mint(r#"{"value":"ek_1","expires_at":1760000600}"#), Some(("ek_1".into(), 1_760_000_600)));
        assert_eq!(parse_mint(r#"{"value":"","expires_at":1}"#), None);
        assert_eq!(parse_mint(r#"{"value":"ek_1"}"#), None);
        assert_eq!(parse_mint("nope"), None);
    }

    #[test]
    fn ids_are_short_lowercase_tokens() {
        assert!(valid_id("0b9c1e7e-1f0a-4c1e-9a55-3c7a8c4b2d10"));
        assert!(!valid_id(""));
        assert!(!valid_id("ABC"));
        assert!(!valid_id("a/b"));
        assert!(!valid_id(&"a".repeat(65)));
    }

    #[test]
    fn billable_rounds_up_and_never_beats_the_clock() {
        let created = DateTime::from_millis(1_000_000);
        let after = |s: i64| DateTime::from_millis(1_000_000 + s * 1000);
        assert_eq!(billable(12.2, created, after(600)), 13);
        assert_eq!(billable(-3.0, created, after(600)), 0);
        // 900 giây báo sau 300 giây thật: chỉ tính 300 + 5 giây lệch đồng hồ.
        assert_eq!(billable(900.0, created, after(300)), 305);
        // Đồng hồ server lùi: vẫn không âm.
        assert_eq!(billable(10.0, created, DateTime::from_millis(0)), 5);
    }

    #[test]
    fn reserve_steps_shrink_to_a_minute() {
        assert_eq!(RESERVE_STEPS[0], TOKEN_TTL_S);
        assert!(RESERVE_STEPS.windows(2).all(|w| w[0] > w[1]));
        assert_eq!(RESERVE_STEPS[RESERVE_STEPS.len() - 1], 60);
    }

    /// Cấp token thật qua OpenAI (cần mạng, không tốn phút audio): `OPENAI_API_KEY_FILE=<tệp chứa key>
    /// cargo test live_realtime_mint -- --ignored`. Không in token.
    #[tokio::test]
    #[ignore]
    async fn live_realtime_mint_returns_a_ten_minute_token() {
        let key_file = std::env::var("OPENAI_API_KEY_FILE").expect("OPENAI_API_KEY_FILE");
        let key = std::fs::read_to_string(key_file).expect("đọc key");
        let (token, expires_at) = mint(&reqwest::Client::new(), key.trim()).await.expect("cấp token");
        let ttl = expires_at - chrono::Utc::now().timestamp();
        println!("token {} ký tự, còn {ttl} giây", token.len());
        assert!(token.starts_with("ek_"));
        assert!((590..=610).contains(&ttl), "ttl {ttl}");
    }

    /// Trên Mongo thật (DB dev): `TRANSCRIPT_TEST_MONGO_URI=… TRANSCRIPT_TEST_DB_NAME=… cargo test
    /// realtime_metering -- --ignored`. Tháng giả riêng, tự dọn.
    #[tokio::test]
    #[ignore]
    async fn realtime_metering_reserves_settles_once_and_keeps_late_reports() {
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        let month = format!("test-{}", uuid::Uuid::new_v4().simple());
        let user = format!("rt-{}", uuid::Uuid::new_v4().simple());
        let charge = Charge { user_id: user.clone(), month: month.clone(), free_pool: true, day: Some(month.clone()) };
        let limits = crate::config::TranscribeLimits {
            free_seconds: 1_000,
            plus_seconds: 10_000,
            free_pool_seconds: 10_000,
            monthly_cap_seconds: 10_000,
            free_daily_seconds: 1_000,
            free_delay_hours: 0,
        };
        // Hạn mức 1000 giây: 600 rồi 300 (còn 100 ⟹ bậc 60), rồi chặn hẳn ở 960/1000.
        let a = reserve_window(&db, &charge, 1_000, &limits).await.expect("giữ");
        let b = reserve_window(&db, &charge, 1_000, &limits).await.expect("giữ");
        let c = reserve_window(&db, &charge, 1_000, &limits).await.expect("giữ");
        assert_eq!((a, b, c), (Ok(600), Ok(300), Ok(60)));
        assert_eq!(reserve_window(&db, &charge, 1_000, &limits).await.expect("giữ"), Err(Blocked::UserQuota));
        assert_eq!(usage::month_usage(&db, &user, &month).await.expect("đọc").user_seconds, 960);

        let created = DateTime::from_millis(DateTime::now().timestamp_millis() - 700_000);
        let token = |id: &str, charged: i64, report_by: DateTime| TokenDoc {
            id: id.into(),
            user_id: user.clone(),
            session_id: None,
            month: month.clone(),
            free_pool: true,
            day: Some(month.clone()),
            charged_s: charged,
                settled: false,
            created_at: created,
            report_by,
        };
        let later = DateTime::from_millis(DateTime::now().timestamp_millis() + 3_600_000);
        let id_a = format!("{user}-a");
        let id_late = format!("{user}-late");
        realtime_token::insert(&db, &token(&id_a, 600, later)).await.expect("ghi");
        realtime_token::insert(&db, &token(&id_late, 300, DateTime::from_millis(0))).await.expect("ghi");

        // Báo 250,4 giây ⟹ tính 251, hoàn 349 trên mọi sổ; báo lại không đổi gì.
        let now = DateTime::now();
        assert_eq!(settle_report(&db, &user, &id_a, 250.4, now).await.expect("báo"), Report::Settled { charged: 600, sent: 251 });
        assert_eq!(settle_report(&db, &user, &id_a, 250.4, now).await.expect("báo"), Report::Already);
        let used = usage::month_usage(&db, &user, &month).await.expect("đọc");
        assert_eq!((used.user_seconds, used.free_pool_seconds, used.total_seconds), (611, 611, 611));
        assert_eq!(usage::day_usage_seconds(&db, &user, &month).await.expect("ngày"), 611);
        // Quá hạn báo: phần giữ 300 giây tính trọn; token người khác thì không thấy.
        assert_eq!(settle_report(&db, &user, &id_late, 10.0, now).await.expect("báo"), Report::TooLate);
        assert_eq!(settle_report(&db, "other", &id_a, 10.0, now).await.expect("báo"), Report::NotFound);
        assert_eq!(usage::month_usage(&db, &user, &month).await.expect("đọc").user_seconds, 611);
        assert_eq!(realtime_token::unsettled(&db, &month).await.expect("đọc"), vec![(user.clone(), true, 300)]);

        db.collection::<mongodb::bson::Document>("transcribe_usage")
            .delete_many(doc! { "_id": { "$regex": format!("{month}$") } })
            .await
            .expect("dọn");
        db.collection::<mongodb::bson::Document>("realtime_tokens").delete_many(doc! { "userId": &user }).await.expect("dọn");
        db.collection::<mongodb::bson::Document>(usage::RUNS_COLL).delete_many(doc! { "userId": &user }).await.expect("dọn");
    }
}
