//! Chép trực tiếp trong lúc ghi trên iPhone: backend cấp token tạm thời của nhà cung cấp bằng key riêng của người dùng
//! ("Key riêng", `user_keys`), app stream PCM thẳng tới nhà cung cấp bằng token đó. Key không bao giờ rời server.
//!
//! - Chỉ chạy bằng key riêng (chủ dự án chốt 02/11/2026: Scribe không trả tiền AI thay người dùng): ElevenLabs Scribe v2
//!   Realtime — token dùng một lần (`POST /v1/single-use-token/realtime_scribe`), một token mở được đúng một WebSocket,
//!   hết hạn 15 phút sau khi cấp; hoặc OpenAI Realtime (`gpt-4o-mini-transcribe`). Không có key hợp lệ cho chép trực
//!   tiếp ⟹ 403 `NO_USER_KEY` (app dừng chép trực tiếp với câu đó, bản ghi vẫn tiếp tục).
//! - Không giữ chỗ hay chặn theo hạn mức nào; số token mỗi phút mỗi tài khoản có trần ([`OWN_TOKENS_PER_MINUTE`]). App
//!   vẫn không stream quá `reservedSeconds` bằng một token, hết thì xin token mới (ElevenLabs: mỗi lần nối lại cũng xin).
//! - App báo số giây đã stream (`POST /api/realtime/usage`) khi đóng phiên hoặc đổi token; số đó chỉ ghi vào sổ phút key
//!   riêng của người dùng để họ xem (`models::realtime_token`).
//! - Cấu hình phiên (model, tiếng Việt, định dạng audio, VAD) do backend quyết: OpenAI gắn vào token lúc cấp; ElevenLabs
//!   nằm trong `url` trả cho app (app chỉ thêm `token`).
//! - Key lỗi lúc cấp ⟹ 403 `USER_KEY_FAILED`, key bị đánh dấu.

use crate::audit;
use crate::auth::UserId;
use crate::config::{Provider, Vendor};
use crate::error::AppError;
use crate::models::realtime_token::{self, TokenDoc};
use crate::models::{usage, user};
use crate::state::AppState;
use crate::transcribe::{elevenlabs, openai};
use crate::user_keys::{self, Use};
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
/// Số giây app được stream bằng một token (`reservedSeconds`), bằng hạn token OpenAI.
pub const TOKEN_SECONDS: i64 = 600;
/// Sau khi token hết hạn, app còn chừng này giây để báo số giây đã stream.
pub const REPORT_GRACE_S: i64 = 7_200;
/// App gửi PCM16 mono 24 kHz cho cả hai nhà cung cấp (ElevenLabs nhận `pcm_24000`; OpenAI chỉ nhận 24 kHz).
pub const SAMPLE_RATE: i64 = 24_000;
/// Im lặng bao lâu thì OpenAI chốt một câu.
const OPENAI_SILENCE_MS: i64 = 500;
/// Im lặng bao lâu (giây) thì ElevenLabs chốt một câu (VAD phía ElevenLabs, nhận 0,3–3).
const ELEVENLABS_SILENCE_S: &str = "1.0";

/// Số token tối đa mỗi phút mỗi tài khoản (không có sổ nào giữ chỗ để hãm): một phiên ghi xin token mới
/// khoảng 10 phút một lần, nối lại khi mất mạng cũng xin token, nên 10 lần/phút là dư cho người dùng thật.
const OWN_TOKENS_PER_MINUTE: u32 = 10;

/// Còn được cấp token bằng key riêng trong phút này không.
fn own_token_allowed(rate: &crate::rate_limit::RateLimiter, user_id: &str) -> bool {
    rate.allow(&format!("realtime-own:{user_id}"), OWN_TOKENS_PER_MINUTE)
}

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
    /// ElevenLabs báo `quota_exceeded` trên WebSocket của token này: key của người dùng hết credit.
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

/// Xin token tạm thời của OpenAI. Lỗi theo `openai::api_error` (key hỏng ⟹ `user_keys::problem_of`).
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

