//! Chép trực tiếp trong lúc ghi trên iPhone: backend cấp token tạm thời của nhà cung cấp, app stream PCM thẳng tới nhà
//! cung cấp bằng token đó. Key server không bao giờ rời server.
//!
//! - Nhà cung cấp theo biến build `REALTIME_PROVIDER` (chủ dự án chốt 19/10/2026): ElevenLabs Scribe v2 Realtime mặc
//!   định — token dùng một lần (`POST /v1/single-use-token/realtime_scribe`), một token mở được đúng một WebSocket, hết
//!   hạn 15 phút sau khi cấp; `openai` giữ OpenAI Realtime (`gpt-4o-mini-transcribe`) làm dự phòng một bản phát hành.
//! - Chỉ email trong `REALTIME_EMAILS` được dùng (rỗng = không ai), độc lập với `TRANSCRIBE_EMAILS`.
//! - Tính phút chung sổ với chép lời cả file (`models::usage`): hạn mức tháng của gói, sổ ngày và quỹ chung của gói
//!   free, trần tổng. Sổ quy theo USD (quỹ free, trần tổng) tính theo giá chép trực tiếp (Scribe v2 Realtime 0,39
//!   USD/giờ, đắt hơn chép lời theo tệp). Qua ElevenLabs thì còn tính vào sổ giờ ElevenLabs của tháng
//!   (`ELEVENLABS_MONTHLY_HOURS`), quy đổi theo giá Realtime / giá Scribe v2 vì cùng trừ credit của một gói.
//! - Mỗi token giữ chỗ tối đa [`RESERVE_STEPS`]`[0]` giây; còn ít hơn thì giữ bậc nhỏ hơn. App không stream quá
//!   `reservedSeconds` bằng một token, hết thì xin token mới (ElevenLabs: mỗi lần nối lại cũng xin token mới).
//! - App báo số giây đã stream (`POST /api/realtime/usage`) khi đóng phiên hoặc đổi token; quyết toán theo số đó (cộng
//!   thêm nếu vượt phần giữ) trên mọi sổ. Không báo trong [`REPORT_GRACE_S`] sau khi token hết hạn thì phần giữ chỗ tính
//!   trọn (`models::realtime_token`).
//! - Cấu hình phiên (model, tiếng Việt, định dạng audio, VAD) do backend quyết: OpenAI gắn vào token lúc cấp; ElevenLabs
//!   nằm trong `url` trả cho app (app chỉ thêm `token`).
//! - Người dùng có key riêng cho chép trực tiếp (ElevenLabs hoặc OpenAI, `user_keys`): token cấp bằng key đó, không giữ
//!   chỗ trên sổ nào, không chặn theo hạn mức hay sổ giờ của Scribe; báo số giây chỉ ghi vào sổ phút riêng của người
//!   dùng. Key lỗi lúc cấp ⟹ 403 `USER_KEY_FAILED`, key bị đánh dấu, không dùng key server thay.

use crate::audit;
use crate::auth::UserId;
use crate::config::{Provider, TranscribeLimits, Vendor};
use crate::error::AppError;
use crate::models::realtime_token::{self, weighted, TokenDoc};
use crate::models::usage::{self, Blocked, Charge};
use crate::models::{plan, user};
use crate::notify;
use crate::state::AppState;
use crate::transcribe::{self, elevenlabs, openai};
use axum::extract::State;
use axum::http::StatusCode;
use axum::{Extension, Json};
use mongodb::bson::{doc, DateTime};
use mongodb::Database;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

const OPENAI_MINT_URL: &str = "https://api.openai.com/v1/realtime/client_secrets";
const OPENAI_WS_URL: &str = "wss://api.openai.com/v1/realtime";
/// Gốc API ElevenLabs (cấp token dùng một lần).
pub const ELEVENLABS_API: &str = "https://api.elevenlabs.io";
const ELEVENLABS_WS_URL: &str = "wss://api.elevenlabs.io/v1/speech-to-text/realtime";
/// Hạn token OpenAI (giây): mở phiên mới phải trước mốc này; phiên đã mở vẫn chạy tiếp sau đó.
pub const TOKEN_TTL_S: i64 = 600;
/// Token dùng một lần của ElevenLabs hết hạn 15 phút sau khi cấp (tính từ lúc backend bắt đầu xin).
const ELEVENLABS_TOKEN_TTL_S: i64 = 900;
/// Bậc giữ chỗ cho một token, lớn trước: hạn mức còn ít thì vẫn chép được phần còn lại.
pub const RESERVE_STEPS: [i64; 4] = [600, 300, 120, 60];
/// Sau khi token hết hạn, app còn chừng này giây để báo số giây đã stream.
pub const REPORT_GRACE_S: i64 = 7_200;
/// App gửi PCM16 mono 24 kHz cho cả hai nhà cung cấp (ElevenLabs nhận `pcm_24000`; OpenAI chỉ nhận 24 kHz).
pub const SAMPLE_RATE: i64 = 24_000;
/// Im lặng bao lâu thì OpenAI chốt một câu.
const OPENAI_SILENCE_MS: i64 = 500;
/// Im lặng bao lâu (giây) thì ElevenLabs chốt một câu (VAD phía ElevenLabs, nhận 0,3–3).
const ELEVENLABS_SILENCE_S: &str = "1.0";

/// Số token tối đa mỗi phút mỗi tài khoản cấp bằng key riêng (không có sổ nào giữ chỗ để hãm): một phiên ghi xin token mới
/// khoảng 10 phút một lần, nối lại khi mất mạng cũng xin token, nên 10 lần/phút là dư cho người dùng thật.
const OWN_TOKENS_PER_MINUTE: u32 = 10;

