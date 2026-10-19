//! Kiểm key lúc lưu bằng một lần gọi rẻ, không tốn credit:
//! - ElevenLabs: `POST /v1/single-use-token/realtime_scribe` (cấp token chép trực tiếp dùng một lần; cần quyền
//!   Speech-to-Text, không cần quyền đọc tài khoản; token không dùng thì tự hết hạn sau 15 phút);
//! - OpenAI: `GET /v1/models`;
//! - Gemini: `GET /v1beta/models?pageSize=1`;
//! - Anthropic: `GET /v1/models?limit=1`.
//!
//! Key chỉ đi qua header. Thông điệp của nhà cung cấp không được trả cho người dùng hay ghi log (có thể nêu tài khoản).

use crate::config::Vendor;
use std::time::Duration;

/// Gốc API của từng nhà cung cấp (test thay bằng máy chủ giả).
pub struct Bases<'a> {
    pub elevenlabs: &'a str,
    pub openai: &'a str,
    pub gemini: &'a str,
    pub anthropic: &'a str,
}

pub const LIVE: Bases<'static> = Bases {
    elevenlabs: "https://api.elevenlabs.io",
    openai: "https://api.openai.com",
    gemini: "https://generativelanguage.googleapis.com",
    anthropic: "https://api.anthropic.com",
};

/// Kết quả kiểm.
#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    Valid,
    /// Nhà cung cấp từ chối key (sai, thu hồi, thiếu quyền).
    Invalid,
    /// Key đúng nhưng tài khoản hết credit.
    NoCredit,
    /// Không kiểm được lúc này (mạng, nhà cung cấp lỗi): chưa lưu, người dùng thử lại.
    Unreachable,
}

/// Dạng key: một chuỗi ASCII liền 20–300 ký tự (key OpenAI còn phải bắt đầu bằng `sk-`). `Err` là câu cho người dùng.
pub fn check_format(provider: Vendor, key: &str) -> Result<(), &'static str> {
    if key.len() < 20 || key.len() > 300 || !key.chars().all(|c| c.is_ascii_graphic()) {
        return Err("Key không đúng dạng: dán nguyên key (một chuỗi liền, không khoảng trắng).");
    }
    if provider == Vendor::OpenAi && key.starts_with("sk-ant-") {
        return Err("Đây là key Anthropic, hãy dán vào ô Anthropic Claude.");
    }
    if provider == Vendor::OpenAi && !key.starts_with("sk-") {
        return Err("Key OpenAI bắt đầu bằng “sk-”.");
    }
    Ok(())
}

/// Gọi nhà cung cấp để kiểm `key`.
pub async fn check(http: &reqwest::Client, bases: &Bases<'_>, provider: Vendor, key: &str) -> Verdict {
    let request = match provider {
        Vendor::ElevenLabs => http
            .post(format!("{}/v1/single-use-token/realtime_scribe", bases.elevenlabs))
            .header("xi-api-key", key)
            .header(reqwest::header::CONTENT_LENGTH, "0"),
        Vendor::OpenAi => http.get(format!("{}/v1/models", bases.openai)).bearer_auth(key),
        Vendor::Gemini => http.get(format!("{}/v1beta/models?pageSize=1", bases.gemini)).header("x-goog-api-key", key),
        Vendor::Anthropic => http
            .get(format!("{}/v1/models?limit=1", bases.anthropic))
            .header("x-api-key", key)
            .header("anthropic-version", crate::summary::llm::ANTHROPIC_VERSION),
    };
    let response = match request.timeout(Duration::from_secs(15)).send().await {
        Ok(r) => r,
        Err(_) => return Verdict::Unreachable,
    };
    let status = response.status().as_u16();
    let body = response.text().await.unwrap_or_default();
    verdict(provider, status, &body)
}

