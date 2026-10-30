//! State dùng chung cho handlers (Clone rẻ — mọi field Arc/handle nội bộ).

use crate::config::Config;
use crate::auth::SessionCache;
use crate::oidc::Oidc;
use crate::rate_limit::RateLimiter;
use crate::transcribe::Transcriber;
use mongodb::Database;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: Database,
    pub http: reqwest::Client,
    /// Client OIDC của hoandat-auth (discovery + JWKS cache).
    pub oidc: Arc<Oidc>,
    /// Secret client web (đọc từ tệp lúc khởi động, không nhúng binary).
    pub oidc_web_secret: Arc<String>,
    pub session_cache: Arc<SessionCache>,
    /// Khoá 32 byte sinh lúc chạy (tệp 600): mã hoá refresh token IdP của thiết bị.
    pub session_key: Arc<Vec<u8>>,
    /// Rate-limit theo IP (in-memory, red-team #2).
    pub rate: Arc<RateLimiter>,
    /// Key OpenAI của server + hàng đợi chép lời.
    pub transcriber: Arc<Transcriber>,
    /// Phiên đang được tóm tắt (một lượt mỗi phiên) và cờ tạm ngưng riêng của tóm tắt.
    pub summaries: Arc<crate::summary::Summaries>,
    /// Username bot chat.hoandat.com (đọc một lần).
    pub chat_bot: Arc<crate::chat_link::BotProfile>,
    /// Bí mật mã hoá key riêng của người dùng (`USER_KEYS_SECRET_PATH`); `None` = tính năng Key riêng tắt.
    pub user_keys: Option<Arc<crate::user_keys::crypto::Secret>>,
}

#[cfg(test)]
impl AppState {
    /// State cho test: `config`, database `db` (có thể chưa kết nối được), mọi phần khác rỗng như lúc chưa cấu hình.
    pub fn for_tests(config: Config, db: Database) -> AppState {
        let transcriber = Transcriber::load(&config);
        AppState {
            oidc: Arc::new(Oidc::new(&config.oidc_issuer)),
            config: Arc::new(config),
            db,
            http: reqwest::Client::new(),
            oidc_web_secret: Arc::new(String::new()),
            session_cache: Arc::default(),
            session_key: Arc::new(vec![7u8; 32]),
            rate: Arc::new(RateLimiter::new()),
            transcriber: Arc::new(transcriber),
            summaries: Arc::default(),
            chat_bot: Arc::default(),
            user_keys: None,
        }
    }
}