/// Còn được cấp token bằng key riêng trong phút này không.
fn own_token_allowed(rate: &crate::rate_limit::RateLimiter, user_id: &str) -> bool {
    rate.allow(&format!("realtime-own:{user_id}"), OWN_TOKENS_PER_MINUTE)
}

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
    /// ElevenLabs báo `quota_exceeded` trên WebSocket của token này: tài khoản server hết credit tháng.
    #[serde(rename = "quotaExceeded", default)]
    quota_exceeded: bool,
}

/// Id phiên/token hợp lệ: 1–64 ký tự chữ số, chữ thường a–z hoặc `-`.
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_digit() || b.is_ascii_lowercase() || b == b'-')
}

/// Token vừa cấp: giá trị (không lưu), hạn theo giây Unix và địa chỉ WebSocket app mở.
#[derive(Debug, PartialEq)]
struct Minted {
    token: String,
    expires_at: i64,
    url: String,
}

/// Cấu hình phiên OpenAI gắn vào token: PCM16 24 kHz mono, `gpt-4o-mini-transcribe`, tiếng Việt, VAD server.
fn openai_mint_body() -> Value {
    json!({
        "expires_after": { "anchor": "created_at", "seconds": TOKEN_TTL_S },
        "session": {
            "type": "transcription",
            "audio": { "input": {
                "format": { "type": "audio/pcm", "rate": SAMPLE_RATE },
                "transcription": { "model": openai::MODEL, "language": "vi" },
                "turn_detection": { "type": "server_vad", "silence_duration_ms": OPENAI_SILENCE_MS },
            } },
        },
    })
}

/// Xin token tạm thời của OpenAI. Lỗi theo `openai::api_error` (key hỏng ⟹ `openai::is_key_broken`).
async fn mint_openai(http: &reqwest::Client, api_key: &str) -> Result<Minted, String> {
    let response = http
        .post(OPENAI_MINT_URL)
        .bearer_auth(api_key)
        .json(&openai_mint_body())
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| if e.is_timeout() { "OpenAI: quá thời gian chờ phản hồi" } else { "OpenAI: không kết nối được" })?;
    let status = response.status().as_u16();
    let body = response.text().await.unwrap_or_default();
    if !(200..300).contains(&status) {
        return Err(openai::api_error(status, &body));
    }
    let (token, expires_at) = parse_openai_mint(&body).ok_or_else(|| "OpenAI trả token không đọc được".to_string())?;
    Ok(Minted { token, expires_at, url: OPENAI_WS_URL.into() })
}

fn parse_openai_mint(body: &str) -> Option<(String, i64)> {
    let v: Value = serde_json::from_str(body).ok()?;
    let token = v["value"].as_str().filter(|t| !t.is_empty())?.to_string();
    Some((token, v["expires_at"].as_i64()?))
}

/// WebSocket ElevenLabs với cấu hình phiên; app thêm `&token=…`. Mốc từ trong câu đã chốt
/// (`committed_transcript_with_timestamps`) tính từ khung audio đầu tiên gửi trên kết nối đó.
fn elevenlabs_ws_url() -> String {
    format!(
        "{ELEVENLABS_WS_URL}?model_id={}&audio_format=pcm_{SAMPLE_RATE}&language_code=vi&commit_strategy=vad\
         &vad_silence_threshold_secs={ELEVENLABS_SILENCE_S}&include_timestamps=true",
        Provider::ElevenLabs.realtime_model()
    )
}

/// Xin token dùng một lần của ElevenLabs (`{api}/v1/single-use-token/realtime_scribe`, header `xi-api-key`). Lỗi theo
/// `elevenlabs::api_error`: 402/hết credit ⟹ `QUOTA_EXHAUSTED`, 401/403 ⟹ `KEY_INVALID`.
async fn mint_elevenlabs(http: &reqwest::Client, api: &str, api_key: &str) -> Result<Minted, String> {
    let issued = chrono::Utc::now().timestamp();
    let response = http
        .post(format!("{api}/v1/single-use-token/realtime_scribe"))
        .header("xi-api-key", api_key)
        // Không có thân nhưng phải có `Content-Length: 0`: thiếu thì ElevenLabs trả 411.
        .header(reqwest::header::CONTENT_LENGTH, "0")
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| if e.is_timeout() { "ElevenLabs: quá thời gian chờ phản hồi" } else { "ElevenLabs: không kết nối được" })?;
    let status = response.status().as_u16();
    let body = response.text().await.unwrap_or_default();
    if !(200..300).contains(&status) {
        return Err(elevenlabs::api_error(status, &body));
    }
    let token = serde_json::from_str::<Value>(&body)
        .ok()
        .and_then(|v| v["token"].as_str().filter(|t| !t.is_empty()).map(str::to_string))
        .ok_or_else(|| "ElevenLabs trả token không đọc được".to_string())?;
    Ok(Minted { token, expires_at: issued + ELEVENLABS_TOKEN_TTL_S, url: elevenlabs_ws_url() })
}

/// Giá một token, theo giây audio: `cost` giây trên sổ quy theo USD (giá chép trực tiếp / giá chép lời theo tệp của bản
/// build, vì trần USD đổi ra giây theo giá đó) và `el` giây trên sổ giờ ElevenLabs (0 khi không qua ElevenLabs).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Price {
    cost: f64,
    el: f64,
}

fn price(realtime: Provider, transcribe: Provider) -> Price {
    let usd = realtime.realtime_usd_per_minute();
    let el = match realtime {
        Provider::ElevenLabs => usd / Provider::ElevenLabs.usd_per_minute(),
        Provider::OpenAi => 0.0,
    };
    Price { cost: usd / transcribe.usd_per_minute(), el }
}

