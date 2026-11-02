//! Cấu hình đọc từ biến nhúng compile-time (`option_env!`) + fallback dev.
//!
//! [Red-team #13] Dùng `option_env!` (KHÔNG `env!`) → máy dev thiếu secret vẫn `cargo check`/build
//! được (fallback về localhost). Production: `build.rs` bake giá trị thật lúc build.
//!
//! Không có key AI của server (chủ dự án chốt 02/11/2026: mọi việc AI chạy bằng key riêng của người dùng, `user_keys`).
//! Các biến cũ về key server, nhà cung cấp và hạn mức AI (`OPENAI_KEY_PATH`, `ELEVENLABS_KEY_PATH`, `GEMINI_KEY_PATH`,
//! `ANTHROPIC_KEY_PATH`, `TRANSCRIBE_PROVIDER`, `REALTIME_PROVIDER`, `LLM_*`, `SUMMARY_*`, `ASK_*`, `TRANSLATE_*`,
//! `TRANSCRIBE_*` trừ `TRANSCRIBE_CONCURRENCY`, `ELEVENLABS_*`, `REALTIME_EMAILS`) không còn được đọc: còn đặt lúc build
//! thì bị bỏ qua.

/// Giới hạn lưu trữ (một mức cho mọi người) ở `models::storage::LIMITS`.
///
/// Hạn giữ tệp dài nhất từng có (gói plus cũ, 14 ngày): doc cũ thiếu `expiresAt` và job cũ dùng mốc này. Red-team #1:
/// sweep là nguồn xoá DUY NHẤT (KHÔNG dùng TTL index native → tránh Mongo xoá doc out-of-band làm mồ côi
/// file disk).
pub const RETENTION_DAYS: i64 = 14;

/// Nhà cung cấp chép lời (theo tệp và trực tiếp) bằng key riêng của người dùng: ElevenLabs Scribe v2 (cả tệp hoặc cả
/// phiên iPhone trong một request, có người nói) hoặc OpenAI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    ElevenLabs,
    OpenAi,
}

impl Provider {
    /// Tên model chép lời theo tệp của nhà cung cấp.
    pub fn model(self) -> &'static str {
        match self {
            Provider::ElevenLabs => "scribe_v2",
            Provider::OpenAi => "gpt-4o-mini-transcribe",
        }
    }

    /// Model chép trực tiếp.
    pub fn realtime_model(self) -> &'static str {
        match self {
            Provider::ElevenLabs => "scribe_v2_realtime",
            Provider::OpenAi => "gpt-4o-mini-transcribe",
        }
    }
}

impl Provider {
    /// Mã nhà cung cấp trong API và Mongo (`elevenlabs` | `openai`).
    pub fn code(self) -> &'static str {
        self.vendor().code()
    }

    /// Nhà cung cấp key tương ứng.
    pub fn vendor(self) -> Vendor {
        match self {
            Provider::ElevenLabs => Vendor::ElevenLabs,
            Provider::OpenAi => Vendor::OpenAi,
        }
    }
}

/// Nhà cung cấp mà người dùng có thể nhập key riêng (`user_keys`). Chủ dự án chốt 19/10/2026.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Vendor {
    ElevenLabs,
    OpenAi,
    Gemini,
    Anthropic,
}

impl Vendor {
    pub const ALL: [Vendor; 4] = [Vendor::ElevenLabs, Vendor::OpenAi, Vendor::Gemini, Vendor::Anthropic];

    pub fn code(self) -> &'static str {
        match self {
            Vendor::ElevenLabs => "elevenlabs",
            Vendor::OpenAi => "openai",
            Vendor::Gemini => "gemini",
            Vendor::Anthropic => "anthropic",
        }
    }

    /// Mã đúng từng chữ (chữ thường), như [`Vendor::code`].
    pub fn parse(code: &str) -> Option<Vendor> {
        Vendor::ALL.into_iter().find(|v| v.code() == code)
    }

    /// Tên cho người đọc.
    pub fn label(self) -> &'static str {
        match self {
            Vendor::ElevenLabs => "ElevenLabs",
            Vendor::OpenAi => "OpenAI",
            Vendor::Gemini => "Google Gemini",
            Vendor::Anthropic => "Anthropic Claude",
        }
    }
}

/// Số lượt chép lời chạy cùng lúc, mặc định 3 (chủ dự án chốt 27/10/2026 theo đo đạc: máy .65 còn dư RAM/CPU). Trần
/// 12 (gói ElevenLabs Starter cho 12 request đồng thời); 1 = chạy lần lượt như trước.
const DEFAULT_TRANSCRIBE_CONCURRENCY: usize = 3;
const MAX_TRANSCRIBE_CONCURRENCY: usize = 12;

/// `TRANSCRIBE_CONCURRENCY` (số nguyên 1..=12); sai dạng hoặc 0 thì dùng mặc định, lớn hơn trần thì lấy trần.
fn transcribe_concurrency(raw: Option<&str>) -> usize {
    raw.and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|n| *n >= 1)
        .map_or(DEFAULT_TRANSCRIBE_CONCURRENCY, |n| n.min(MAX_TRANSCRIBE_CONCURRENCY))
}

