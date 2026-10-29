//! Cấu hình đọc từ biến nhúng compile-time (`option_env!`) + fallback dev.
//!
//! [Red-team #13] Dùng `option_env!` (KHÔNG `env!`) → máy dev thiếu secret vẫn `cargo check`/build
//! được (fallback về localhost). Production: `build.rs` bake giá trị thật lúc build.

const MIB: u64 = 1024 * 1024;
/// Dung lượng theo gói (chủ dự án chốt 17/10/2026). Gói free: tổng 200 MB, mỗi tệp tối đa 200 MB, tệp tự
/// xoá sau 7 ngày. Gói plus giữ mức cũ: tổng 5 GB, mỗi tệp tối đa 2 GB, 14 ngày. Áp qua `plan::Plan::storage`.
pub const FREE_QUOTA_BYTES: u64 = 200 * MIB;
pub const FREE_MAX_FILE_BYTES: u64 = 200 * MIB;
pub const FREE_RETENTION_DAYS: i64 = 7;
pub const PLUS_QUOTA_BYTES: u64 = 5 * 1024 * MIB;
pub const PLUS_MAX_FILE_BYTES: u64 = 2 * 1024 * MIB;
/// Hạn giữ tệp dài nhất (gói plus, 14 ngày): doc cũ thiếu `expiresAt` và job cũ dùng mốc này. Red-team #1:
/// sweep là nguồn xoá DUY NHẤT (KHÔNG dùng TTL index native → tránh Mongo xoá doc out-of-band làm mồ côi
/// file disk).
pub const RETENTION_DAYS: i64 = 14;

/// Nhà cung cấp chép lời theo tệp (`TRANSCRIBE_PROVIDER`, chủ dự án chốt 17/10/2026): ElevenLabs Scribe v2 mặc định,
/// OpenAI giữ làm dự phòng một bản phát hành. Chép trực tiếp trên iPhone chọn riêng bằng `REALTIME_PROVIDER` (chủ dự án
/// chốt 19/10/2026: ElevenLabs Scribe v2 Realtime mặc định, OpenAI Realtime dự phòng một bản phát hành).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Provider {
    #[default]
    ElevenLabs,
    OpenAi,
}

impl Provider {
    /// `openai` (không phân biệt hoa thường) ⟹ OpenAI; còn lại (kể cả không đặt) ⟹ ElevenLabs.
    pub fn parse(raw: Option<&str>) -> Provider {
        match raw.map(str::trim) {
            Some(v) if v.eq_ignore_ascii_case("openai") => Provider::OpenAi,
            _ => Provider::ElevenLabs,
        }
    }

    /// Nhà cung cấp của bản build này.
    pub fn from_build() -> Provider {
        Provider::parse(option_env!("TRANSCRIBE_PROVIDER"))
    }

    /// Giá một phút audio gửi đi (USD): Scribe v2 0,22 USD/giờ, `gpt-4o-mini-transcribe` 0,003 USD/phút.
    pub fn usd_per_minute(self) -> f64 {
        match self {
            Provider::ElevenLabs => 0.22 / 60.0,
            Provider::OpenAi => 0.003,
        }
    }