/// Vì sao không giữ được chỗ cho một token.
#[derive(Debug, PartialEq)]
enum Refused {
    Books(Blocked),
    /// Sổ giờ ElevenLabs của tháng không còn đủ chỗ.
    ProviderHours,
}

/// Giữ chỗ theo bậc lớn nhất còn vừa, trên sổ phút (sổ quy theo USD theo `price.cost`) và sổ giờ ElevenLabs (theo
/// `price.el`, trần `el_limit`). Bị chặn ở mọi bậc thì trả lý do của bậc nhỏ nhất.
async fn reserve_window(
    db: &Database,
    charge: &Charge,
    user_limit: i64,
    limits: &TranscribeLimits,
    price: Price,
    el_limit: i64,
) -> Result<Result<i64, Refused>, AppError> {
    let mut last = Refused::Books(Blocked::UserQuota);
    for need in RESERVE_STEPS {
        let cost = weighted(need, price.cost);
        if let Err(blocked) = usage::reserve_priced(db, charge, user_limit, need, cost, limits).await? {
            last = Refused::Books(blocked);
            continue;
        }
        let el = weighted(need, price.el);
        let hours = if el > 0 { usage::reserve_provider(db, &charge.month, el, el_limit).await } else { Ok(true) };
        match hours {
            Ok(true) => return Ok(Ok(need)),
            Ok(false) => last = Refused::ProviderHours,
            Err(_) => {}
        }
        if let Err(e) = usage::settle_priced(db, charge, -need, -cost).await {
            tracing::error!(need, "trả phút chép trực tiếp khi sổ giờ đầy lỗi: {e:?}");
        }
        hours?;
    }
    Ok(Err(last))
}

/// Hoàn trọn phần một token đã giữ trên mọi sổ.
async fn refund(db: &Database, charge: &Charge, reserved: i64, price: Price, why: &str) {
    if let Err(e) = usage::settle_priced(db, charge, -reserved, -weighted(reserved, price.cost)).await {
        tracing::error!(reserved, "hoàn phút chép trực tiếp ({why}) lỗi: {e:?}");
    }
    let el = weighted(reserved, price.el);
    if el > 0 {
        if let Err(e) = usage::settle_provider(db, &charge.month, -el).await {
            tracing::error!(el, "hoàn giờ ElevenLabs của chép trực tiếp ({why}) lỗi: {e:?}");
        }
    }
}

/// Key của nhà cung cấp chép trực tiếp hỏng. Cùng key với chép lời theo tệp thì ngắt như chép lời
/// (`transcribe::report_key_broken`); khác key thì không được ngắt chép lời: ghi audit và báo chủ sản phẩm một lần mỗi
/// tháng.
async fn report_key_broken(state: &AppState, provider: Provider, month: &str, e: &str) {
    if provider == state.transcriber.provider() {
        transcribe::report_key_broken(state, e);
        return;
    }
    tracing::error!("key chép trực tiếp của server không dùng được ({e})");
    if usage::mark_once(&state.db, &format!("realtime.key_invalid:{month}")).await {
        audit::record(state, None, "realtime.key_invalid", doc! { "reason": e });
        notify::send(state, format!("Key chép trực tiếp của server không dùng được ({e}); thay key rồi khởi động lại."));
    }
}

/// `POST /api/realtime/token` → `{token, tokenId, expiresAt, model, reservedSeconds, provider, url, sampleRate}`.
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
    if let Some(own) = crate::user_keys::active(&state, &user.0).await?.live {
        return own_token(&state, &user.0, session_id, own).await.map(Json);
    }
    let provider = state.config.realtime_provider;
    let api_key = match provider {
        Provider::OpenAi => state.transcriber.api_key(),
        Provider::ElevenLabs => state.transcriber.elevenlabs_key(),
    };
    let Some(api_key) = api_key else {
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
    let price = price(provider, state.config.transcribe_provider);
    let el_limit = state.config.elevenlabs_monthly_seconds;
    // ElevenLabs đã báo hết quota tháng: không giữ chỗ, không xin token.
    if price.el > 0.0 && usage::provider_exhausted(&state.db, &charge.month).await? {
        return Err(hours_blocked(&state, &user.0, &charge.month, el_limit, now).await);
    }
    let user_limit = plan.limit_seconds(&limits);
    let reserved = match reserve_window(&state.db, &charge, user_limit, &limits, price, el_limit).await? {
        Ok(seconds) => seconds,
        Err(Refused::ProviderHours) => return Err(hours_blocked(&state, &user.0, &charge.month, el_limit, now).await),
        Err(Refused::Books(blocked)) => {
            let used_s = usage::blocked_usage(&state.db, &charge, &blocked).await?;
            audit::record(&state, Some(&user.0), "realtime.blocked", doc! { "reason": format!("{blocked:?}") });
            transcribe::note_repeated_block(&state, &charge, &blocked, &profile.email).await;
            return Err(transcribe::blocked_error(&blocked, used_s, usage::limit_of(&blocked, user_limit, &limits), now));
        }
    };
    // Từ đây mọi đường lỗi phải hoàn phần vừa giữ.
    let minted = match provider {
        Provider::OpenAi => mint_openai(&state.http, api_key).await,
        Provider::ElevenLabs => mint_elevenlabs(&state.http, ELEVENLABS_API, api_key).await,
    };
    let minted = match minted {
        Ok(minted) => minted,
        Err(e) => {
            refund(&state.db, &charge, reserved, price, "không cấp được token").await;
            if e == elevenlabs::QUOTA_EXHAUSTED {
                transcribe::report_quota_exhausted(&state, &charge.month).await;
                return Err(hours_blocked(&state, &user.0, &charge.month, el_limit, now).await);
            }
            if openai::is_key_broken(&e) || elevenlabs::is_key_broken(&e) {
                report_key_broken(&state, provider, &charge.month, &e).await;
                return Err(AppError::Unavailable(transcribe::PAUSED.into()));
            }
            tracing::warn!("cấp token chép trực tiếp lỗi: {e}");
            return Err(AppError::Unavailable(MINT_FAILED.into()));
        }
    };
    let doc = TokenDoc {
        id: uuid::Uuid::new_v4().to_string(),
        user_id: user.0.clone(),
        session_id: session_id.clone(),
        month: charge.month.clone(),
        free_pool: charge.free_pool,
        day: charge.day.clone(),
        charged_s: reserved,
        settled: false,
        created_at: DateTime::now(),
        report_by: DateTime::from_millis((minted.expires_at + REPORT_GRACE_S) * 1000),
        provider: provider.code().into(),
        cost_ratio: price.cost,
        el_ratio: price.el,
        byok: None,
        key_nonce: None,
    };
    if let Err(e) = realtime_token::insert(&state.db, &doc).await {
        refund(&state.db, &charge, reserved, price, "không ghi được token").await;
        return Err(e);
    }
    transcribe::note_cap_levels(&state, &charge.month).await;
    let details = doc! {
        "tokenId": &doc.id,
        "sessionId": session_id,
        "seconds": reserved,
        "plan": plan.name(),
        "provider": &doc.provider,
    };
    audit::record(&state, Some(&user.0), "realtime.token", details);
    Ok(Json(token_reply(&minted, &doc.id, reserved, provider)))
}

