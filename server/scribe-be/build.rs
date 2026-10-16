// Bake secret/config build-time → cargo:rustc-env (server chạy KHÔNG cần env runtime).
// CHỈ set khi env tồn tại lúc build → máy dev thiếu secret vẫn `cargo check` được (đọc qua
// option_env! + fallback ở config.rs). Secret NGOÀI (Mongo URI) nhúng lúc build production.
fn main() {
    for key in ["MONGO_URI", "PORT", "ALLOWED_DOMAINS", "ALLOWED_EMAILS", "TRANSCRIBE_EMAILS", "SESSION_KEY_PATH",
        "DATA_DIR", "DB_NAME", "PUBLIC_URL", "OIDC_ISSUER", "OIDC_WEB_CLIENT_ID",
        "OIDC_WEB_CLIENT_SECRET_PATH", "OIDC_MOBILE_CLIENT_ID", "OIDC_MOBILE_REDIRECT_URI", "OPENAI_KEY_PATH", "TRANSCRIBE_FREE_MINUTES",
        "TRANSCRIBE_PLUS_MINUTES", "TRANSCRIBE_FREE_POOL_USD", "TRANSCRIBE_MONTHLY_CAP_USD", "TRANSCRIBE_OPEN",
        "TRANSCRIBE_FREE_DAILY_MINUTES", "TRANSCRIBE_FREE_DELAY_HOURS",
        "TELEGRAM_TOKEN_PATH", "TELEGRAM_CHAT_ID"] {
        if let Ok(v) = std::env::var(key) {
            println!("cargo:rustc-env={key}={v}");
        }
        println!("cargo:rerun-if-env-changed={key}");
    }
}