#[derive(Clone, Debug)]
pub struct Config {
    pub port: u16,
    pub mongo_uri: String,
    /// Tên database Mongo của app.
    pub db_name: String,
    /// Origin của app như trình duyệt thấy (`https://scribe.hoandat.com`, dev `http://localhost:5174`):
    /// redirect OIDC, cookie Secure hay không.
    pub public_url: String,
    pub oidc_issuer: String,
    /// Client confidential của web (`scribe-web`, dev `scribe-web-dev`); secret đọc từ tệp lúc khởi động.
    pub oidc_web_client_id: String,
    pub oidc_web_client_secret_path: String,
    pub oidc_mobile_client_id: String,
    pub oidc_mobile_redirect_uri: String,
    /// Allowlist domain (rỗng = cho mọi email đã verified — user chốt mở).
    pub allowed_domains: Vec<String>,
    /// Allowlist email cụ thể (rỗng = không lọc theo email).
    pub allowed_emails: Vec<String>,
    /// Tài khoản IdP (`sub`) được đăng nhập dù email chưa xác minh (`UNVERIFIED_EMAIL_SUBS`), cho
    /// tài khoản duyệt App Store của hoandat-auth: theo `sub` chứ không theo email, vì email chưa
    /// xác minh thì ai cũng có thể khai.
    pub unverified_email_subs: Vec<String>,
    /// Số lượt chép lời chạy cùng lúc (`TRANSCRIBE_CONCURRENCY`).
    pub transcribe_concurrency: usize,
    /// Báo chủ sản phẩm qua bot chat.hoandat.com: gốc Bot API, tệp token bot (600) và chat id. Thiếu token
    /// hoặc chat id thì chỉ log.
    pub notify_bot_api: String,
    /// `NOTIFY_BOT_RESOLVE`: địa chỉ IP thay DNS cho host của `notify_bot_api` (vd `127.0.0.1` khi chat chạy cùng máy:
    /// đi thẳng nginx nội bộ, không qua Cloudflare; TLS và Host vẫn là tên miền). Trống = phân giải DNS như thường.
    pub notify_bot_resolve: Option<String>,
    pub notify_bot_token_path: Option<String>,
    pub notify_chat_id: Option<String>,
    /// Tệp 32 byte bí mật mã hoá key riêng của người dùng (`USER_KEYS_SECRET_PATH`, 600). Thiếu thì tính năng tắt.
    pub user_keys_secret_path: Option<String>,
    /// Tên tài khoản đăng nhập hiện cho người dùng (`IDP_DISPLAY_NAME`, mặc định "hoandat.com"), trả ở `GET /api/idp`.
    pub idp_display_name: String,
    /// `CHAT_LINK_POLL=1`: chạy task long-poll của bot để liên kết chat. Mặc định tắt: dev và prod dùng
    /// chung token bot, backend dev bật poll sẽ lấy mất update của prod.
    pub chat_link_poll: bool,
    /// Username của bot trên chat.hoandat.com (`CHAT_BOT_USERNAME`); thiếu thì hỏi `/bot/v1/me`.
    pub chat_bot_username: Option<String>,
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
            ("PUBLIC_URL", option_env!("PUBLIC_URL")),
            ("SESSION_KEY_PATH", option_env!("SESSION_KEY_PATH")),
            ("DATA_DIR", option_env!("DATA_DIR")),
            ("OIDC_WEB_CLIENT_SECRET_PATH", option_env!("OIDC_WEB_CLIENT_SECRET_PATH")),
        ];
        let missing: Vec<&str> =
            required.iter().filter(|(_, v)| v.is_none_or(str::is_empty)).map(|(k, _)| *k).collect();
        if !missing.is_empty() {
            return Err(format!("thiếu biến build bắt buộc: {}", missing.join(", ")));
        }
        let get = |i: usize| required[i].1.unwrap_or_default();
        Ok(Self::with_required(get(0), get(1), get(2), get(3), get(4)))
    }

    /// Cấu hình từ các biến bắt buộc đã kiểm (theo thứ tự `DB_NAME`, `PUBLIC_URL`, `SESSION_KEY_PATH`, `DATA_DIR`,
    /// `OIDC_WEB_CLIENT_SECRET_PATH`) cộng các biến tuỳ chọn nhúng lúc build.
    fn with_required(
        db_name: &str,
        public_url: &str,
        session_key_path: &str,
        data_dir: &str,
        oidc_web_client_secret_path: &str,
    ) -> Self {
        Config {
            port: option_env!("PORT").and_then(|p| p.parse().ok()).unwrap_or(8787),
            mongo_uri: option_env!("MONGO_URI")
                .unwrap_or("mongodb://127.0.0.1:27017")
                .to_string(),
            db_name: db_name.to_string(),
            public_url: public_url.trim_end_matches('/').to_string(),
            oidc_issuer: option_env!("OIDC_ISSUER").unwrap_or("https://auth.hoandat.com").to_string(),
            oidc_web_client_id: option_env!("OIDC_WEB_CLIENT_ID").unwrap_or("scribe-web").to_string(),
            oidc_web_client_secret_path: oidc_web_client_secret_path.to_string(),
            oidc_mobile_client_id: option_env!("OIDC_MOBILE_CLIENT_ID").unwrap_or("scribe-mobile").to_string(),
            oidc_mobile_redirect_uri: option_env!("OIDC_MOBILE_REDIRECT_URI")
                .unwrap_or("com.hoandat.scribe:/auth/callback")
                .to_string(),
            allowed_domains: parse_csv(option_env!("ALLOWED_DOMAINS")),
            allowed_emails: parse_csv(option_env!("ALLOWED_EMAILS")),
            unverified_email_subs: parse_csv(option_env!("UNVERIFIED_EMAIL_SUBS")),
            transcribe_concurrency: transcribe_concurrency(option_env!("TRANSCRIBE_CONCURRENCY")),
            notify_bot_api: option_env!("NOTIFY_BOT_API").unwrap_or("https://chat.hoandat.com/bot/v1").trim_end_matches('/').to_string(),
            notify_bot_resolve: option_env!("NOTIFY_BOT_RESOLVE").map(str::trim).filter(|r| !r.is_empty()).map(str::to_string),
            notify_bot_token_path: option_env!("NOTIFY_BOT_TOKEN_PATH").filter(|p| !p.is_empty()).map(str::to_string),
            notify_chat_id: option_env!("NOTIFY_CHAT_ID").filter(|c| !c.is_empty()).map(str::to_string),
            user_keys_secret_path: option_env!("USER_KEYS_SECRET_PATH").filter(|p| !p.is_empty()).map(str::to_string),
            idp_display_name: crate::idp_brand::build_display_name(option_env!("IDP_DISPLAY_NAME")),
            chat_link_poll: matches!(option_env!("CHAT_LINK_POLL"), Some("1" | "true")),
            chat_bot_username: option_env!("CHAT_BOT_USERNAME")
                .map(|u| u.trim().trim_start_matches('@'))
                .filter(|u| !u.is_empty())
                .map(str::to_string),
            session_key_path: session_key_path.to_string(),
            data_dir: data_dir.to_string(),
            ffmpeg_path: option_env!("FFMPEG_PATH").unwrap_or("ffmpeg").to_string(),
            ffprobe_path: option_env!("FFPROBE_PATH").unwrap_or("ffprobe").to_string(),
        }
    }

    /// Cấu hình cho test: origin `public_url`, IdP `oidc_issuer`, đường dẫn tạm; còn lại như bản build không đặt biến.
    #[cfg(test)]
    pub fn for_tests(public_url: &str, oidc_issuer: &str) -> Self {
        let mut config = Self::with_required("scribe_test", public_url, "/nonexistent/session-key", "/nonexistent/data", "/nonexistent/secret");
        config.oidc_issuer = oidc_issuer.trim_end_matches('/').to_string();
        config
    }

    /// Email của tài khoản này dùng được dù IdP chưa xác minh (chỉ `sub` có tên trong danh sách).
    pub fn accepts_unverified_email(&self, sub: &str) -> bool {
        sub_listed(&self.unverified_email_subs, sub)
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

/// `sub` của IdP so khớp tuyệt đối (phân biệt hoa thường), khác email.
fn sub_listed(list: &[String], sub: &str) -> bool {
    !sub.is_empty() && list.iter().any(|s| s == sub)
}

fn parse_csv(v: Option<&str>) -> Vec<String> {
    v.unwrap_or("")
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_models_and_codes() {
        assert_eq!((Provider::ElevenLabs.model(), Provider::OpenAi.model()), ("scribe_v2", "gpt-4o-mini-transcribe"));
        assert_eq!(Provider::ElevenLabs.realtime_model(), "scribe_v2_realtime");
        assert_eq!(Provider::ElevenLabs.code(), "elevenlabs");
        assert_eq!(Vendor::parse("gemini"), Some(Vendor::Gemini));
        assert_eq!(Vendor::parse("Gemini"), None);
        assert!(Vendor::ALL.iter().all(|v| Vendor::parse(v.code()) == Some(*v)));
    }

    #[test]
    fn transcribe_concurrency_defaults_and_caps() {
        assert_eq!(transcribe_concurrency(None), 3);
        assert_eq!(transcribe_concurrency(Some(" 1 ")), 1);
        assert_eq!(transcribe_concurrency(Some("0")), 3);
        assert_eq!(transcribe_concurrency(Some("x")), 3);
        assert_eq!(transcribe_concurrency(Some("50")), 12);
    }

    #[test]
    fn unverified_email_subs_match_the_exact_sub_only() {
        let list = parse_csv(Some(" acc_Review1 ,"));
        assert!(sub_listed(&list, "acc_Review1"));
        assert!(!sub_listed(&list, "acc_review1"));
        assert!(!sub_listed(&list, ""));
        assert!(!sub_listed(&parse_csv(None), "acc_Review1"));
    }
}