/// Sổ giờ ElevenLabs của tháng đã đầy hoặc ElevenLabs báo hết quota: ghi audit, trả lỗi `PROVIDER_HOURS`.
async fn hours_blocked(state: &AppState, user_id: &str, month: &str, limit: i64, now: chrono::DateTime<chrono::Utc>) -> AppError {
    audit::record(state, Some(user_id), "realtime.blocked", doc! { "reason": "ProviderHours" });
    let used = usage::provider_seconds(&state.db, month).await.unwrap_or(limit);
    transcribe::provider_hours_error(used, limit, now)
}

/// Token cấp bằng key riêng của người dùng (`own`: ElevenLabs hoặc OpenAI). Không giữ chỗ; app vẫn không stream quá
/// `reservedSeconds` bằng một token (đổi token như thường).
async fn own_token(state: &AppState, user_id: &str, session_id: Option<String>, own: Vendor) -> Result<Value, AppError> {
    if !own_token_allowed(&state.rate, user_id) {
        return Err(AppError::TooManyRequests("xin token chép trực tiếp quá nhanh, thử lại sau một phút".into()));
    }
    let provider = if own == Vendor::ElevenLabs { Provider::ElevenLabs } else { Provider::OpenAi };
    let key = crate::user_keys::open(state, user_id, own).await.map_err(crate::user_keys::user_key_error)?;
    let minted = match provider {
        Provider::OpenAi => mint_openai(&state.http, key.value()).await,
        Provider::ElevenLabs => mint_elevenlabs(&state.http, ELEVENLABS_API, key.value()).await,
    };
    let minted = match minted {
        Ok(minted) => minted,
        Err(e) => {
            if let Some(problem) = crate::user_keys::problem_of(&e) {
                return Err(crate::user_keys::user_key_error(crate::user_keys::report(state, &key, problem).await));
            }
            tracing::warn!("cấp token chép trực tiếp bằng key riêng lỗi: {e}");
            return Err(AppError::Unavailable(MINT_FAILED.into()));
        }
    };
    let key_nonce = realtime_token::nonce_hex(key.nonce());
    drop(key);
    let reserved = RESERVE_STEPS[0];
    let doc = TokenDoc {
        id: uuid::Uuid::new_v4().to_string(),
        user_id: user_id.to_string(),
        session_id: session_id.clone(),
        month: usage::month_key(chrono::Utc::now()),
        free_pool: false,
        day: None,
        charged_s: 0,
        settled: false,
        created_at: DateTime::now(),
        report_by: DateTime::from_millis((minted.expires_at + REPORT_GRACE_S) * 1000),
        provider: provider.code().into(),
        cost_ratio: 0.0,
        el_ratio: 0.0,
        byok: Some(own.code().into()),
        key_nonce: Some(key_nonce),
    };
    realtime_token::insert(&state.db, &doc).await?;
    let details = doc! { "tokenId": &doc.id, "sessionId": session_id, "seconds": reserved, "provider": &doc.provider, "byok": true };
    audit::record(state, Some(user_id), "realtime.token", details);
    Ok(token_reply(&minted, &doc.id, reserved, provider))
}

/// Phản hồi cho app. `provider` vắng = OpenAI (app cũ đọc như trước); `url` là WebSocket app mở.
fn token_reply(minted: &Minted, token_id: &str, reserved: i64, provider: Provider) -> Value {
    let expires = chrono::DateTime::from_timestamp(minted.expires_at, 0).map(|t| t.to_rfc3339()).unwrap_or_default();
    json!({
        "token": minted.token,
        "tokenId": token_id,
        "expiresAt": expires,
        "model": provider.realtime_model(),
        "reservedSeconds": reserved,
        "provider": provider.code(),
        "url": minted.url,
        "sampleRate": SAMPLE_RATE,
    })
}

