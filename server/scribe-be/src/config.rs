//! Cấu hình đọc từ biến nhúng compile-time (`option_env!`) + fallback dev.
//!
//! [Red-team #13] Dùng `option_env!` (KHÔNG `env!`) → máy dev thiếu secret vẫn `cargo check`/build
//! được (fallback về localhost). Production: `build.rs` bake giá trị thật lúc build.

/// Size cap mỗi file (2GB — chốt validate 2026-07-11, ~40% quota 5GB). Red-team #14.
pub const MAX_FILE_SIZE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// Quota mỗi user (5GB — chốt validate). Red-team #3.
pub const QUOTA_BYTES: u64 = 5 * 1024 * 1024 * 1024;
/// Retention: file tự xoá sau N ngày (14 — chốt validate). Red-team #1: sweep là nguồn xoá DUY NHẤT
/// (KHÔNG dùng TTL index native → tránh Mongo xoá doc out-of-band làm mồ côi file disk).
pub const RETENTION_DAYS: i64 = 14;

/// Giá `gpt-4o-mini-transcribe` theo phút audio gửi đi (USD) — dùng để đổi trần chi phí ra giây.
pub const USD_PER_MINUTE: f64 = 0.003;
/// Mặc định hạn mức (người dùng duyệt 2026-10-16 theo gợi ý của Jev); đổi bằng biến build cùng tên.
const DEFAULT_FREE_MINUTES: i64 = 120;
const DEFAULT_PLUS_MINUTES: i64 = 3000;
/// Quỹ chung của user gói free; trần tổng chặn mọi người (người dùng chốt 2026-10-16).
const DEFAULT_FREE_POOL_USD: f64 = 10.0;
const DEFAULT_MONTHLY_CAP_USD: f64 = 30.0;
/// Chống lạm dụng gói free (người dùng chốt 2026-10-16; bỏ giới hạn theo IP vì người dùng 4G chung IP
/// nhà mạng): phút mỗi ngày mỗi tài khoản và số giờ tài khoản mới phải chờ trước khi chép lời.
const DEFAULT_FREE_DAILY_MINUTES: i64 = 30;
const DEFAULT_FREE_DELAY_HOURS: i64 = 24;

/// Tóm tắt (biên bản): lượt mỗi ngày giờ Việt Nam (Jev chốt 16/10/2026) và model mặc định.
const DEFAULT_SUMMARY_FREE_DAILY: i64 = 10;
const DEFAULT_SUMMARY_PAID_DAILY: i64 = 50;
const DEFAULT_SUMMARY_MODEL: &str = "gpt-4.1-mini";

/// Lượt tóm tắt mỗi ngày (giờ Việt Nam) của một tài khoản, theo gói.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SummaryLimits {
    pub free_daily: i64,
    pub paid_daily: i64,
}

impl SummaryLimits {
    fn from_parts(free: Option<&str>, paid: Option<&str>) -> Self {
        let num = |v: Option<&str>, default: i64| v.and_then(|s| s.trim().parse().ok()).filter(|n: &i64| *n >= 0).unwrap_or(default);
        SummaryLimits {
            free_daily: num(free, DEFAULT_SUMMARY_FREE_DAILY),
            paid_daily: num(paid, DEFAULT_SUMMARY_PAID_DAILY),
        }
    }
}

/// Hạn mức chép lời, tính bằng giây audio mỗi tháng.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TranscribeLimits {
    pub free_seconds: i64,
    pub plus_seconds: i64,
    /// Quỹ chung của mọi user gói free, đổi ra giây audio.
    pub free_pool_seconds: i64,
    /// Trần chi phí toàn hệ thống (chặn cả gói plus), đổi ra giây audio.
    pub monthly_cap_seconds: i64,
    /// Gói free: giây mỗi ngày (giờ Việt Nam) của một tài khoản. `0` = gói free không chép được.
    pub free_daily_seconds: i64,
    /// Gói free: số giờ từ lúc tạo tài khoản tới khi được chép lời. `0` = không chờ.
    pub free_delay_hours: i64,
}

