// Bake secret/config build-time → cargo:rustc-env (server chạy KHÔNG cần env runtime).
// CHỈ set khi env tồn tại lúc build → máy dev thiếu secret vẫn `cargo check` được (đọc qua
// option_env! + fallback ở config.rs). Secret NGOÀI (Mongo URI) nhúng lúc build production.
fn main() {
    for key in ["MONGO_URI", "PORT", "ALLOWED_DOMAINS", "ALLOWED_EMAILS", "TRANSCRIBE_EMAILS", "REALTIME_EMAILS", "UNVERIFIED_EMAIL_SUBS", "SESSION_KEY_PATH",
        "DATA_DIR", "DB_NAME", "PUBLIC_URL", "OIDC_ISSUER", "OIDC_WEB_CLIENT_ID",
        "OIDC_WEB_CLIENT_SECRET_PATH", "OIDC_MOBILE_CLIENT_ID", "OIDC_MOBILE_REDIRECT_URI", "OPENAI_KEY_PATH", "TRANSCRIBE_PROVIDER", "REALTIME_PROVIDER", "ELEVENLABS_KEY_PATH", "ELEVENLABS_MONTHLY_HOURS", "TRANSCRIBE_FREE_MINUTES",
        "TRANSCRIBE_PLUS_MINUTES", "TRANSCRIBE_FREE_POOL_USD", "TRANSCRIBE_MONTHLY_CAP_USD", "TRANSCRIBE_OPEN",
        "TRANSCRIBE_FREE_DAILY_MINUTES", "TRANSCRIBE_FREE_DELAY_HOURS",
        "NOTIFY_BOT_API", "NOTIFY_BOT_TOKEN_PATH", "NOTIFY_CHAT_ID",
        "SUMMARY_FREE_DAILY", "SUMMARY_PAID_DAILY", "SUMMARY_FREE_MONTHLY_MINUTES",
        "SUMMARY_PAID_MONTHLY_MINUTES", "SUMMARY_MODEL", "ASK_FREE_DAILY", "ASK_PAID_DAILY", "TRANSLATE_FREE_DAILY", "TRANSLATE_PAID_DAILY", "CHAT_LINK_POLL",
        "CHAT_BOT_USERNAME", "LLM_PROVIDER", "LLM_MODEL", "GEMINI_KEY_PATH", "ANTHROPIC_KEY_PATH", "USER_KEYS_SECRET_PATH",
        "IDP_DISPLAY_NAME"] {
        if let Ok(v) = std::env::var(key) {
            println!("cargo:rustc-env={key}={v}");
        }
        println!("cargo:rerun-if-env-changed={key}");
    }
}
