//! State dùng chung cho handlers (Clone rẻ — mọi field Arc/handle nội bộ).

use crate::config::Config;
use crate::jwks::JwksCache;
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
}
