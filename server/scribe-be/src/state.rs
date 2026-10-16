//! State dùng chung cho handlers (Clone rẻ — mọi field Arc/handle nội bộ).

use crate::config::Config;
use crate::jwks::JwksCache;
use crate::rate_limit::RateLimiter;
use crate::transcribe::PendingKeys;
use mongodb::Database;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: Database,
    pub http: reqwest::Client,
    pub jwks: Arc<JwksCache>,
    /// Session signing key (HS256) — sinh runtime, KHÔNG bake (red-team #4).
    pub session_key: Arc<Vec<u8>>,
    /// Rate-limit theo IP (in-memory, red-team #2).
    pub rate: Arc<RateLimiter>,
    /// API key OpenAI của người dùng đang chờ chép lời — chỉ trong RAM.
    pub transcribe_keys: Arc<PendingKeys>,
}