    /// Tên model ghi vào lượt chép.
    pub fn model(self) -> &'static str {
        match self {
            Provider::ElevenLabs => "scribe_v2",
            Provider::OpenAi => "gpt-4o-mini-transcribe",
        }
    }

    /// Nhà cung cấp chép trực tiếp của bản build này (`REALTIME_PROVIDER`).
    pub fn realtime_from_build() -> Provider {
        Provider::parse(option_env!("REALTIME_PROVIDER"))
    }

    /// Giá một phút chép trực tiếp (USD): Scribe v2 Realtime 0,39 USD/giờ, OpenAI Realtime với
    /// `gpt-4o-mini-transcribe` 0,003 USD/phút.
    pub fn realtime_usd_per_minute(self) -> f64 {
        match self {
            Provider::ElevenLabs => 0.39 / 60.0,
            Provider::OpenAi => 0.003,
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

/// Nhà cung cấp mà người dùng có thể nhập key riêng (`user_keys`), cũng là nhà cung cấp model chữ của server
/// (`LLM_PROVIDER`: OpenAI, Gemini hoặc Anthropic). Chủ dự án chốt 19/10/2026.
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

    /// Model chữ (biên bản, hỏi đáp, dịch) chạy được bằng key của nhà cung cấp này.
    pub fn has_text(self) -> bool {
        matches!(self, Vendor::OpenAi | Vendor::Gemini | Vendor::Anthropic)
    }
}

/// Nhà cung cấp model chữ của server (`LLM_PROVIDER`): `gemini`, `anthropic`; còn lại (kể cả không đặt) là OpenAI.
fn llm_provider(raw: Option<&str>) -> Vendor {
    match raw.map(str::trim).and_then(|v| Vendor::parse(&v.to_ascii_lowercase())) {
        Some(v) if v.has_text() => v,
        _ => Vendor::OpenAi,
    }
}

/// Model chữ được đặt cho server: `LLM_MODEL`, rồi `SUMMARY_MODEL` (tên cũ, chỉ khi nhà cung cấp là OpenAI).
fn configured_llm_model(provider: Vendor, llm_model: Option<&str>, summary_model: Option<&str>) -> Option<String> {
    let pick = |v: Option<&str>| v.map(str::trim).filter(|m| !m.is_empty()).map(str::to_string);
    pick(llm_model).or_else(|| pick(summary_model).filter(|_| provider == Vendor::OpenAi))
}

/// Model chữ của server: model được đặt nếu có trong bảng giá (`summary::llm::MODELS`), không thì model mặc định của nhà
/// cung cấp — chi phí quy ra trần luôn theo đúng giá của model đang chạy. `main` ghi lỗi lúc khởi động khi model được đặt
/// bị bỏ ([`unlisted_llm_model`]).
fn llm_model(provider: Vendor, llm_model: Option<&str>, summary_model: Option<&str>) -> String {
    configured_llm_model(provider, llm_model, summary_model)
        .filter(|m| crate::summary::llm::is_listed(provider, m))
        .unwrap_or_else(|| crate::summary::llm::default_model(provider).to_string())
}

/// Model chữ đặt qua biến build nhưng không có trong bảng giá (đã đổi về model mặc định), để ghi lỗi lúc khởi động.
pub fn unlisted_llm_model() -> Option<String> {
    let provider = llm_provider(option_env!("LLM_PROVIDER"));
    configured_llm_model(provider, option_env!("LLM_MODEL"), option_env!("SUMMARY_MODEL"))
        .filter(|m| !crate::summary::llm::is_listed(provider, m))
}

/// Nhà cung cấp và model chữ của bản build này.
pub fn llm_from_build() -> (Vendor, String) {
    let provider = llm_provider(option_env!("LLM_PROVIDER"));
    (provider, llm_model(provider, option_env!("LLM_MODEL"), option_env!("SUMMARY_MODEL")))
}

/// Trần giờ audio ElevenLabs mỗi tháng (giờ Việt Nam), mặc định 25 giờ: gói Starter ($6/tháng) có khoảng 27 giờ Scribe
/// v2, chừa biên. Độc lập với trần USD (trần USD còn gồm chi phí tóm tắt, hỏi đáp, dịch của OpenAI).
const DEFAULT_ELEVENLABS_MONTHLY_HOURS: f64 = 25.0;

/// `ELEVENLABS_MONTHLY_HOURS` (giờ, số thực ≥ 0) ra giây; sai dạng hoặc âm thì dùng mặc định.
fn elevenlabs_monthly_seconds(raw: Option<&str>) -> i64 {
    let hours = raw.and_then(|v| v.trim().parse::<f64>().ok()).filter(|h| h.is_finite() && *h >= 0.0);
    (hours.unwrap_or(DEFAULT_ELEVENLABS_MONTHLY_HOURS) * 3600.0).round() as i64
}

/// `ELEVENLABS_RESET_DAY` (1–28): ngày trong tháng ElevenLabs gia hạn credit; sổ giờ ElevenLabs và dấu hết quota tính
/// theo kỳ bắt đầu từ ngày đó (giờ Việt Nam). Thiếu, sai dạng hoặc ngoài 1–28 thì là 1 (theo tháng dương lịch).
fn elevenlabs_reset_day(raw: Option<&str>) -> u32 {
    raw.and_then(|v| v.trim().parse::<u32>().ok()).filter(|d| (1..=28).contains(d)).unwrap_or(1)
}

/// Số lượt chép lời chạy cùng lúc, mặc định 3 (chủ dự án chốt 27/10/2026 theo đo đạc: máy .65 còn dư RAM/CPU, gói
/// ElevenLabs Starter cho 12 request đồng thời). Trần 12 theo gói đó; 1 = chạy lần lượt như trước.
const DEFAULT_TRANSCRIBE_CONCURRENCY: usize = 3;
const MAX_TRANSCRIBE_CONCURRENCY: usize = 12;

/// `TRANSCRIBE_CONCURRENCY` (số nguyên 1..=12); sai dạng hoặc 0 thì dùng mặc định, lớn hơn trần thì lấy trần.
fn transcribe_concurrency(raw: Option<&str>) -> usize {
    raw.and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|n| *n >= 1)
        .map_or(DEFAULT_TRANSCRIBE_CONCURRENCY, |n| n.min(MAX_TRANSCRIBE_CONCURRENCY))
}

