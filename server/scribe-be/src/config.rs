//! Cấu hình đọc từ biến nhúng compile-time (`option_env!`) + fallback dev.
//!
//! [Red-team #13] Dùng `option_env!` (KHÔNG `env!`) → máy dev thiếu secret vẫn `cargo check`/build
//! được (fallback về localhost). Production: `build.rs` bake giá trị thật lúc build.

/// Issuer hợp lệ của Firebase ID token cho một project.
pub fn firebase_issuer(project_id: &str) -> String {
    format!("https://securetoken.google.com/{project_id}")
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
    /// Tên database Mongo của app.
    pub db_name: String,
    /// Firebase project ID (công khai): aud/iss khi verify ID token.
    pub firebase_project_id: String,
    /// Allowlist domain (rỗng = cho mọi email đã verified — user chốt mở).
    pub allowed_domains: Vec<String>,
    /// Allowlist email cụ thể (rỗng = không lọc theo email).
    pub allowed_emails: Vec<String>,
    /// Email được dùng chép lời bằng OpenAI (rỗng = tắt tính năng với mọi người).
    pub transcribe_emails: Vec<String>,
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
    /// Đọc cấu hình nhúng lúc build. Thiếu biến bắt buộc thì trả lỗi liệt kê đủ tên biến, để server
    /// dừng ngay khi khởi động thay vì chạy với database hay thư mục sai.
    pub fn from_env() -> Result<Self, String> {
        let required = [
            ("DB_NAME", option_env!("DB_NAME")),
            ("FIREBASE_PROJECT_ID", option_env!("FIREBASE_PROJECT_ID")),
            ("SESSION_KEY_PATH", option_env!("SESSION_KEY_PATH")),
            ("DATA_DIR", option_env!("DATA_DIR")),
        ];
        let missing: Vec<&str> =
            required.iter().filter(|(_, v)| v.is_none_or(str::is_empty)).map(|(k, _)| *k).collect();
        if !missing.is_empty() {
            return Err(format!("thiếu biến build bắt buộc: {}", missing.join(", ")));
        }
        let get = |i: usize| required[i].1.unwrap_or_default().to_string();
        Ok(Config {
            port: option_env!("PORT").and_then(|p| p.parse().ok()).unwrap_or(8787),
            mongo_uri: option_env!("MONGO_URI")
                .unwrap_or("mongodb://127.0.0.1:27017")
                .to_string(),
            db_name: get(0),
            firebase_project_id: get(1),
            allowed_domains: parse_csv(option_env!("ALLOWED_DOMAINS")),
            allowed_emails: parse_csv(option_env!("ALLOWED_EMAILS")),
            transcribe_emails: parse_csv(option_env!("TRANSCRIBE_EMAILS")),
            session_ttl_secs: 60 * 60 * 12, // 12h — TTL ngắn, FE re-login qua Firebase khi hết.
            session_key_path: get(2),
            data_dir: get(3),
            ffmpeg_path: option_env!("FFMPEG_PATH").unwrap_or("ffmpeg").to_string(),
            ffprobe_path: option_env!("FFPROBE_PATH").unwrap_or("ffprobe").to_string(),
        })
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

impl Config {
    /// Email có được chép lời không. Khác `email_allowed`: danh sách rỗng nghĩa là KHÔNG ai được.
    pub fn transcribe_allowed(&self, email: &str) -> bool {
        !email.is_empty() && self.transcribe_emails.iter().any(|e| e.eq_ignore_ascii_case(email))
    }
}

fn parse_csv(v: Option<&str>) -> Vec<String> {
    v.unwrap_or("")
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}
