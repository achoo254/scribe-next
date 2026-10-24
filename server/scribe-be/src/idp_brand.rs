//! `GET /api/idp`: tài khoản đăng nhập của Scribe, công khai (trang đăng nhập chưa có phiên). Tên hiển thị là tên tài
//! khoản đăng nhập (`IDP_DISPLAY_NAME` của bản build, mặc định "Auth Next" như `TENANT_DISPLAY_NAME` mặc định của
//! auth-next): giao diện gọi tài khoản người dùng theo tên này thay vì viết cứng.
//!
//! Shape `{ displayName, domain, issuer, primary, scribe }` giữ nguyên từ thời đa tenant: app iPhone/Android đã phát
//! hành đọc nó. Scribe chỉ còn một tenant nên `primary` luôn `true` và `scribe` luôn `"enabled"`.

use crate::state::AppState;
use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

/// Tên mặc định của tài khoản đăng nhập ở auth-next (`TENANT_DISPLAY_NAME` khi IdP không đặt).
pub const DEFAULT_DISPLAY_NAME: &str = "Auth Next";
const MAX_NAME_CHARS: usize = 64;
/// Nhãn đầu của host Scribe: `scribe.hoandat.com` ⟹ domain `hoandat.com`.
const HOST_PREFIX: &str = "scribe.";

/// Tên dùng được để hiện: bỏ khoảng trắng hai đầu, không rỗng, không ký tự điều khiển, tối đa 64 ký tự.
fn clean_name(raw: &str) -> Option<String> {
    let name = raw.trim();
    let ok = !name.is_empty() && name.chars().count() <= MAX_NAME_CHARS && !name.chars().any(char::is_control);
    ok.then(|| name.to_string())
}

/// Giá trị `IDP_DISPLAY_NAME` của bản build sau khi làm sạch; không đặt hoặc không dùng được ⟹ "Auth Next".
pub fn build_display_name(raw: Option<&str>) -> String {
    raw.and_then(clean_name).unwrap_or_else(|| DEFAULT_DISPLAY_NAME.to_string())
}

/// Domain của Scribe: host của `PUBLIC_URL` bỏ nhãn `scribe.` (`https://scribe.hoandat.com` ⟹ `hoandat.com`).
fn domain_of(public_url: &str) -> String {
    let host = reqwest::Url::parse(public_url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.trim_end_matches('.').to_ascii_lowercase()))
        .unwrap_or_default();
    match host.strip_prefix(HOST_PREFIX) {
        Some(domain) if !domain.is_empty() => domain.to_string(),
        _ => host,
    }
}

/// Thân của `GET /api/idp` dựng từ cấu hình của bản build.
fn body(public_url: &str, issuer: &str, display_name: &str) -> Value {
    json!({
        "displayName": display_name,
        "domain": domain_of(public_url),
        "issuer": issuer.trim_end_matches('/'),
        "primary": true,
        "scribe": "enabled",
    })
}

/// `GET /api/idp` — `{ displayName, domain, issuer, primary: true, scribe: "enabled" }`. Không có gì bí mật.
pub async fn info(State(state): State<AppState>) -> Json<Value> {
    let config = &state.config;
    Json(body(&config.public_url, &config.oidc_issuer, &config.idp_display_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_validated() {
        assert_eq!(clean_name(" Auth Next "), Some("Auth Next".into()));
        assert_eq!(clean_name(""), None);
        assert_eq!(clean_name("a\nb"), None);
        assert_eq!(clean_name(&"x".repeat(65)), None);
        assert_eq!(clean_name(&"đ".repeat(64)), Some("đ".repeat(64)));
        assert_eq!(build_display_name(None), "Auth Next");
        assert_eq!(build_display_name(Some(" ")), "Auth Next");
        assert_eq!(build_display_name(Some("hoandat")), "hoandat");
    }

    /// Đúng từng byte như production trả trước khi bỏ đa tenant, với `IDP_DISPLAY_NAME` không đặt hay đặt "Auth Next":
    /// app di động đã phát hành đọc shape này.
    #[test]
    fn production_body_is_unchanged_byte_for_byte() {
        const PROD: &str = r#"{"displayName":"Auth Next","domain":"hoandat.com","issuer":"https://auth.hoandat.com","primary":true,"scribe":"enabled"}"#;
        for name in [None, Some("Auth Next")] {
            let value = body("https://scribe.hoandat.com", "https://auth.hoandat.com", &build_display_name(name));
            assert_eq!(serde_json::to_string(&value).expect("json"), PROD);
        }
        let trailing = body("https://scribe.hoandat.com/", "https://auth.hoandat.com/", DEFAULT_DISPLAY_NAME);
        assert_eq!(serde_json::to_string(&trailing).expect("json"), PROD);
    }

    #[test]
    fn domain_drops_the_scribe_label() {
        assert_eq!(domain_of("https://Scribe.Hoandat.com"), "hoandat.com");
        assert_eq!(domain_of("http://localhost:5173"), "localhost");
        assert_eq!(domain_of("not a url"), "");
    }
}