/// Giá một phút chép lời của bản build này (USD) — đổi trần chi phí ra giây và quy chi phí tóm tắt ra giây chép lời.
pub fn usd_per_minute() -> f64 {
    Provider::from_build().usd_per_minute()
}
/// Mặc định hạn mức (người dùng duyệt 2026-10-16 theo gợi ý của Jev); đổi bằng biến build cùng tên.
const DEFAULT_FREE_MINUTES: i64 = 120;
const DEFAULT_PLUS_MINUTES: i64 = 3000;
/// Quỹ chung của user gói free; trần tổng chặn mọi người (người dùng chốt 2026-10-16).
const DEFAULT_FREE_POOL_USD: f64 = 10.0;
const DEFAULT_MONTHLY_CAP_USD: f64 = 30.0;
/// Chống lạm dụng gói free (người dùng chốt 2026-10-16; bỏ giới hạn theo IP vì người dùng 4G chung IP
/// nhà mạng): phút mỗi ngày mỗi tài khoản và số giờ tài khoản mới phải chờ trước khi chép lời. Chủ dự án bỏ thời gian
/// chờ 19/10/2026: mặc định 0 (không chờ); `TRANSCRIBE_FREE_DELAY_HOURS` vẫn đặt lại được.
const DEFAULT_FREE_DAILY_MINUTES: i64 = 30;
const DEFAULT_FREE_DELAY_HOURS: i64 = 0;

/// Tóm tắt (biên bản): lượt mỗi ngày giờ Việt Nam (Jev chốt 16/10/2026) và model mặc định.
const DEFAULT_SUMMARY_FREE_DAILY: i64 = 10;
const DEFAULT_SUMMARY_PAID_DAILY: i64 = 50;
/// Trần chi phí tóm tắt mỗi tháng của một tài khoản, tính bằng phút chép lời tương đương: một người không
/// tiêu hết trần tổng của mọi người. 60 phút ≈ $0,18 ≈ vài chục biên bản thường.
const DEFAULT_SUMMARY_FREE_MONTHLY_MINUTES: i64 = 60;
const DEFAULT_SUMMARY_PAID_MONTHLY_MINUTES: i64 = 600;
/// Hỏi đáp trên bản chép lời: lượt mỗi ngày giờ Việt Nam (Jev chốt 16/10/2026). Trần chi phí tháng của
/// tài khoản dùng chung với tóm tắt.
const DEFAULT_ASK_DAILY: (i64, i64) = (30, 200);
/// Dịch lời chép: lượt mỗi ngày giờ Việt Nam (Jev chốt 17/10/2026). Trần chi phí tháng của tài khoản dùng
/// chung với tóm tắt và hỏi đáp.
const DEFAULT_TRANSLATE_DAILY: (i64, i64) = (3, 20);

/// Lượt mỗi ngày (giờ Việt Nam) theo gói của một tính năng gọi model chữ có sổ ngày riêng (hỏi đáp, dịch).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DailyLimits {
    pub free_daily: i64,
    pub paid_daily: i64,
}

impl DailyLimits {
    /// `defaults` = (free, trả phí). Sai dạng hoặc âm thì dùng mặc định.
    fn from_parts(free: Option<&str>, paid: Option<&str>, defaults: (i64, i64)) -> Self {
        let num = |v: Option<&str>, default: i64| v.and_then(|s| s.trim().parse().ok()).filter(|n: &i64| *n >= 0).unwrap_or(default);
        DailyLimits { free_daily: num(free, defaults.0), paid_daily: num(paid, defaults.1) }
    }
}

