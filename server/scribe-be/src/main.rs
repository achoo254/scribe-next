//! Scribe Web backend — axum, bind 127.0.0.1 (nginx `/api` proxy), graceful shutdown SIGTERM.

mod access_log;
mod audit;
mod auth;
mod config;
mod db;
mod error;
mod files;
mod jobs;
mod jwks;
mod models;
mod rate_limit;
mod recordings;
mod retention;
mod routes;
mod session;
mod state;
mod transcribe;
mod upload;
mod worker;

use config::Config;
use jwks::JwksCache;
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
    let session_key = session::load_or_create_key(&config.session_key_path)
        .unwrap_or_else(|e| panic!("không tạo được session key ({}): {e}", config.session_key_path));
    let db = db::connect(&config.mongo_uri, &config.db_name)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let http = reqwest::Client::builder()
        .build()
        .expect("dựng reqwest client");

    // Tạo unique index users (non-blocking) — chặn user trùng firebaseUid khi 2 upsert đua nhau.
    {
        let db = db.clone();
        tokio::spawn(async move {
            if let Err(e) = models::user::ensure_indexes(&db).await {
                tracing::warn!("tạo index users thất bại (chống-trùng yếu đi nếu Mongo lỗi): {e}");
            }
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
        });
    }

    let port = config.port;
    let jwks = Arc::new(JwksCache::new(config.firebase_project_id.clone()));
    let state = AppState {
        config: Arc::new(config),
        db,
        http,
        jwks,
        session_key: Arc::new(session_key),
        rate: Arc::new(RateLimiter::new()),
        transcribe_keys: Arc::default(),
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