/// `POST /api/realtime/token` → `{token, tokenId, expiresAt, model, reservedSeconds, provider, url, sampleRate}`. Token cấp
/// bằng key riêng của người dùng cho chép trực tiếp (ElevenLabs hoặc OpenAI); không có ⟹ 403 `NO_USER_KEY`.
pub async fn token(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Json(req): Json<TokenReq>,
) -> Result<Json<Value>, AppError> {
    let session_id = req.session_id.filter(|s| !s.is_empty());
    if session_id.as_deref().is_some_and(|s| !valid_id(s)) {
        return Err(AppError::BadRequest("phiên ghi không hợp lệ".into()));
    }
    let (own, _) = user_keys::required(&state, &user.0, Use::Live).await?;
    let profile = user::find_profile(&state.db, &user.0)
        .await?
        .ok_or_else(|| AppError::Unauthorized("user không tồn tại".into()))?;
    if profile.blocked {
        return Err(AppError::Forbidden("tài khoản đã bị khoá".into()));
    }
    if !own_token_allowed(&state.rate, &user.0) {
        return Err(AppError::TooManyRequests("xin token chép trực tiếp quá nhanh, thử lại sau một phút".into()));
    }
    let provider = if own == Vendor::ElevenLabs { Provider::ElevenLabs } else { Provider::OpenAi };
    let key = user_keys::open(&state, &user.0, own).await.map_err(user_keys::user_key_error)?;
    let minted = match provider {
        Provider::OpenAi => mint_openai(&state.http, key.value()).await,
        Provider::ElevenLabs => mint_elevenlabs(&state.http, ELEVENLABS_API, key.value()).await,
    };
    let minted = match minted {
        Ok(minted) => minted,
        Err(e) => {
            if let Some(problem) = user_keys::problem_of(&e) {
                return Err(user_keys::user_key_error(user_keys::report(&state, &key, problem).await));
            }
            tracing::warn!("cấp token chép trực tiếp bằng key riêng lỗi: {e}");
            return Err(AppError::Unavailable(MINT_FAILED.into()));
        }
    };
    let key_nonce = realtime_token::nonce_hex(key.nonce());
    drop(key);
    let doc = TokenDoc {
        id: uuid::Uuid::new_v4().to_string(),
        user_id: user.0.clone(),
        session_id: session_id.clone(),
        month: usage::month_key(chrono::Utc::now()),
        settled: false,
        created_at: DateTime::now(),
        report_by: DateTime::from_millis((minted.expires_at + REPORT_GRACE_S) * 1000),
        provider: provider.code().into(),
        byok: Some(own.code().into()),
        key_nonce: Some(key_nonce),
    };
    realtime_token::insert(&state.db, &doc).await?;
    let details = doc! { "tokenId": &doc.id, "sessionId": session_id, "seconds": TOKEN_SECONDS, "provider": &doc.provider, "byok": true };
    audit::record(&state, Some(&user.0), "realtime.token", details);
    Ok(Json(token_reply(&minted, &doc.id, TOKEN_SECONDS, provider)))
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

/// Nonce của bản key riêng đã cấp token (token cũ thời key server hay chưa có trường này: `None`).
fn own_key_nonce(t: &TokenDoc) -> Option<Vec<u8>> {
    t.byok.as_ref()?;
    realtime_token::nonce_from_hex(t.key_nonce.as_deref()?)
}

/// Kết quả một lần báo số giây.
#[derive(Debug, PartialEq)]
pub enum Report {
    /// Đã ghi sổ: giây tính thật.
    Settled { sent: i64 },
    /// Token đã ghi sổ trước đó (app gửi lại): không đổi gì.
    Already,
    /// Quá hạn báo: không ghi nữa.
    TooLate,
    NotFound,
}

/// Giây ghi cho một báo cáo: làm tròn lên, không quá thời gian thật từ lúc cấp token (+5 giây lệch
/// đồng hồ), vì không thể stream nhanh hơn thời gian thực.
pub fn billable(seconds: f64, created: DateTime, now: DateTime) -> i64 {
    let wall = (now.timestamp_millis() - created.timestamp_millis()).max(0) / 1000 + 5;
    (seconds.max(0.0).ceil() as i64).min(wall)
}

/// Đóng token `token_id` của `user_id` theo `seconds` app báo: ghi số giây vào sổ phút key riêng của người dùng và sổ chi
/// tiết. Token cũ thời key server chỉ được đóng.
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
        // Một báo cáo khác vừa nhận quyền ghi sổ (hoặc vừa quá hạn).
        let again = realtime_token::find_owned(db, user_id, token_id).await?;
        return Ok(if again.is_some_and(|t| t.settled) { Report::Already } else { Report::TooLate });
    }
    let Some(own) = &t.byok else { return Ok(Report::Settled { sent }) };
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
    Ok(Report::Settled { sent })
}

