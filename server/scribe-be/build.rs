// Bake config build-time → cargo:rustc-env (server chạy KHÔNG cần env runtime).
// CHỈ set khi env tồn tại lúc build → máy dev thiếu biến vẫn `cargo check` được (đọc qua
// option_env! + fallback ở config.rs). Không biến nào ở đây được chứa bí mật trong bản build prod: artifact của repo
// public tải về được. Bí mật nằm trong tệp 600 trên server, biến build chỉ mang đường dẫn (`*_PATH`).
// `MONGO_URI` (nhúng cả URI có mật khẩu) chỉ còn cho dev; prod dùng `MONGO_URI_PATH`.
fn main() {
    let set = |key: &str| std::env::var(key).is_ok_and(|v| !v.is_empty());
    if set("MONGO_URI") && set("MONGO_URI_PATH") {
        panic!("đặt MONGO_URI_PATH (tệp đọc lúc khởi động) hoặc MONGO_URI (nhúng vào binary, chỉ dev), không cả hai");
    }
    // Biến cũ của key AI server và hạn mức AI (OPENAI_KEY_PATH, TRANSCRIBE_PROVIDER, SUMMARY_*, …) không còn trong danh
    // sách: còn đặt lúc build thì bị bỏ qua (chủ dự án chốt 02/11/2026 mọi việc AI chạy bằng key riêng của người dùng).
    for key in ["MONGO_URI", "MONGO_URI_PATH", "PORT", "ALLOWED_DOMAINS", "ALLOWED_EMAILS", "UNVERIFIED_EMAIL_SUBS", "SESSION_KEY_PATH",
        "DATA_DIR", "DB_NAME", "PUBLIC_URL", "OIDC_ISSUER", "OIDC_WEB_CLIENT_ID",
        "OIDC_WEB_CLIENT_SECRET_PATH", "OIDC_MOBILE_CLIENT_ID", "OIDC_MOBILE_REDIRECT_URI", "TRANSCRIBE_CONCURRENCY",
        "NOTIFY_BOT_API", "NOTIFY_BOT_TOKEN_PATH", "NOTIFY_CHAT_ID", "CHAT_LINK_POLL",
        "CHAT_BOT_USERNAME", "USER_KEYS_SECRET_PATH", "IDP_DISPLAY_NAME", "NOTIFY_BOT_RESOLVE"] {
        if let Ok(v) = std::env::var(key) {
            println!("cargo:rustc-env={key}={v}");
        }
        println!("cargo:rerun-if-env-changed={key}");
    }
}
