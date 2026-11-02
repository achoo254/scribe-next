// Bake secret/config build-time → cargo:rustc-env (server chạy KHÔNG cần env runtime).
// CHỈ set khi env tồn tại lúc build → máy dev thiếu secret vẫn `cargo check` được (đọc qua
// option_env! + fallback ở config.rs). Secret NGOÀI (Mongo URI) nhúng lúc build production.
fn main() {
    // Biến cũ của key AI server và hạn mức AI (OPENAI_KEY_PATH, TRANSCRIBE_PROVIDER, SUMMARY_*, …) không còn trong danh
    // sách: còn đặt lúc build thì bị bỏ qua (chủ dự án chốt 02/11/2026 mọi việc AI chạy bằng key riêng của người dùng).
    for key in ["MONGO_URI", "PORT", "ALLOWED_DOMAINS", "ALLOWED_EMAILS", "UNVERIFIED_EMAIL_SUBS", "SESSION_KEY_PATH",
        "DATA_DIR", "DB_NAME", "PUBLIC_URL", "OIDC_ISSUER", "OIDC_WEB_CLIENT_ID",
        "OIDC_WEB_CLIENT_SECRET_PATH", "OIDC_MOBILE_CLIENT_ID", "OIDC_MOBILE_REDIRECT_URI", "TRANSCRIBE_CONCURRENCY",
        "NOTIFY_BOT_API", "NOTIFY_BOT_TOKEN_PATH", "NOTIFY_CHAT_ID", "CHAT_LINK_POLL",
        "CHAT_BOT_USERNAME", "USER_KEYS_SECRET_PATH", "IDP_DISPLAY_NAME"] {
        if let Ok(v) = std::env::var(key) {
            println!("cargo:rustc-env={key}={v}");
        }
        println!("cargo:rerun-if-env-changed={key}");
    }
}