impl TranscribeLimits {
    fn from_parts(
        free_minutes: Option<&str>,
        plus_minutes: Option<&str>,
        free_pool_usd: Option<&str>,
        cap_usd: Option<&str>,
    ) -> Self {
        let minutes =
            |v: Option<&str>, default: i64| v.and_then(|s| s.trim().parse().ok()).filter(|m| *m >= 0).unwrap_or(default);
        let usd_seconds = |v: Option<&str>, default: f64| {
            let usd = v.and_then(|s| s.trim().parse::<f64>().ok()).filter(|c| *c >= 0.0).unwrap_or(default);
            (usd / USD_PER_MINUTE * 60.0).round() as i64
        };
        TranscribeLimits {
            free_seconds: minutes(free_minutes, DEFAULT_FREE_MINUTES) * 60,
            plus_seconds: minutes(plus_minutes, DEFAULT_PLUS_MINUTES) * 60,
            free_pool_seconds: usd_seconds(free_pool_usd, DEFAULT_FREE_POOL_USD),
            monthly_cap_seconds: usd_seconds(cap_usd, DEFAULT_MONTHLY_CAP_USD),
            free_daily_seconds: DEFAULT_FREE_DAILY_MINUTES * 60,
            free_delay_hours: DEFAULT_FREE_DELAY_HOURS,
        }
    }

