//! Tên hiển thị của tài khoản đăng nhập (thương hiệu tenant của IdP, vd "Auth Next"): giao diện gọi tài khoản
//! người dùng theo tên này thay vì viết cứng. BE hỏi API nội bộ của hoandat-auth
//! (`GET {IDP_INTERNAL_API_URL}/internal/tenants/lookup?host=<host của PUBLIC_URL>`, Basic `scribe:<secret>`)
//! lúc khởi động rồi mỗi giờ, giữ trong bộ nhớ và trả công khai ở `GET /api/idp` (trang đăng nhập chưa có phiên).
//! Lỗi thì giữ giá trị đúng gần nhất; chưa hỏi được lần nào thì dùng `IDP_DISPLAY_NAME` của bản build.

use crate::state::AppState;
use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::RwLock;
use std::time::Duration;

/// Tên mặc định của tenant ở hoandat-auth (`TENANT_DISPLAY_NAME` khi IdP không đặt).
pub const DEFAULT_DISPLAY_NAME: &str = "Auth Next";
/// Tài khoản Basic của Scribe ở API nội bộ của IdP.
const INTERNAL_USER: &str = "scribe";
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(5);
const REFRESH_EVERY: Duration = Duration::from_secs(60 * 60);
/// Lỗi thì hỏi lại sớm hơn: IdP khởi động lại hoặc chưa cấp quyền cho `scribe` lúc deploy.
const RETRY_AFTER_ERROR: Duration = Duration::from_secs(5 * 60);
const MAX_NAME_CHARS: usize = 64;

/// Tên đang dùng (đọc nhiều, ghi mỗi giờ).
pub struct IdpBrand {
    name: RwLock<String>,
}

impl IdpBrand {
    pub fn new(fallback: &str) -> Self {
        Self { name: RwLock::new(fallback.to_string()) }
    }

    pub fn display_name(&self) -> String {
        match self.name.read() {
            Ok(name) => name.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    fn set(&self, name: String) {
        match self.name.write() {
            Ok(mut current) => *current = name,
            Err(poisoned) => *poisoned.into_inner() = name,
        }
    }
}

/// Tên dùng được để hiện: bỏ khoảng trắng hai đầu, không rỗng, không ký tự điều khiển, tối đa 64 ký tự.
fn clean_name(raw: &str) -> Option<String> {
    let name = raw.trim();
    let ok = !name.is_empty() && name.chars().count() <= MAX_NAME_CHARS && !name.chars().any(char::is_control);
    ok.then(|| name.to_string())
}

/// Host (không cổng) của `PUBLIC_URL`: khoá tra tenant ở IdP.
fn host_of(public_url: &str) -> Option<String> {
    reqwest::Url::parse(public_url).ok()?.host_str().map(str::to_ascii_lowercase)
}

/// Bí mật Basic trong tệp (600): một dòng, không khoảng trắng; IdP đòi tối thiểu 32 ký tự.
fn read_secret(path: &str) -> Result<String, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("không đọc được tệp bí mật ({})", e.kind()))?;
    let secret = raw.trim();
    if secret.len() < 32 || secret.chars().any(char::is_whitespace) {
        return Err("tệp bí mật không đúng dạng (một dòng, tối thiểu 32 ký tự)".into());
    }
    Ok(secret.to_string())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TenantSummary {
    display_name: String,
}

/// Hỏi IdP tên của tenant phục vụ `host`. Lỗi trả mô tả không chứa bí mật.
async fn fetch_display_name(http: &reqwest::Client, api_url: &str, host: &str, secret: &str) -> Result<String, String> {
    let url = format!("{}/internal/tenants/lookup", api_url.trim_end_matches('/'));
    let res = http
        .get(url)
        .query(&[("host", host)])
        .basic_auth(INTERNAL_USER, Some(secret))
        .timeout(LOOKUP_TIMEOUT)
        .send()
        .await
        .map_err(|e| format!("không gọi được IdP ({})", if e.is_timeout() { "quá giờ" } else { "lỗi mạng" }))?;
    let status = res.status();
    if !status.is_success() {
        return Err(format!("IdP trả {status}"));
    }
    let body: TenantSummary = res.json().await.map_err(|_| "IdP trả JSON không đúng dạng".to_string())?;
    clean_name(&body.display_name).ok_or_else(|| "IdP trả displayName không dùng được".to_string())
}

/// Lúc khởi động: nếu đủ cấu hình thì đọc bí mật và chạy task nền hỏi tên ngay rồi mỗi giờ.
/// Thiếu cấu hình hoặc tệp bí mật hỏng ⟹ tắt, giữ tên mặc định.
pub fn start(state: &AppState) {
    let config = &state.config;
    let (Some(api_url), Some(secret_path)) = (&config.idp_internal_api_url, &config.idp_internal_api_secret_path) else {
        if config.idp_internal_api_url.is_some() || config.idp_internal_api_secret_path.is_some() {
            tracing::warn!("tên IdP: cần đặt cả IDP_INTERNAL_API_URL và IDP_INTERNAL_API_SECRET_PATH — dùng tên mặc định");
        } else {
            tracing::info!("tên IdP: chưa cấu hình API nội bộ — dùng tên mặc định");
        }
        return;
    };
    let Some(host) = host_of(&config.public_url) else {
        tracing::warn!("tên IdP: PUBLIC_URL không có host — dùng tên mặc định");
        return;
    };
    let secret = match read_secret(secret_path) {
        Ok(secret) => secret,
        Err(e) => {
            tracing::error!(path = %secret_path, "tên IdP: {e} — dùng tên mặc định");
            return;
        }
    };
    // Client riêng không theo redirect: header Basic chỉ đi tới đúng địa chỉ đã cấu hình.
    let http = match reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).build() {
        Ok(http) => http,
        Err(e) => {
            tracing::error!("tên IdP: không dựng được HTTP client: {e}");
            return;
        }
    };
    let api_url = api_url.clone();
    let brand = state.idp_brand.clone();
    tokio::spawn(async move {
        loop {
            let wait = match fetch_display_name(&http, &api_url, &host, &secret).await {
                Ok(name) => {
                    if name != brand.display_name() {
                        tracing::info!(name = %name, "tên IdP cập nhật");
                    }
                    brand.set(name);
                    REFRESH_EVERY
                }
                Err(e) => {
                    tracing::warn!(current = %brand.display_name(), "tên IdP: {e} — giữ tên đang dùng");
                    RETRY_AFTER_ERROR
                }
            };
            tokio::time::sleep(wait).await;
        }
    });
}