/// Nonce của bản key riêng đã cấp token (token cấp bằng key server hay token cũ không có trường này: `None`).
fn own_key_nonce(t: &TokenDoc) -> Option<Vec<u8>> {
    t.byok.as_ref()?;
    realtime_token::nonce_from_hex(t.key_nonce.as_deref()?)
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

/// Quyết toán token `token_id` của `user_id` theo `seconds` app báo, trên sổ phút và sổ giờ ElevenLabs.
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
    if let Some(own) = &t.byok {
        // Key riêng: chỉ ghi giây cho người dùng xem và sổ chi tiết.
        if let Err(e) = usage::add_byok(db, user_id, &t.month, sent).await {
            tracing::warn!(token = %t.id, "ghi giây chép trực tiếp bằng key riêng lỗi: {e:?}");
        }
        let run = doc! {
            "kind": "realtime", "provider": &t.provider, "byok": own, "tokenId": &t.id, "userId": &t.user_id,
            "month": &t.month, "sentS": sent, "at": now,
        };
        if let Err(e) = db.collection::<mongodb::bson::Document>(usage::RUNS_COLL).insert_one(run).await {
            tracing::error!(token = %t.id, "ghi sổ lượt chép trực tiếp bằng key riêng lỗi: {e}");
        }
        return Ok(Report::Settled { charged: 0, sent });
    }
    let charge = t.charge();
    let cost_delta = t.cost_s(sent) - t.cost_s(t.charged_s);
    if let Err(e) = usage::settle_priced(db, &charge, sent - t.charged_s, cost_delta).await {
        tracing::error!(token = %t.id, "quyết toán phút chép trực tiếp lỗi: {e:?}");
    }
    let el_delta = t.el_s(sent) - t.el_s(t.charged_s);
    if el_delta != 0 {
        if let Err(e) = usage::settle_provider(db, &t.month, el_delta).await {
            tracing::error!(token = %t.id, "quyết toán giờ ElevenLabs của chép trực tiếp lỗi: {e:?}");
        }
    }
    // Sổ chi tiết chỉ thêm, chung với lượt chép file: nguồn của lệnh đối soát.
    let run = doc! {
        "kind": "realtime",
        "provider": &t.provider,
        "tokenId": &t.id,
        "userId": &t.user_id,
        "month": &charge.month,
        "freePool": charge.free_pool,
        "day": &charge.day,
        "chargedS": t.charged_s,
        "sentS": sent,
        "costS": t.cost_s(sent),
        "elS": t.el_s(sent),
        "at": now,
    };
    if let Err(e) = db.collection::<mongodb::bson::Document>(usage::RUNS_COLL).insert_one(run).await {
        tracing::error!(token = %t.id, "ghi sổ lượt chép trực tiếp lỗi: {e}");
    }
    Ok(Report::Settled { charged: t.charged_s, sent })
}