/// Hạn mức tóm tắt của một tài khoản theo gói: lượt mỗi ngày (giờ Việt Nam) và trần chi phí mỗi tháng
/// (giây chép lời tương đương).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SummaryLimits {
    pub free_daily: i64,
    pub paid_daily: i64,
    pub free_monthly_seconds: i64,
    pub paid_monthly_seconds: i64,
}

impl SummaryLimits {
    /// Thứ tự: lượt ngày free, lượt ngày trả phí, phút tháng free, phút tháng trả phí. Sai dạng hoặc âm
    /// thì dùng mặc định.
    fn from_parts(parts: [Option<&str>; 4]) -> Self {
        let num = |v: Option<&str>, default: i64| v.and_then(|s| s.trim().parse().ok()).filter(|n: &i64| *n >= 0).unwrap_or(default);
        let [free, paid, free_month, paid_month] = parts;
        SummaryLimits {
            free_daily: num(free, DEFAULT_SUMMARY_FREE_DAILY),
            paid_daily: num(paid, DEFAULT_SUMMARY_PAID_DAILY),
            free_monthly_seconds: num(free_month, DEFAULT_SUMMARY_FREE_MONTHLY_MINUTES).saturating_mul(60),
            paid_monthly_seconds: num(paid_month, DEFAULT_SUMMARY_PAID_MONTHLY_MINUTES).saturating_mul(60),
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
        usd_per_minute: f64,
    ) -> Self {
        let minutes =
            |v: Option<&str>, default: i64| v.and_then(|s| s.trim().parse().ok()).filter(|m| *m >= 0).unwrap_or(default);
        let usd_seconds = |v: Option<&str>, default: f64| {
            let usd = v.and_then(|s| s.trim().parse::<f64>().ok()).filter(|c| *c >= 0.0).unwrap_or(default);
            (usd / usd_per_minute * 60.0).round() as i64
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
    /// Tài khoản IdP (`sub`) được đăng nhập dù email chưa xác minh (`UNVERIFIED_EMAIL_SUBS`), cho
    /// tài khoản duyệt App Store của hoandat-auth: theo `sub` chứ không theo email, vì email chưa
    /// xác minh thì ai cũng có thể khai.
    pub unverified_email_subs: Vec<String>,
    /// Email được dùng chép lời bằng OpenAI (rỗng = tắt tính năng với mọi người).
    pub transcribe_emails: Vec<String>,
    /// Email được dùng chép trực tiếp trên iPhone (`REALTIME_EMAILS`; rỗng = không ai). Không theo
    /// `TRANSCRIBE_OPEN`: chỉ mở cho người đã được thêm tên.
    pub realtime_emails: Vec<String>,
    /// Tệp chứa key OpenAI của chủ sản phẩm (đọc lúc khởi động; không nhúng key vào binary).
    pub openai_key_path: Option<String>,
    /// Nhà cung cấp chép lời theo tệp (`TRANSCRIBE_PROVIDER`).
    pub transcribe_provider: Provider,
    /// Tệp chứa key ElevenLabs của chủ sản phẩm (`ELEVENLABS_KEY_PATH`, đọc lúc khởi động).
    pub elevenlabs_key_path: Option<String>,
    /// Trần giây audio ElevenLabs mỗi tháng (`ELEVENLABS_MONTHLY_HOURS`).
    pub elevenlabs_monthly_seconds: i64,
    /// Ngày gia hạn credit ElevenLabs mỗi tháng (`ELEVENLABS_RESET_DAY`): mốc kỳ của sổ giờ ElevenLabs.
    pub elevenlabs_reset_day: u32,
    /// Số lượt chép lời chạy cùng lúc (`TRANSCRIBE_CONCURRENCY`).
    pub transcribe_concurrency: usize,
    /// Nhà cung cấp chép trực tiếp trên iPhone (`REALTIME_PROVIDER`).
    pub realtime_provider: Provider,
    /// `TRANSCRIBE_OPEN=1`: mọi user đăng nhập được chép lời (trong hạn mức gói), không chỉ `TRANSCRIBE_EMAILS`.
    pub transcribe_open: bool,
    pub transcribe_limits: TranscribeLimits,
    /// Báo chủ sản phẩm qua bot chat.hoandat.com: gốc Bot API, tệp token bot (600) và chat id. Thiếu token
    /// hoặc chat id thì chỉ log.
    pub notify_bot_api: String,
    pub notify_bot_token_path: Option<String>,
    pub notify_chat_id: Option<String>,
    pub summary_limits: SummaryLimits,
    pub ask_limits: DailyLimits,
    pub translate_limits: DailyLimits,
    /// Nhà cung cấp model chữ của server cho biên bản, hỏi đáp và dịch (`LLM_PROVIDER`, mặc định OpenAI).
    pub llm_provider: Vendor,
    /// Model chữ của server (`LLM_MODEL`; `SUMMARY_MODEL` cũ vẫn nhận khi nhà cung cấp là OpenAI). Chỉ nhận model có
    /// trong bảng giá ở `summary::llm` (giá quy ra trần chi phí); model khác thì dùng model mặc định và ghi lỗi lúc khởi động.
    pub summary_model: String,
    /// Tệp key Gemini / Anthropic của server, chỉ đọc khi `LLM_PROVIDER` là nhà cung cấp đó.
    pub gemini_key_path: Option<String>,
    pub anthropic_key_path: Option<String>,
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
            realtime_emails: parse_csv(option_env!("REALTIME_EMAILS")),
            unverified_email_subs: parse_csv(option_env!("UNVERIFIED_EMAIL_SUBS")),
            openai_key_path: option_env!("OPENAI_KEY_PATH").filter(|p| !p.is_empty()).map(str::to_string),
            transcribe_provider: Provider::from_build(),
            elevenlabs_key_path: option_env!("ELEVENLABS_KEY_PATH").filter(|p| !p.is_empty()).map(str::to_string),
            elevenlabs_monthly_seconds: elevenlabs_monthly_seconds(option_env!("ELEVENLABS_MONTHLY_HOURS")),
            elevenlabs_reset_day: elevenlabs_reset_day(option_env!("ELEVENLABS_RESET_DAY")),
            transcribe_concurrency: transcribe_concurrency(option_env!("TRANSCRIBE_CONCURRENCY")),
            realtime_provider: Provider::realtime_from_build(),
            transcribe_open: matches!(option_env!("TRANSCRIBE_OPEN"), Some("1" | "true")),
            transcribe_limits: TranscribeLimits::from_parts(
                option_env!("TRANSCRIBE_FREE_MINUTES"),
                option_env!("TRANSCRIBE_PLUS_MINUTES"),
                option_env!("TRANSCRIBE_FREE_POOL_USD"),
                option_env!("TRANSCRIBE_MONTHLY_CAP_USD"),
                usd_per_minute(),
            )
            .with_free_guards(
                option_env!("TRANSCRIBE_FREE_DAILY_MINUTES"),
                option_env!("TRANSCRIBE_FREE_DELAY_HOURS"),
            ),
            notify_bot_api: option_env!("NOTIFY_BOT_API").unwrap_or("https://chat.hoandat.com/bot/v1").trim_end_matches('/').to_string(),
            notify_bot_token_path: option_env!("NOTIFY_BOT_TOKEN_PATH").filter(|p| !p.is_empty()).map(str::to_string),
            notify_chat_id: option_env!("NOTIFY_CHAT_ID").filter(|c| !c.is_empty()).map(str::to_string),
            summary_limits: SummaryLimits::from_parts([
                option_env!("SUMMARY_FREE_DAILY"),
                option_env!("SUMMARY_PAID_DAILY"),
                option_env!("SUMMARY_FREE_MONTHLY_MINUTES"),
                option_env!("SUMMARY_PAID_MONTHLY_MINUTES"),
            ]),
            ask_limits: DailyLimits::from_parts(option_env!("ASK_FREE_DAILY"), option_env!("ASK_PAID_DAILY"), DEFAULT_ASK_DAILY),
            translate_limits: DailyLimits::from_parts(
                option_env!("TRANSLATE_FREE_DAILY"),
                option_env!("TRANSLATE_PAID_DAILY"),
                DEFAULT_TRANSLATE_DAILY,
            ),
            llm_provider: llm_from_build().0,
            summary_model: llm_from_build().1,
            gemini_key_path: option_env!("GEMINI_KEY_PATH").filter(|p| !p.is_empty()).map(str::to_string),
            anthropic_key_path: option_env!("ANTHROPIC_KEY_PATH").filter(|p| !p.is_empty()).map(str::to_string),
            user_keys_secret_path: option_env!("USER_KEYS_SECRET_PATH").filter(|p| !p.is_empty()).map(str::to_string),
            idp_display_name: crate::idp_brand::build_display_name(option_env!("IDP_DISPLAY_NAME")),
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

impl Config {
    /// Email có được chép lời không. Khác `email_allowed`: danh sách rỗng nghĩa là KHÔNG ai được, trừ
    /// khi bật `TRANSCRIBE_OPEN` (mọi user đăng nhập, hạn mức gói vẫn áp).
    pub fn transcribe_allowed(&self, email: &str) -> bool {
        !email.is_empty()
            && (self.transcribe_open || self.transcribe_emails.iter().any(|e| e.eq_ignore_ascii_case(email)))
    }
}

impl Config {
    /// Email có được chép trực tiếp không: chỉ email có tên trong `REALTIME_EMAILS`.
    pub fn realtime_allowed(&self, email: &str) -> bool {
        listed(&self.realtime_emails, email)
    }
}

/// `email` có trong danh sách không (không phân biệt hoa thường; email rỗng không bao giờ có).
fn listed(list: &[String], email: &str) -> bool {
    !email.is_empty() && list.iter().any(|e| e.eq_ignore_ascii_case(email))
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
    fn limits_default_to_the_approved_numbers_and_ignore_bad_values() {
        let openai = Provider::OpenAi.usd_per_minute();
        let d = TranscribeLimits::from_parts(None, None, None, None, openai);
        assert_eq!((d.free_seconds, d.plus_seconds), (120 * 60, 3000 * 60));
        // $10 / $0.003 mỗi phút ≈ 3333 phút; $30 ≈ 10000 phút.
        assert_eq!((d.free_pool_seconds, d.monthly_cap_seconds), (200_000, 600_000));
        let c = TranscribeLimits::from_parts(Some("60"), Some("x"), Some("-5"), Some("15"), openai);
        assert_eq!((c.free_seconds, c.plus_seconds), (3600, 3000 * 60));
        assert_eq!((c.free_pool_seconds, c.monthly_cap_seconds), (200_000, 300_000));
        assert_eq!((d.free_daily_seconds, d.free_delay_hours), (1800, 0));
        // Biến build vẫn đặt lại được thời gian chờ.
        let g = d.with_free_guards(Some("10"), Some("24"));
        assert_eq!((g.free_daily_seconds, g.free_delay_hours), (600, 24));
        assert_eq!(d.with_free_guards(Some("bad"), Some("-1")).free_daily_seconds, 1800);
    }

    #[test]
    fn provider_defaults_to_elevenlabs_and_sets_the_price() {
        assert_eq!(Provider::parse(None), Provider::ElevenLabs);
        assert_eq!(Provider::parse(Some("elevenlabs")), Provider::ElevenLabs);
        assert_eq!(Provider::parse(Some(" OpenAI ")), Provider::OpenAi);
        assert_eq!(Provider::parse(Some("khác")), Provider::ElevenLabs);
        // 0,22 USD mỗi giờ audio.
        assert!((Provider::ElevenLabs.usd_per_minute() * 60.0 - 0.22).abs() < 1e-12);
        assert_eq!(Provider::OpenAi.usd_per_minute(), 0.003);
        // Trần $30 với ElevenLabs ≈ 8182 phút; quỹ free $10 ≈ 2727 phút.
        let d = TranscribeLimits::from_parts(None, None, None, None, Provider::ElevenLabs.usd_per_minute());
        assert_eq!((d.free_pool_seconds, d.monthly_cap_seconds), (163_636, 490_909));
        assert_eq!((Provider::ElevenLabs.model(), Provider::OpenAi.model()), ("scribe_v2", "gpt-4o-mini-transcribe"));
        // Chép trực tiếp: 0,39 USD mỗi giờ với Scribe v2 Realtime.
        assert!((Provider::ElevenLabs.realtime_usd_per_minute() * 60.0 - 0.39).abs() < 1e-12);
        assert_eq!(Provider::OpenAi.realtime_usd_per_minute(), 0.003);
        assert_eq!(Provider::ElevenLabs.realtime_model(), "scribe_v2_realtime");
        assert_eq!(elevenlabs_monthly_seconds(None), 90_000);
        assert_eq!(elevenlabs_monthly_seconds(Some(" 10.5 ")), 37_800);
        assert_eq!(elevenlabs_monthly_seconds(Some("-1")), 90_000);
        assert_eq!(elevenlabs_monthly_seconds(Some("x")), 90_000);
        assert_eq!(elevenlabs_monthly_seconds(Some("0")), 0);
        assert_eq!(elevenlabs_reset_day(None), 1);
        assert_eq!(elevenlabs_reset_day(Some(" 17 ")), 17);
        assert_eq!(elevenlabs_reset_day(Some("0")), 1);
        assert_eq!(elevenlabs_reset_day(Some("31")), 1);
        assert_eq!(elevenlabs_reset_day(Some("x")), 1);
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
    fn realtime_list_is_closed_when_empty() {
        let list = parse_csv(Some(" Owner@Example.com , "));
        assert!(listed(&list, "owner@example.com"));
        assert!(!listed(&list, "other@example.com"));
        assert!(!listed(&list, ""));
        assert!(!listed(&parse_csv(Some("")), "owner@example.com"));
    }

    #[test]
    fn unverified_email_subs_match_the_exact_sub_only() {
        let list = parse_csv(Some(" acc_Review1 ,"));
        assert!(sub_listed(&list, "acc_Review1"));
        assert!(!sub_listed(&list, "acc_review1"));
        assert!(!sub_listed(&list, ""));
        assert!(!sub_listed(&parse_csv(None), "acc_Review1"));
    }

    #[test]
    fn llm_provider_and_model_default_to_openai_mini() {
        assert_eq!(llm_provider(None), Vendor::OpenAi);
        assert_eq!(llm_provider(Some(" Gemini ")), Vendor::Gemini);
        assert_eq!(llm_provider(Some("anthropic")), Vendor::Anthropic);
        // ElevenLabs không có model chữ.
        assert_eq!(llm_provider(Some("elevenlabs")), Vendor::OpenAi);
        assert_eq!(llm_model(Vendor::OpenAi, None, None), "gpt-4.1-mini");
        assert_eq!(llm_model(Vendor::OpenAi, None, Some("gpt-4.1-nano")), "gpt-4.1-nano");
        // `SUMMARY_MODEL` là model OpenAI: không áp cho nhà cung cấp khác.
        assert_eq!(llm_model(Vendor::Anthropic, None, Some("gpt-4.1-nano")), "claude-haiku-5-5");
        assert_eq!(llm_model(Vendor::Gemini, Some(" gemini-3.5-flash-lite "), None), "gemini-3.5-flash-lite");
        // Model ngoài bảng giá (vd Claude Opus đắt gấp ~40 lần Haiku) không chạy với giá sai: về model mặc định.
        assert_eq!(llm_model(Vendor::Anthropic, Some("claude-opus-5-5"), None), "claude-haiku-5-5");
        assert_eq!(llm_model(Vendor::OpenAi, None, Some("gpt-9")), "gpt-4.1-mini");
        assert_eq!(configured_llm_model(Vendor::Anthropic, Some("claude-opus-5-5"), None).as_deref(), Some("claude-opus-5-5"));
        assert_eq!(Vendor::parse("gemini"), Some(Vendor::Gemini));
        assert_eq!(Vendor::parse("Gemini"), None);
        assert!(Vendor::ALL.iter().all(|v| Vendor::parse(v.code()) == Some(*v)));
        assert_eq!(Provider::ElevenLabs.code(), "elevenlabs");
    }

    #[test]
    fn daily_limits_default_per_feature() {
        let ask = |free, paid| DailyLimits::from_parts(free, paid, DEFAULT_ASK_DAILY);
        assert_eq!(ask(None, None), DailyLimits { free_daily: 30, paid_daily: 200 });
        assert_eq!(ask(Some(" 5 "), Some("-1")), DailyLimits { free_daily: 5, paid_daily: 200 });
        let translate = DailyLimits::from_parts(None, Some("x"), DEFAULT_TRANSLATE_DAILY);
        assert_eq!(translate, DailyLimits { free_daily: 3, paid_daily: 20 });
    }

    #[test]
    fn summary_limits_default_to_ten_and_fifty() {
        let d = SummaryLimits::from_parts([None; 4]);
        assert_eq!((d.free_daily, d.paid_daily), (10, 50));
        assert_eq!((d.free_monthly_seconds, d.paid_monthly_seconds), (3_600, 36_000));
        let s = SummaryLimits::from_parts([Some(" 3 "), Some("-1"), Some("5"), Some("x")]);
        assert_eq!(s, SummaryLimits { free_daily: 3, paid_daily: 50, free_monthly_seconds: 300, paid_monthly_seconds: 36_000 });
        assert_eq!(SummaryLimits::from_parts([Some("x"), Some("0"), None, None]).paid_daily, 0);
    }
}