/// `GET /api/idp` — công khai: `{ "displayName": "Auth Next" }`.
pub async fn info(State(state): State<AppState>) -> Json<Value> {
    Json(json!({ "displayName": state.idp_brand.display_name() }))
}

/// Giá trị `IDP_DISPLAY_NAME` của bản build sau khi làm sạch; không đặt hoặc không dùng được ⟹ "Auth Next".
pub fn build_display_name(raw: Option<&str>) -> String {
    raw.and_then(clean_name).unwrap_or_else(|| DEFAULT_DISPLAY_NAME.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::Query;
    use axum::http::{HeaderMap, StatusCode};
    use axum::response::IntoResponse;
    use axum::routing::get;
    use axum::Router;
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    use std::collections::HashMap;

    const SECRET: &str = "s3cret-s3cret-s3cret-s3cret-s3cret";

    /// API nội bộ giả: chỉ nhận đúng Basic `scribe:<SECRET>`, trả tên theo host.
    async fn fake_idp(reply: &'static str) -> String {
        let handler = move |headers: HeaderMap, Query(q): Query<HashMap<String, String>>| async move {
            let expected = format!("Basic {}", STANDARD.encode(format!("scribe:{SECRET}")));
            if headers.get("authorization").and_then(|v| v.to_str().ok()) != Some(expected.as_str()) {
                return (StatusCode::UNAUTHORIZED, "{}").into_response();
            }
            match q.get("host").map(String::as_str) {
                Some("scribe.example.com") => {
                    ([("content-type", "application/json")], reply).into_response()
                }
                _ => (StatusCode::NOT_FOUND, "{}").into_response(),
            }
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let api = format!("http://{}", listener.local_addr().expect("addr"));
        let app = Router::new().route("/internal/tenants/lookup", get(handler));
        tokio::spawn(async move { axum::serve(listener, app).await });
        api
    }

    #[tokio::test]
    async fn fetch_sends_basic_auth_and_reads_the_display_name() {
        let api = fake_idp(r#"{"id":"t1","displayName":"  Acme Login ","status":"active"}"#).await;
        let http = reqwest::Client::new();
        assert_eq!(fetch_display_name(&http, &format!("{api}/"), "scribe.example.com", SECRET).await, Ok("Acme Login".into()));
        let wrong = fetch_display_name(&http, &api, "scribe.example.com", "other-secret-other-secret-other-se").await;
        assert_eq!(wrong, Err("IdP trả 401 Unauthorized".into()));
        assert_eq!(fetch_display_name(&http, &api, "unknown.example.com", SECRET).await, Err("IdP trả 404 Not Found".into()));
    }

    #[tokio::test]
    async fn fetch_rejects_a_missing_or_unusable_name() {
        let http = reqwest::Client::new();
        let empty = fake_idp(r#"{"displayName":"   "}"#).await;
        assert!(fetch_display_name(&http, &empty, "scribe.example.com", SECRET).await.is_err());
        let missing = fake_idp(r#"{"id":"t1"}"#).await;
        assert!(fetch_display_name(&http, &missing, "scribe.example.com", SECRET).await.is_err());
        let error = fetch_display_name(&http, "http://127.0.0.1:1", "scribe.example.com", SECRET).await.expect_err("không có máy chủ");
        assert!(!error.contains(SECRET), "lỗi không lộ bí mật");
    }

    #[test]
    fn names_hosts_and_secrets_are_validated() {
        assert_eq!(clean_name(" Auth Next "), Some("Auth Next".into()));
        assert_eq!(clean_name(""), None);
        assert_eq!(clean_name("a\nb"), None);
        assert_eq!(clean_name(&"x".repeat(65)), None);
        assert_eq!(clean_name(&"đ".repeat(64)), Some("đ".repeat(64)));
        assert_eq!(build_display_name(None), "Auth Next");
        assert_eq!(build_display_name(Some(" ")), "Auth Next");
        assert_eq!(build_display_name(Some("hoandat")), "hoandat");
        assert_eq!(host_of("https://Scribe.Example.com"), Some("scribe.example.com".into()));
        assert_eq!(host_of("http://localhost:5174"), Some("localhost".into()));
        assert_eq!(host_of("not a url"), None);

        let dir = std::env::temp_dir().join(format!("scribe-idp-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("tạo thư mục");
        let path = dir.join("secret");
        std::fs::write(&path, format!("{SECRET}\n")).expect("ghi");
        assert_eq!(read_secret(path.to_str().expect("path")), Ok(SECRET.to_string()));
        std::fs::write(&path, "short").expect("ghi");
        assert!(read_secret(path.to_str().expect("path")).is_err());
        assert!(read_secret(dir.join("missing").to_str().expect("path")).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn brand_keeps_the_fallback_until_set() {
        let brand = IdpBrand::new("Auth Next");
        assert_eq!(brand.display_name(), "Auth Next");
        brand.set("Acme".into());
        assert_eq!(brand.display_name(), "Acme");
    }
}