    /// Ghi đè các giới hạn chống lạm dụng gói free bằng biến build (giá trị sai/âm thì giữ mặc định).
    fn with_free_guards(self, daily_minutes: Option<&str>, delay_hours: Option<&str>) -> Self {
        let num = |v: Option<&str>, default: i64| v.and_then(|s| s.trim().parse().ok()).filter(|m: &i64| *m >= 0).unwrap_or(default);
        TranscribeLimits {
            free_daily_seconds: num(daily_minutes, DEFAULT_FREE_DAILY_MINUTES) * 60,
            free_delay_hours: num(delay_hours, DEFAULT_FREE_DELAY_HOURS),
            ..self
        }
    }
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
    /// Email được dùng chép lời bằng OpenAI (rỗng = tắt tính năng với mọi người).
    pub transcribe_emails: Vec<String>,
    /// Tệp chứa key OpenAI của chủ sản phẩm (đọc lúc khởi động; không nhúng key vào binary).
    pub openai_key_path: Option<String>,
    /// `TRANSCRIBE_OPEN=1`: mọi user đăng nhập được chép lời (trong hạn mức gói), không chỉ `TRANSCRIBE_EMAILS`.
    pub transcribe_open: bool,
    pub transcribe_limits: TranscribeLimits,
    /// Báo chủ sản phẩm qua bot chat.hoandat.com: gốc Bot API, tệp token bot (600) và chat id. Thiếu token
    /// hoặc chat id thì chỉ log.
    pub notify_bot_api: String,
    pub notify_bot_token_path: Option<String>,
    pub notify_chat_id: Option<String>,
    pub summary_limits: SummaryLimits,
    /// Model OpenAI viết biên bản (`SUMMARY_MODEL`). Giá token để quy ra trần chi phí là giá của
    /// `gpt-4.1-mini` (`summary::cost`): đổi model thì cập nhật giá ở đó.
    pub summary_model: String,
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
        let get = |i: usize| required[i].1.unwrap_or_default().to_string();
        Ok(Config {
            port: option_env!("PORT").and_then(|p| p.parse().ok()).unwrap_or(8787),
            mongo_uri: option_env!("MONGO_URI")
                .unwrap_or("mongodb://127.0.0.1:27017")
                .to_string(),
            db_name: get(0),
            public_url: get(1).trim_end_matches('/').to_string(),
            oidc_issuer: option_env!("OIDC_ISSUER").unwrap_or("https://auth.hoandat.com").to_string(),
            oidc_web_client_id: option_env!("OIDC_WEB_CLIENT_ID").unwrap_or("scribe-web").to_string(),
            oidc_web_client_secret_path: get(4),
            oidc_mobile_client_id: option_env!("OIDC_MOBILE_CLIENT_ID").unwrap_or("scribe-mobile").to_string(),
            oidc_mobile_redirect_uri: option_env!("OIDC_MOBILE_REDIRECT_URI")
                .unwrap_or("com.hoandat.scribe:/auth/callback")
                .to_string(),
            allowed_domains: parse_csv(option_env!("ALLOWED_DOMAINS")),
            allowed_emails: parse_csv(option_env!("ALLOWED_EMAILS")),
            transcribe_emails: parse_csv(option_env!("TRANSCRIBE_EMAILS")),
            openai_key_path: option_env!("OPENAI_KEY_PATH").filter(|p| !p.is_empty()).map(str::to_string),
            transcribe_open: matches!(option_env!("TRANSCRIBE_OPEN"), Some("1" | "true")),
            transcribe_limits: TranscribeLimits::from_parts(
                option_env!("TRANSCRIBE_FREE_MINUTES"),
                option_env!("TRANSCRIBE_PLUS_MINUTES"),
                option_env!("TRANSCRIBE_FREE_POOL_USD"),
                option_env!("TRANSCRIBE_MONTHLY_CAP_USD"),
            )
            .with_free_guards(
                option_env!("TRANSCRIBE_FREE_DAILY_MINUTES"),
                option_env!("TRANSCRIBE_FREE_DELAY_HOURS"),
            ),
            notify_bot_api: option_env!("NOTIFY_BOT_API").unwrap_or("https://chat.hoandat.com/bot/v1").trim_end_matches('/').to_string(),
            notify_bot_token_path: option_env!("NOTIFY_BOT_TOKEN_PATH").filter(|p| !p.is_empty()).map(str::to_string),
            notify_chat_id: option_env!("NOTIFY_CHAT_ID").filter(|c| !c.is_empty()).map(str::to_string),
            summary_limits: SummaryLimits::from_parts(option_env!("SUMMARY_FREE_DAILY"), option_env!("SUMMARY_PAID_DAILY")),
            summary_model: option_env!("SUMMARY_MODEL")
                .map(str::trim)
                .filter(|m| !m.is_empty())
                .unwrap_or(DEFAULT_SUMMARY_MODEL)
                .to_string(),
            chat_link_poll: matches!(option_env!("CHAT_LINK_POLL"), Some("1" | "true")),
            chat_bot_username: option_env!("CHAT_BOT_USERNAME")
                .map(|u| u.trim().trim_start_matches('@'))
                .filter(|u| !u.is_empty())
                .map(str::to_string),
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
    /// Email có được chép lời không. Khác `email_allowed`: danh sách rỗng nghĩa là KHÔNG ai được, trừ
    /// khi bật `TRANSCRIBE_OPEN` (mọi user đăng nhập, hạn mức gói vẫn áp).
    pub fn transcribe_allowed(&self, email: &str) -> bool {
        !email.is_empty()
            && (self.transcribe_open || self.transcribe_emails.iter().any(|e| e.eq_ignore_ascii_case(email)))
    }
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
    fn limits_default_to_the_approved_numbers_and_ignore_bad_values() {
        let d = TranscribeLimits::from_parts(None, None, None, None);
        assert_eq!((d.free_seconds, d.plus_seconds), (120 * 60, 3000 * 60));
        // $10 / $0.003 mỗi phút ≈ 3333 phút; $30 ≈ 10000 phút.
        assert_eq!((d.free_pool_seconds, d.monthly_cap_seconds), (200_000, 600_000));
        let c = TranscribeLimits::from_parts(Some("60"), Some("x"), Some("-5"), Some("15"));
        assert_eq!((c.free_seconds, c.plus_seconds), (3600, 3000 * 60));
        assert_eq!((c.free_pool_seconds, c.monthly_cap_seconds), (200_000, 300_000));
        assert_eq!((d.free_daily_seconds, d.free_delay_hours), (1800, 24));
        let g = d.with_free_guards(Some("10"), Some("0"));
        assert_eq!((g.free_daily_seconds, g.free_delay_hours), (600, 0));
        assert_eq!(d.with_free_guards(Some("bad"), Some("-1")).free_daily_seconds, 1800);
    }

    #[test]
    fn summary_limits_default_to_ten_and_fifty() {
        assert_eq!(SummaryLimits::from_parts(None, None), SummaryLimits { free_daily: 10, paid_daily: 50 });
        assert_eq!(SummaryLimits::from_parts(Some(" 3 "), Some("-1")), SummaryLimits { free_daily: 3, paid_daily: 50 });
        assert_eq!(SummaryLimits::from_parts(Some("x"), Some("0")), SummaryLimits { free_daily: 10, paid_daily: 0 });
    }
}