/// `POST /api/realtime/usage {tokenId, seconds, quotaExceeded?}` — app báo số giây đã stream bằng một token.
/// `quotaExceeded` (ElevenLabs báo hết credit trên WebSocket) dừng chép lời bằng ElevenLabs tới tháng sau như khi
/// lượt chép theo tệp gặp 402.
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
    if req.quota_exceeded {
        let owned = realtime_token::find_owned(&state.db, &user.0, &req.token_id).await?;
        match owned.filter(|t| t.provider == "elevenlabs") {
            // Key riêng hết credit: đánh dấu key đó, không đặt dấu hết quota tháng của server.
            Some(t) if t.byok.is_some() => {
                if let Some(nonce) = own_key_nonce(&t) {
                    let problem = crate::user_keys::Problem::NoCredit;
                    crate::user_keys::report_nonce(&state, &user.0, Vendor::ElevenLabs, &nonce, problem).await;
                }
            }
            Some(t) => {
                audit::record(&state, Some(&user.0), "realtime.provider_quota", doc! { "tokenId": &t.id });
                transcribe::report_quota_exhausted(&state, &t.month).await;
            }
            None => {}
        }
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
    use axum::http::HeaderMap;
    use axum::routing::post;
    use axum::Router;

    #[test]
    fn openai_mint_body_locks_model_language_and_vad() {
        let b = openai_mint_body();
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
    fn parses_openai_client_secret_reply() {
        assert_eq!(parse_openai_mint(r#"{"value":"ek_1","expires_at":1760000600}"#), Some(("ek_1".into(), 1_760_000_600)));
        assert_eq!(parse_openai_mint(r#"{"value":"","expires_at":1}"#), None);
        assert_eq!(parse_openai_mint(r#"{"value":"ek_1"}"#), None);
        assert_eq!(parse_openai_mint("nope"), None);
    }

    #[test]
    fn elevenlabs_url_locks_model_format_language_and_vad() {
        let url = elevenlabs_ws_url();
        let (base, query) = url.split_once('?').expect("có query");
        assert_eq!(base, "wss://api.elevenlabs.io/v1/speech-to-text/realtime");
        let params: Vec<&str> = query.split('&').collect();
        assert_eq!(
            params,
            vec![
                "model_id=scribe_v2_realtime",
                "audio_format=pcm_24000",
                "language_code=vi",
                "commit_strategy=vad",
                "vad_silence_threshold_secs=1.0",
                "include_timestamps=true",
            ]
        );
        assert!(!url.contains("token="), "app tự thêm token");
    }

    /// ElevenLabs giả trên cổng cục bộ: lần lượt trả token, 401, 402, rồi thân không đọc được.
    async fn fake_elevenlabs(replies: Vec<(StatusCode, &'static str)>) -> String {
        let replies = std::sync::Arc::new(std::sync::Mutex::new(replies.into_iter()));
        let handler = move |headers: HeaderMap, body: axum::body::Bytes| {
            let replies = replies.clone();
            async move {
                assert_eq!(headers.get("xi-api-key").and_then(|v| v.to_str().ok()), Some("k-test"));
                assert!(headers.get("authorization").is_none(), "key chỉ đi qua xi-api-key");
                assert!(body.is_empty(), "không gửi thân");
                assert_eq!(headers.get("content-length").and_then(|v| v.to_str().ok()), Some("0"), "ElevenLabs đòi Content-Length");
                replies.lock().expect("khoá").next().expect("còn phản hồi")
            }
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let api = format!("http://{}", listener.local_addr().expect("addr"));
        let app = Router::new().route("/v1/single-use-token/realtime_scribe", post(handler));
        tokio::spawn(async move { axum::serve(listener, app).await });
        api
    }

    #[tokio::test]
    async fn elevenlabs_mint_maps_token_and_errors() {
        let api = fake_elevenlabs(vec![
            (StatusCode::OK, r#"{"token":"sutkn_1"}"#),
            (StatusCode::UNAUTHORIZED, r#"{"detail":{"code":"invalid_api_key","message":"bad"}}"#),
            (StatusCode::PAYMENT_REQUIRED, r#"{"detail":{"code":"insufficient_credits"}}"#),
            (StatusCode::OK, r#"{"token":""}"#),
        ])
        .await;
        let http = reqwest::Client::new();
        let before = chrono::Utc::now().timestamp();
        let minted = mint_elevenlabs(&http, &api, "k-test").await.expect("cấp token");
        assert_eq!(minted.token, "sutkn_1");
        assert_eq!(minted.url, elevenlabs_ws_url());
        assert!((before + 899..=before + 901).contains(&minted.expires_at), "hạn 15 phút");
        assert_eq!(mint_elevenlabs(&http, &api, "k-test").await, Err(elevenlabs::KEY_INVALID.to_string()));
        assert_eq!(mint_elevenlabs(&http, &api, "k-test").await, Err(elevenlabs::QUOTA_EXHAUSTED.to_string()));
        assert!(mint_elevenlabs(&http, &api, "k-test").await.is_err());

        let reply = token_reply(&minted, "t-1", 600, Provider::ElevenLabs);
        assert_eq!(reply["token"], "sutkn_1");
        assert_eq!(reply["tokenId"], "t-1");
        assert_eq!(reply["provider"], "elevenlabs");
        assert_eq!(reply["model"], "scribe_v2_realtime");
        assert_eq!(reply["reservedSeconds"], 600);
        assert_eq!(reply["sampleRate"], 24_000);
        assert_eq!(reply["url"], elevenlabs_ws_url());
        let expires = reply["expiresAt"].as_str().expect("hạn");
        assert_eq!(chrono::DateTime::parse_from_rfc3339(expires).expect("rfc3339").timestamp(), minted.expires_at);
    }

    #[test]
    fn price_follows_the_realtime_rate() {
        let el = price(Provider::ElevenLabs, Provider::ElevenLabs);
        // 0,39 / 0,22 USD mỗi giờ: 600 giây audio = 1064 giây chép lời theo tệp, trên cả sổ USD và sổ giờ.
        assert!((el.cost - 0.39 / 0.22).abs() < 1e-9 && (el.el - 0.39 / 0.22).abs() < 1e-9);
        assert_eq!((weighted(600, el.cost), weighted(600, el.el)), (1064, 1064));
        // Chép lời theo tệp chạy OpenAI (trần đổi theo 0,003 USD/phút): sổ USD tính 2,17 lần.
        let mixed = price(Provider::ElevenLabs, Provider::OpenAi);
        assert_eq!(weighted(600, mixed.cost), 1300);
        // OpenAI Realtime: giá như chép lời OpenAI, không đụng sổ giờ ElevenLabs.
        assert_eq!(price(Provider::OpenAi, Provider::OpenAi), Price { cost: 1.0, el: 0.0 });
        assert_eq!(weighted(600, 0.0), 0);
    }

    #[test]
    fn a_late_quota_report_targets_the_key_that_minted_the_token() {
        let nonce = [0u8, 1, 0xab, 0xff, 7, 8, 9, 10, 11, 12, 13, 14];
        let hex = realtime_token::nonce_hex(&nonce);
        assert_eq!(hex, "0001abff0708090a0b0c0d0e");
        assert_eq!(realtime_token::nonce_from_hex(&hex).as_deref(), Some(&nonce[..]));
        assert_eq!(realtime_token::nonce_from_hex("abc"), None);
        assert_eq!(realtime_token::nonce_from_hex("zz"), None);
        let token = |byok: Option<&str>, key_nonce: Option<&str>| TokenDoc {
            id: "t".into(),
            user_id: "u".into(),
            session_id: None,
            month: "2026-10".into(),
            free_pool: false,
            day: None,
            charged_s: 0,
            settled: false,
            created_at: DateTime::from_millis(0),
            report_by: DateTime::from_millis(0),
            provider: "elevenlabs".into(),
            cost_ratio: 0.0,
            el_ratio: 0.0,
            byok: byok.map(str::to_string),
            key_nonce: key_nonce.map(str::to_string),
        };
        assert_eq!(own_key_nonce(&token(Some("elevenlabs"), Some(&hex))), Some(nonce.to_vec()));
        // Token cũ không có nonce: không đánh dấu key nào (không đoán là key hiện tại).
        assert_eq!(own_key_nonce(&token(Some("elevenlabs"), None)), None);
        assert_eq!(own_key_nonce(&token(None, Some(&hex))), None);
    }

    #[test]
    fn minting_with_an_own_key_is_rate_limited_per_account() {
        let rate = crate::rate_limit::RateLimiter::new();
        for _ in 0..OWN_TOKENS_PER_MINUTE {
            assert!(own_token_allowed(&rate, "u1"));
        }
        assert!(!own_token_allowed(&rate, "u1"), "lần thứ 11 trong phút bị chặn");
        assert!(own_token_allowed(&rate, "u2"), "tài khoản khác không bị ảnh hưởng");
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
    fn reserve_steps_shrink_to_a_minute_within_every_token_lifetime() {
        assert_eq!(RESERVE_STEPS[0], TOKEN_TTL_S);
        assert!(RESERVE_STEPS[0] < ELEVENLABS_TOKEN_TTL_S);
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
        let minted = mint_openai(&reqwest::Client::new(), key.trim()).await.expect("cấp token");
        let ttl = minted.expires_at - chrono::Utc::now().timestamp();
        println!("token {} ký tự, còn {ttl} giây", minted.token.len());
        assert!(minted.token.starts_with("ek_"));
        assert!((590..=610).contains(&ttl), "ttl {ttl}");
    }

    /// Cấp token dùng một lần thật qua ElevenLabs (cần mạng, không tốn credit): `ELEVENLABS_API_KEY_FILE=<tệp chứa
    /// key> cargo test live_elevenlabs_realtime_mint -- --ignored`. Không in token. In `url` để mở WebSocket thử.
    #[tokio::test]
    #[ignore]
    async fn live_elevenlabs_realtime_mint_returns_a_single_use_token() {
        let key_file = std::env::var("ELEVENLABS_API_KEY_FILE").expect("ELEVENLABS_API_KEY_FILE");
        let key = std::fs::read_to_string(key_file).expect("đọc key");
        let minted = mint_elevenlabs(&reqwest::Client::new(), ELEVENLABS_API, key.trim()).await.expect("cấp token");
        println!("token {} ký tự, url {}", minted.token.len(), minted.url);
        if let Ok(out) = std::env::var("ELEVENLABS_TOKEN_OUT") {
            // Cho script thử WebSocket đọc (tệp 600 trong thư mục tạm), không in ra.
            std::fs::write(&out, &minted.token).expect("ghi token");
            #[cfg(unix)]
            std::fs::set_permissions(&out, std::os::unix::fs::PermissionsExt::from_mode(0o600)).expect("chmod");
        }
        assert!(!minted.token.is_empty());
    }

    /// Báo hết quota trễ của token cấp bằng key cũ trên Mongo thật (DB dev): người dùng đã nhập key mới thì key mới không bị
    /// đánh dấu; token của key hiện tại thì đánh dấu đúng key đó. `TRANSCRIPT_TEST_MONGO_URI=… TRANSCRIPT_TEST_DB_NAME=…
    /// cargo test late_quota_report_on_real_mongo -- --ignored`. User giả, key giả, tự dọn.
    #[tokio::test]
    #[ignore]
    async fn late_quota_report_on_real_mongo() {
        use crate::user_keys::{crypto, store};
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        let user = format!("selftest-{}", uuid::Uuid::new_v4());
        let secret = crypto::Secret::from_bytes(&[5u8; 32]).expect("bí mật");
        let old = crypto::seal(&secret, &user, "elevenlabs", "sk_test_old_key_000001").expect("mã hoá");
        store::save(&db, &user, Vendor::ElevenLabs, &old, "0001").await.expect("lưu key cũ");
        let token = TokenDoc {
            id: format!("{user}-t"),
            user_id: user.clone(),
            session_id: None,
            month: "2026-10".into(),
            free_pool: false,
            day: None,
            charged_s: 0,
            settled: false,
            created_at: DateTime::now(),
            report_by: DateTime::now(),
            provider: "elevenlabs".into(),
            cost_ratio: 0.0,
            el_ratio: 0.0,
            byok: Some("elevenlabs".into()),
            key_nonce: Some(realtime_token::nonce_hex(&old.nonce)),
        };
        let fresh = crypto::seal(&secret, &user, "elevenlabs", "sk_test_new_key_000002").expect("mã hoá");
        store::save(&db, &user, Vendor::ElevenLabs, &fresh, "0002").await.expect("lưu key mới");
        let nonce = own_key_nonce(&token).expect("nonce của token");
        assert!(!store::set_status(&db, &user, Vendor::ElevenLabs, &nonce, store::Status::Quota).await.expect("đánh dấu"));
        assert_eq!(store::list(&db, &user).await.expect("đọc")[0].status, store::Status::Valid, "key mới không bị đánh dấu");
        let current = TokenDoc { key_nonce: Some(realtime_token::nonce_hex(&fresh.nonce)), ..token };
        let nonce = own_key_nonce(&current).expect("nonce");
        assert!(store::set_status(&db, &user, Vendor::ElevenLabs, &nonce, store::Status::Quota).await.expect("đánh dấu"));
        assert_eq!(store::list(&db, &user).await.expect("đọc")[0].status, store::Status::Quota);
        store::delete(&db, &user, Vendor::ElevenLabs).await.expect("dọn");
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
        let limits = TranscribeLimits {
            free_seconds: 1_000,
            plus_seconds: 10_000,
            free_pool_seconds: 10_000,
            monthly_cap_seconds: 10_000,
            free_daily_seconds: 1_000,
            free_delay_hours: 0,
        };
        let openai_price = Price { cost: 1.0, el: 0.0 };
        // Hạn mức 1000 giây: 600 rồi 300 (còn 100 ⟹ bậc 60), rồi chặn hẳn ở 960/1000.
        let a = reserve_window(&db, &charge, 1_000, &limits, openai_price, 0).await.expect("giữ");
        let b = reserve_window(&db, &charge, 1_000, &limits, openai_price, 0).await.expect("giữ");
        let c = reserve_window(&db, &charge, 1_000, &limits, openai_price, 0).await.expect("giữ");
        assert_eq!((a, b, c), (Ok(600), Ok(300), Ok(60)));
        let blocked = reserve_window(&db, &charge, 1_000, &limits, openai_price, 0).await.expect("giữ");
        assert_eq!(blocked, Err(Refused::Books(Blocked::UserQuota)));
        assert_eq!(usage::month_usage(&db, &user, &month).await.expect("đọc").user_seconds, 960);
        assert_eq!(usage::provider_seconds(&db, &month).await.expect("giờ"), 0, "OpenAI không đụng sổ giờ ElevenLabs");

        let created = DateTime::from_millis(DateTime::now().timestamp_millis() - 700_000);
        let token = |id: &str, charged: i64, report_by: DateTime, price: Price| TokenDoc {
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
            provider: if price.el > 0.0 { "elevenlabs".into() } else { "openai".into() },
            cost_ratio: price.cost,
            el_ratio: price.el,
            byok: None,
            key_nonce: None,
        };
        let later = DateTime::from_millis(DateTime::now().timestamp_millis() + 3_600_000);
        let id_a = format!("{user}-a");
        let id_late = format!("{user}-late");
        realtime_token::insert(&db, &token(&id_a, 600, later, openai_price)).await.expect("ghi");
        realtime_token::insert(&db, &token(&id_late, 300, DateTime::from_millis(0), openai_price)).await.expect("ghi");

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
        assert_eq!(realtime_token::unsettled(&db, &month).await.expect("đọc"), vec![(user.clone(), true, 300, 300)]);

        // ElevenLabs: sổ USD và sổ giờ tính theo giá Realtime (600 giây ⟹ 1064). Trần giờ 1500: token thứ hai chỉ còn
        // vừa bậc 120 (213 giây trên sổ giờ); hết chỗ ở sổ giờ thì sổ phút được trả lại.
        let el_price = price(Provider::ElevenLabs, Provider::ElevenLabs);
        let el_user = format!("{user}-el");
        let el_charge = Charge { user_id: el_user.clone(), month: month.clone(), free_pool: true, day: Some(month.clone()) };
        let first = reserve_window(&db, &el_charge, 5_000, &limits, el_price, 1_500).await.expect("giữ");
        assert_eq!(first, Ok(600));
        let used = usage::month_usage(&db, &el_user, &month).await.expect("đọc");
        assert_eq!(used.user_seconds, 600);
        assert_eq!(used.free_pool_seconds, 611 + 1064);
        assert_eq!(usage::provider_seconds(&db, &month).await.expect("giờ"), 1064);
        let second = reserve_window(&db, &el_charge, 5_000, &limits, el_price, 1_500).await.expect("giữ");
        assert_eq!(second, Ok(120));
        assert_eq!(usage::provider_seconds(&db, &month).await.expect("giờ"), 1064 + 213);
        let full = reserve_window(&db, &el_charge, 5_000, &limits, el_price, 1_300).await.expect("giữ");
        assert_eq!(full, Err(Refused::ProviderHours));
        assert_eq!(usage::month_usage(&db, &el_user, &month).await.expect("đọc").user_seconds, 720, "sổ phút được trả lại");

        // Quyết toán 100 giây của token 600 giây: hoàn 500 giây phút, 1064 − 178 trên sổ USD và sổ giờ.
        let id_el = format!("{user}-el-a");
        realtime_token::insert(&db, &token(&id_el, 600, later, el_price)).await.expect("ghi");
        let el_doc = realtime_token::find_owned(&db, &user, &id_el).await.expect("đọc").expect("có");
        assert_eq!((el_doc.provider.as_str(), el_doc.cost_s(600), el_doc.el_s(100)), ("elevenlabs", 1064, 178));
        let total_before = usage::month_usage(&db, &user, &month).await.expect("đọc").total_seconds;
        let hours_before = usage::provider_seconds(&db, &month).await.expect("giờ");
        assert_eq!(settle_report(&db, &user, &id_el, 100.0, now).await.expect("báo"), Report::Settled { charged: 600, sent: 100 });
        let used = usage::month_usage(&db, &user, &month).await.expect("đọc");
        assert_eq!(used.user_seconds, 611 - 500);
        assert_eq!(used.total_seconds, total_before - (1064 - 178));
        assert_eq!(usage::provider_seconds(&db, &month).await.expect("giờ"), hours_before - (1064 - 178));
        let run = db
            .collection::<mongodb::bson::Document>(usage::RUNS_COLL)
            .find_one(doc! { "tokenId": &id_el })
            .await
            .expect("đọc")
            .expect("có lượt");
        assert_eq!((run.get_i64("sentS"), run.get_i64("costS"), run.get_i64("elS")), (Ok(100), Ok(178), Ok(178)));

        // Hoàn trọn khi không cấp được token: mọi sổ về như trước.
        let hours_before = usage::provider_seconds(&db, &month).await.expect("giờ");
        refund(&db, &el_charge, 120, el_price, "thử").await;
        assert_eq!(usage::provider_seconds(&db, &month).await.expect("giờ"), hours_before - 213);
        assert_eq!(usage::month_usage(&db, &el_user, &month).await.expect("đọc").user_seconds, 600);

        db.collection::<mongodb::bson::Document>("transcribe_usage")
            .delete_many(doc! { "_id": { "$regex": format!("{month}$") } })
            .await
            .expect("dọn");
        db.collection::<mongodb::bson::Document>("realtime_tokens")
            .delete_many(doc! { "userId": { "$in": [&user, &el_user] } })
            .await
            .expect("dọn");
        db.collection::<mongodb::bson::Document>(usage::RUNS_COLL).delete_many(doc! { "userId": &user }).await.expect("dọn");
    }
}
