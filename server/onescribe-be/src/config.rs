//! Cấu hình đọc từ biến nhúng compile-time (`option_env!`) + fallback dev.
//!
//! [Red-team #13] Dùng `option_env!` (KHÔNG `env!`) → máy dev thiếu secret vẫn `cargo check`/build
//! được (fallback về localhost). Production: `build.rs` bake giá trị thật lúc build.

/// Firebase project ID — công khai (không phải secret), hardcode: dùng cho aud/iss khi verify token.
pub const FIREBASE_PROJECT_ID: &str = "onescribe-inet";

/// Issuer hợp lệ của Firebase ID token.
pub fn firebase_issuer() -> String {
    format!("https://securetoken.google.com/{FIREBASE_PROJECT_ID}")
}

/// JWK endpoint (securetoken) — public key RS256 để verify Firebase ID token.
pub const FIREBASE_JWKS_URL: &str =
    "https://www.googleapis.com/service_accounts/v1/jwk/securetoken@system.gserviceaccount.com";

/// Size cap mỗi file (2GB — chốt validate 2026-07-11, ~40% quota 5GB). Red-team #14.
pub const MAX_FILE_SIZE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// Quota mỗi user (5GB — chốt validate). Red-team #3.
pub const QUOTA_BYTES: u64 = 5 * 1024 * 1024 * 1024;
/// Retention: file tự xoá sau N ngày (14 — chốt validate). Red-team #1: sweep là nguồn xoá DUY NHẤT
/// (KHÔNG dùng TTL index native → tránh Mongo xoá doc out-of-band làm mồ côi file disk).
pub const RETENTION_DAYS: i64 = 14;

#[derive(Clone, Debug)]
pub struct Config {
    pub port: u16,
    pub mongo_uri: String,
    /// Allowlist domain (rỗng = cho mọi email đã verified — user chốt mở).
    pub allowed_domains: Vec<String>,
    /// Allowlist email cụ thể (rỗng = không lọc theo email).
    pub allowed_emails: Vec<String>,
    /// TTL session cookie (giây).
    pub session_ttl_secs: i64,
    /// File lưu session signing key (tự sinh runtime, KHÔNG bake).
    pub session_key_path: String,
    /// Thư mục lưu upload/output ({data_dir}/{userId}/...).
    pub data_dir: String,
    /// Path ffmpeg/ffprobe (đọc từ config, KHÔNG hard-code — red-team #5). Mặc định PATH.
    pub ffmpeg_path: String,
    pub ffprobe_path: String,
}

impl Config {
    pub fn from_env() -> Self {
        Config {
            port: option_env!("PORT").and_then(|p| p.parse().ok()).unwrap_or(8787),
            mongo_uri: option_env!("MONGO_URI")
                .unwrap_or("mongodb://127.0.0.1:27017")
                .to_string(),
            allowed_domains: parse_csv(option_env!("ALLOWED_DOMAINS")),
            allowed_emails: parse_csv(option_env!("ALLOWED_EMAILS")),
            session_ttl_secs: 60 * 60 * 12, // 12h — TTL ngắn, FE re-login qua Firebase khi hết.
            session_key_path: option_env!("SESSION_KEY_PATH")
                .unwrap_or(default_session_key_path())
                .to_string(),
            data_dir: option_env!("DATA_DIR").unwrap_or(default_data_dir()).to_string(),
            ffmpeg_path: option_env!("FFMPEG_PATH").unwrap_or("ffmpeg").to_string(),
            ffprobe_path: option_env!("FFPROBE_PATH").unwrap_or("ffprobe").to_string(),
        }
    }

    /// Email có được phép không: chỉ gọi SAU khi đã xác nhận `email_verified`.
    /// Rỗng cả hai list = cho phép mọi email verified (chế độ mở user chốt).
    pub fn email_allowed(&self, email: &str) -> bool {
        if self.allowed_domains.is_empty() && self.allowed_emails.is_empty() {
            return true;
        }
        let email_lc = email.to_ascii_lowercase();
        if self.allowed_emails.iter().any(|e| e.eq_ignore_ascii_case(&email_lc)) {
            return true;
        }
        if let Some(domain) = email_lc.rsplit('@').next() {
            if self.allowed_domains.iter().any(|d| d.eq_ignore_ascii_case(domain)) {
                return true;
            }
        }
        false
    }
}

fn parse_csv(v: Option<&str>) -> Vec<String> {
    v.unwrap_or("")
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

const fn default_session_key_path() -> &'static str {
    #[cfg(unix)]
    {
        "/opt/onescribe-be/.session-key"
    }
    #[cfg(not(unix))]
    {
        ".session-key"
    }
}

const fn default_data_dir() -> &'static str {
    #[cfg(unix)]
    {
        "/opt/onescribe-be/data"
    }
    #[cfg(not(unix))]
    {
        "./data"
    }
}