/// `POST /api/realtime/usage {tokenId, seconds, quotaExceeded?}` — app báo số giây đã stream bằng một token.
/// `quotaExceeded` (ElevenLabs báo hết credit trên WebSocket) đánh dấu đúng bản key riêng đã cấp token.
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
        let nonce = owned.filter(|t| t.provider == "elevenlabs").as_ref().and_then(own_key_nonce);
        if let Some(nonce) = nonce {
            user_keys::report_nonce(&state, &user.0, Vendor::ElevenLabs, &nonce, user_keys::Problem::NoCredit).await;
        }
    }
    match settle_report(&state.db, &user.0, &req.token_id, req.seconds, DateTime::now()).await? {
        // `chargedSeconds` giữ cho app cũ, luôn 0 (không giữ chỗ gì).
        Report::Settled { sent } => Ok(Json(json!({ "settled": true, "chargedSeconds": 0, "seconds": sent }))),
        Report::Already => Ok(Json(json!({ "settled": true, "already": true }))),
        Report::TooLate => Err(AppError::Coded {
            status: StatusCode::CONFLICT,
            code: "REPORT_TOO_LATE",
            message: "Đã quá hạn báo số giây của token này.".into(),
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
            settled: false,
            created_at: DateTime::from_millis(0),
            report_by: DateTime::from_millis(0),
            provider: "elevenlabs".into(),
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

    // Một token chỉ stream trong hạn của chính nó (OpenAI 600 giây, ElevenLabs 900 giây).
    const _: () = assert!(TOKEN_SECONDS == TOKEN_TTL_S && TOKEN_SECONDS < ELEVENLABS_TOKEN_TTL_S);

    /// Không có key riêng cho chép trực tiếp (tính năng Key riêng tắt ⟹ không đọc Mongo): 403 `NO_USER_KEY`, không cấp
    /// token.
    #[tokio::test]
    async fn without_an_own_live_key_no_token_is_minted() {
        let db = mongodb::Client::with_uri_str("mongodb://127.0.0.1:9/?serverSelectionTimeoutMS=200&connectTimeoutMS=200")
            .await
            .expect("uri")
            .database("scribe_realtime_test");
        let state = AppState::for_tests(crate::config::Config::for_tests("http://localhost", "http://localhost"), db);
        let req = TokenReq { session_id: Some("s-1".into()) };
        let Err(AppError::Coded { status, code, message }) =
            token(State(state), Extension(UserId("u1".into())), Json(req)).await
        else {
            panic!("phải từ chối");
        };
        assert_eq!((status, code), (StatusCode::FORBIDDEN, "NO_USER_KEY"));
        assert!(message.contains("Key riêng"), "{message}");
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
            settled: false,
            created_at: DateTime::now(),
            report_by: DateTime::now(),
            provider: "elevenlabs".into(),
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
    /// realtime_reports -- --ignored`. Báo số giây ghi đúng một lần vào sổ phút key riêng; token cũ thời key server chỉ
    /// được đóng. User giả riêng, tự dọn.
    #[tokio::test]
    #[ignore]
    async fn realtime_reports_are_recorded_once_on_real_mongo() {
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        let user = format!("rt-{}", uuid::Uuid::new_v4().simple());
        let month = usage::month_key(chrono::Utc::now());
        let created = DateTime::from_millis(DateTime::now().timestamp_millis() - 700_000);
        let later = DateTime::from_millis(DateTime::now().timestamp_millis() + 3_600_000);
        let token = |id: &str, report_by: DateTime| TokenDoc {
            id: id.into(),
            user_id: user.clone(),
            session_id: None,
            month: month.clone(),
            settled: false,
            created_at: created,
            report_by,
            provider: "openai".into(),
            byok: Some("openai".into()),
            key_nonce: None,
        };
        let (id_a, id_late) = (format!("{user}-a"), format!("{user}-late"));
        realtime_token::insert(&db, &token(&id_a, later)).await.expect("ghi");
        realtime_token::insert(&db, &token(&id_late, DateTime::from_millis(0))).await.expect("ghi");
        // Token cũ thời key server (có trường sổ server, không có `byok`).
        let id_old = format!("{user}-old");
        db.collection::<mongodb::bson::Document>("realtime_tokens")
            .insert_one(doc! { "_id": &id_old, "userId": &user, "month": "2026-10", "freePool": true, "chargedS": 600_i64,
                "settled": false, "createdAt": created, "reportBy": later, "provider": "elevenlabs", "costRatio": 1.7,
                "elRatio": 1.7, "elPeriod": "2026-10" })
            .await
            .expect("token cũ");

        // Báo 250,4 giây ⟹ ghi 251; báo lại không đổi gì.
        let now = DateTime::now();
        assert_eq!(settle_report(&db, &user, &id_a, 250.4, now).await.expect("báo"), Report::Settled { sent: 251 });
        assert_eq!(settle_report(&db, &user, &id_a, 250.4, now).await.expect("báo"), Report::Already);
        assert_eq!(settle_report(&db, &user, &id_late, 10.0, now).await.expect("báo"), Report::TooLate);
        assert_eq!(settle_report(&db, "other", &id_a, 10.0, now).await.expect("báo"), Report::NotFound);
        assert_eq!(settle_report(&db, &user, &id_old, 30.0, now).await.expect("báo"), Report::Settled { sent: 30 });
        assert_eq!(usage::byok_seconds(&db, &user, &month).await.expect("sổ"), 251, "token cũ không vào sổ key riêng");
        let runs = db.collection::<mongodb::bson::Document>(usage::RUNS_COLL).count_documents(doc! { "userId": &user }).await.expect("đếm");
        assert_eq!(runs, 1);

        db.collection::<mongodb::bson::Document>("realtime_tokens").delete_many(doc! { "userId": &user }).await.expect("dọn");
        db.collection::<mongodb::bson::Document>(usage::RUNS_COLL).delete_many(doc! { "userId": &user }).await.expect("dọn");
        db.collection::<mongodb::bson::Document>("transcribe_usage").delete_many(doc! { "_id": format!("b:{user}:{month}") }).await.expect("dọn");
    }
}