fn verdict(provider: Vendor, status: u16, body: &str) -> Verdict {
    if (200..300).contains(&status) {
        return Verdict::Valid;
    }
    let lower = body.to_ascii_lowercase();
    let quota = lower.contains("quota_exceeded") || lower.contains("insufficient_credits") || lower.contains("insufficient_quota");
    match (provider, status) {
        (_, 402) => Verdict::NoCredit,
        (Vendor::ElevenLabs | Vendor::OpenAi, 401 | 429) if quota => Verdict::NoCredit,
        (_, 401 | 403) => Verdict::Invalid,
        (Vendor::Gemini, 400) if lower.contains("api_key_invalid") || lower.contains("api key not valid") => Verdict::Invalid,
        _ => {
            tracing::warn!(provider = provider.code(), status, "kiểm key riêng: nhà cung cấp trả lỗi không rõ");
            Verdict::Unreachable
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::{get, post};
    use axum::Router;

    #[test]
    fn format_rules() {
        assert!(check_format(Vendor::OpenAi, "sk-test-0123456789abcdef").is_ok());
        assert!(check_format(Vendor::OpenAi, "test-0123456789abcdefgh").is_err());
        assert!(check_format(Vendor::OpenAi, "sk-ant-test-0123456789abcdef").is_err());
        assert!(check_format(Vendor::Anthropic, "sk-ant-test-0123456789abcdef").is_ok());
        assert!(check_format(Vendor::Gemini, "short").is_err());
        assert!(check_format(Vendor::ElevenLabs, "sk_test 0123456789abcdef").is_err());
        assert!(check_format(Vendor::ElevenLabs, &"a".repeat(301)).is_err());
    }

    #[test]
    fn verdict_per_provider() {
        assert_eq!(verdict(Vendor::OpenAi, 200, "{}"), Verdict::Valid);
        assert_eq!(verdict(Vendor::OpenAi, 401, r#"{"error":{"code":"invalid_api_key"}}"#), Verdict::Invalid);
        assert_eq!(verdict(Vendor::ElevenLabs, 401, r#"{"detail":{"status":"missing_permissions"}}"#), Verdict::Invalid);
        assert_eq!(verdict(Vendor::ElevenLabs, 402, r#"{"detail":{"code":"insufficient_credits"}}"#), Verdict::NoCredit);
        assert_eq!(verdict(Vendor::Gemini, 400, r#"{"error":{"details":[{"reason":"API_KEY_INVALID"}]}}"#), Verdict::Invalid);
        assert_eq!(verdict(Vendor::Gemini, 400, r#"{"error":{"message":"other"}}"#), Verdict::Unreachable);
        assert_eq!(verdict(Vendor::Anthropic, 403, "{}"), Verdict::Invalid);
        assert_eq!(verdict(Vendor::Anthropic, 500, "{}"), Verdict::Unreachable);
    }

    fn reply(good: bool) -> (StatusCode, &'static str) {
        if good {
            (StatusCode::OK, r#"{"data":[]}"#)
        } else {
            (StatusCode::UNAUTHORIZED, "{}")
        }
    }

    fn header(h: &HeaderMap, name: &str) -> Option<String> {
        h.get(name).and_then(|v| v.to_str().ok()).map(str::to_string)
    }

    /// Máy chủ giả cho cả bốn nhà cung cấp: đúng key ⟹ 200, sai ⟹ 401; kiểm header từng nhà cung cấp.
    #[tokio::test]
    async fn checks_each_provider_with_its_header() {
        let app = Router::new()
            .route(
                "/v1/single-use-token/realtime_scribe",
                post(|h: HeaderMap| async move { reply(header(&h, "xi-api-key").as_deref() == Some("sk_test_elevenlabs_key")) }),
            )
            .route(
                "/v1/models",
                get(|h: HeaderMap| async move {
                    let openai = header(&h, "authorization").as_deref() == Some("Bearer sk-test-openai-key-1");
                    let claude =
                        header(&h, "x-api-key").as_deref() == Some("sk-ant-test-key-12345") && header(&h, "anthropic-version").is_some();
                    reply(openai || claude)
                }),
            )
            .route(
                "/v1beta/models",
                get(|h: HeaderMap| async move { reply(header(&h, "x-goog-api-key").as_deref() == Some("gm-test-key-0123456789")) }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base = format!("http://{}", listener.local_addr().expect("addr"));
        tokio::spawn(async move { axum::serve(listener, app).await });
        let bases = Bases { elevenlabs: &base, openai: &base, gemini: &base, anthropic: &base };
        let http = reqwest::Client::new();
        for (provider, good) in [
            (Vendor::ElevenLabs, "sk_test_elevenlabs_key"),
            (Vendor::OpenAi, "sk-test-openai-key-1"),
            (Vendor::Gemini, "gm-test-key-0123456789"),
            (Vendor::Anthropic, "sk-ant-test-key-12345"),
        ] {
            assert_eq!(check(&http, &bases, provider, good).await, Verdict::Valid, "{provider:?}");
            assert_eq!(check(&http, &bases, provider, "sk-test-wrong-key-000").await, Verdict::Invalid, "{provider:?}");
        }
        let closed = "http://127.0.0.1:1";
        let nowhere = Bases { elevenlabs: closed, openai: closed, gemini: closed, anthropic: closed };
        assert_eq!(check(&http, &nowhere, Vendor::OpenAi, "sk-test-openai-key-1").await, Verdict::Unreachable);
    }

    /// Kiểm thật với nhà cung cấp (cần mạng, không tốn credit): `USER_KEY_TEST_OPENAI_FILE=<tệp key>
    /// USER_KEY_TEST_ELEVENLABS_FILE=<tệp key> cargo test live_key_check -- --ignored`. Key ElevenLabs chỉ có quyền Speech
    /// to Text vẫn phải hợp lệ; key sai bị từ chối. Không in key.
    #[tokio::test]
    #[ignore]
    async fn live_key_check_accepts_real_keys_and_rejects_wrong_ones() {
        let http = reqwest::Client::new();
        for (provider, var) in [(Vendor::OpenAi, "USER_KEY_TEST_OPENAI_FILE"), (Vendor::ElevenLabs, "USER_KEY_TEST_ELEVENLABS_FILE")] {
            let path = std::env::var(var).unwrap_or_else(|_| panic!("{var}"));
            let key = zeroize::Zeroizing::new(std::fs::read_to_string(path).expect("đọc key").trim().to_string());
            assert!(check_format(provider, &key).is_ok(), "{provider:?} đúng dạng");
            assert_eq!(check(&http, &LIVE, provider, &key).await, Verdict::Valid, "{provider:?}");
            let wrong = if provider == Vendor::OpenAi { "sk-test-not-a-real-key-000000" } else { "sk_test_not_a_real_key_000000" };
            assert_eq!(check(&http, &LIVE, provider, wrong).await, Verdict::Invalid, "{provider:?} key sai");
        }
    }
}
