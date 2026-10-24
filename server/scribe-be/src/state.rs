//! State dùng chung cho handlers (Clone rẻ — mọi field Arc/handle nội bộ).

use crate::config::Config;
use crate::auth::SessionCache;
use crate::oidc_registry::OidcRegistry;
use crate::rate_limit::RateLimiter;
use crate::transcribe::Transcriber;
use mongodb::Database;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: Database,
    pub http: reqwest::Client,
    /// Client OIDC của hoandat-auth theo tenant (discovery + JWKS cache) và secret client web của từng tenant (tenant
    /// chính: đọc từ tệp lúc khởi động, không nhúng binary).
    pub oidc: Arc<OidcRegistry>,
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
    /// Host → tenant (tenant chính tĩnh, tenant sổ tra IdP có cache) và tên tài khoản đăng nhập của tenant chính.
    pub tenants: Arc<crate::tenant::directory::TenantDirectory>,
}

#[cfg(test)]
impl AppState {
    /// State cho test router: cấu hình `Config::for_tests`, không key nào, Mongo `db` (lazy, chỉ nối khi route đụng
    /// tới), danh bạ tenant `tenants`.
    pub fn for_tests(db: Database, tenants: crate::tenant::directory::TenantDirectory) -> AppState {
        let config = Config::for_tests();
        let oidc = OidcRegistry::new(crate::oidc::Oidc::new(&config.oidc_issuer), "scribe-web", "test-secret".into(), None);
        AppState::for_tests_with(config, db, tenants, oidc)
    }

    /// Như [`AppState::for_tests`] với cấu hình và client OIDC riêng.
    pub fn for_tests_with(
        config: Config,
        db: Database,
        tenants: crate::tenant::directory::TenantDirectory,
        oidc: OidcRegistry,
    ) -> AppState {
        AppState {
            transcriber: Arc::new(Transcriber::load(&config)),
            oidc: Arc::new(oidc),
            config: Arc::new(config),
            db,
            http: reqwest::Client::new(),
            session_cache: Arc::default(),
            session_key: Arc::new(vec![7; 32]),
            rate: Arc::new(RateLimiter::new()),
            summaries: Arc::default(),
            chat_bot: Arc::default(),
            user_keys: None,
            tenants: Arc::new(tenants),
        }
    }
}
