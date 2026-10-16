// Bake secret/config build-time → cargo:rustc-env (server chạy KHÔNG cần env runtime).
// CHỈ set khi env tồn tại lúc build → máy dev thiếu secret vẫn `cargo check` được (đọc qua
// option_env! + fallback ở config.rs). Secret NGOÀI (Mongo URI) nhúng lúc build production.
fn main() {
    for key in ["MONGO_URI", "PORT", "ALLOWED_DOMAINS", "ALLOWED_EMAILS", "TRANSCRIBE_EMAILS", "SESSION_KEY_PATH",
        "DATA_DIR", "DB_NAME", "FIREBASE_PROJECT_ID"] {
        if let Ok(v) = std::env::var(key) {
            println!("cargo:rustc-env={key}={v}");
        }
        println!("cargo:rerun-if-env-changed={key}");
    }
}
