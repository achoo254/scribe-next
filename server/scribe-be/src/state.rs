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
}
