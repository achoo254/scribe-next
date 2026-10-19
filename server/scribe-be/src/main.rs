//! Scribe Web backend — axum, bind 127.0.0.1 (nginx `/api` proxy), graceful shutdown SIGTERM.

mod access_log;
mod admin;
mod audit;
mod auth;
mod chat_link;
mod config;
mod db;
mod error;
mod files;
mod jobs;
mod models;
mod notify;
mod oidc;
mod rate_limit;
mod realtime;
mod recordings;
mod retention;
mod routes;
mod search;
mod session;
mod state;
mod summary;
mod transcribe;
mod translate;
mod upload;
mod user_keys;
mod worker;

use config::Config;
use rate_limit::RateLimiter;
use state::AppState;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let config = Config::from_env().unwrap_or_else(|e| {
        tracing::error!("{e}");
        std::process::exit(1);
    });
    let db = db::connect(&config.mongo_uri, &config.db_name)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    // `scribe-be admin …`: chỉ nối Mongo rồi thoát — không session key, index, worker hay sweeper,
    // để chạy cạnh service đang phục vụ mà không nhận lượt chép hay dọn file.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("admin") {
        match admin::run(&config, &db, &args[1..]).await {
            Ok(out) => println!("{out:#}"),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(2);
            }
        }
        return;
    }
    let session_key = session::load_or_create_key(&config.session_key_path)
        .unwrap_or_else(|e| panic!("không tạo được session key ({}): {e}", config.session_key_path));
    // Index unique `transcript_docs` phải có trước request đầu tiên: upsert đua nhau dựa vào nó để
    // không tạo hai doc cho một phiên. Không tạo được thì dừng thay vì chạy thiếu bảo đảm.
    if let Err(e) = models::transcript_doc::ensure_indexes(&db).await {
        tracing::error!("{e}");
        std::process::exit(1);
    }
    // Index unique bản dịch: hai lượt dịch bắt đầu cùng lúc chỉ tạo một doc mỗi ngôn ngữ.
    if let Err(e) = models::translation::ensure_indexes(&db).await {
        tracing::error!("{e}");
        std::process::exit(1);
    }
    // Text index của index tìm kiếm phải có trước request đầu tiên: `$text` lỗi khi chưa có text index.
    if let Err(e) = models::sentence::ensure_indexes(&db).await {
        tracing::error!("{e}");
        std::process::exit(1);
    }
    let http = reqwest::Client::builder()
        .build()
        .expect("dựng reqwest client");

    // Index của files/jobs/transcripts/audit (nền, lỗi chỉ log). Index transcript_docs, transcript_sentences,
    // users và phiên đăng nhập tạo riêng, chờ xong và chặn khởi động nếu lỗi.
    {
        let db = db.clone();
        tokio::spawn(async move {
            if let Err(e) = models::file::ensure_indexes(&db).await {
                tracing::warn!("tạo index files thất bại: {e}");
            }
            if let Err(e) = models::job::ensure_indexes(&db).await {
                tracing::warn!("tạo index jobs thất bại: {e}");
            }
            if let Err(e) = models::transcript::ensure_indexes(&db).await {
                tracing::warn!("tạo index transcripts thất bại: {e}");
            }
            if let Err(e) = audit::ensure_indexes(&db).await {
                tracing::warn!("tạo index audit_events thất bại: {e}");
            }
            if let Err(e) = models::chat_link::ensure_indexes(&db).await {
                tracing::warn!("tạo index chat_link_tokens thất bại: {e}");
            }
            if let Err(e) = models::realtime_token::ensure_indexes(&db).await {
                tracing::warn!("{e}");
            }
        });
    }

    let port = config.port;
    if let Some(model) = config::unlisted_llm_model() {
        tracing::error!(model = %model, fallback = %config.summary_model, "LLM_MODEL không có trong bảng giá — dùng model mặc định");
    }
    let transcriber = transcribe::Transcriber::load(&config);
    // Key riêng: thiếu hoặc sai tệp bí mật thì tính năng tắt (giao diện báo), server vẫn chạy.
    let user_keys_secret = match config.user_keys_secret_path.as_deref() {
        Some(path) => match user_keys::crypto::Secret::load(path) {
            Ok(secret) => {
                tracing::info!("Key riêng bật");
                Some(Arc::new(secret))
            }
            Err(e) => {
                tracing::error!(path = %path, "Key riêng tắt: {e}");
                None
            }
        },
        None => {
            tracing::info!("Key riêng tắt (chưa đặt USER_KEYS_SECRET_PATH)");
            None
        }
    };
    // Index unique `{userId, provider}`: hai lần lưu cùng lúc không tạo hai doc cho một key.
    if let Err(e) = user_keys::store::ensure_indexes(&db).await {
        tracing::error!("{e}");
        std::process::exit(1);
    }
    let oidc_issuer = config.oidc_issuer.clone();
    let oidc_web_secret = std::fs::read_to_string(&config.oidc_web_client_secret_path)
        .map(|s| s.trim().to_string())
        .ok()
        .filter(|s| s.len() >= 32)
        .unwrap_or_else(|| {
            tracing::error!(path = %config.oidc_web_client_secret_path, "không đọc được secret client OIDC (≥ 32 ký tự)");
            std::process::exit(1);
        });
    // Đăng nhập dựa vào index unique của users (không tạo user trùng) và TTL của phiên.
    if let Err(e) = models::user::ensure_indexes(&db).await {
        tracing::error!("{e}");
        std::process::exit(1);
    }
    if let Err(e) = models::session::ensure_indexes(&db).await {
        tracing::error!("{e}");
        std::process::exit(1);
    }
    let state = AppState {
        config: Arc::new(config),
        db,
        http,
        oidc: Arc::new(oidc::Oidc::new(&oidc_issuer)),
        oidc_web_secret: Arc::new(oidc_web_secret),
        session_cache: Arc::default(),
        session_key: Arc::new(session_key),
        rate: Arc::new(RateLimiter::new()),
        transcriber: Arc::new(transcriber),
        summaries: Arc::default(),
        chat_bot: Arc::default(),
        user_keys: user_keys_secret,
    };

    // Hồi phục job kẹt "running" sau crash/restart (red-team) + khởi worker queue nền.
    match models::job::reset_running_to_queued(&state.db).await {
        Ok(n) if n > 0 => tracing::info!("reset {n} job running→queued sau restart"),
        Ok(_) => {}
        Err(e) => tracing::warn!("reset running jobs lỗi: {e:?}"),
    }
    jobs::start_worker(state.clone());
    transcribe::start_worker(state.clone()).await;
    retention::start_sweeper(state.clone()); // sweep purge quá hạn + orphan (red-team #1)
    search::index::start_backfill(state.db.clone()); // đưa dữ liệu cũ vào index tìm kiếm một lần
    chat_link::warm_up(state.clone());
    chat_link::poll::start(state.clone()); // chỉ khi CHAT_LINK_POLL bật và có token bot

    let app = routes::build_router(state);
    let addr = SocketAddr::from(([127, 0, 0, 1], port)); // localhost-only; nginx proxy /api.
    let listener = TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| panic!("bind {addr}: {e}"));
    tracing::info!("scribe-be listening on {addr}");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");
}

/// Chờ Ctrl-C hoặc SIGTERM (systemd gửi khi stop/restart).
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sig) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            sig.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal — dừng nhận request mới");
}
