//! Lắp router: public (health/auth) + protected (guard middleware + upload/jobs).

use crate::state::AppState;
use crate::{auth, files, jobs, rate_limit, recordings, upload};
use axum::extract::DefaultBodyLimit;
use axum::middleware::from_fn_with_state;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

pub fn build_router(state: AppState) -> Router {
    // Route cần đăng nhập → guard verify session; rate-limit chung (ngưỡng cao, không cản recording).
    let protected = Router::new()
        .route("/api/me", get(auth::me))
        .route("/api/me/usage", get(files::usage))
        .route("/api/upload/init", post(upload::init))
        .route("/api/upload/chunk", post(upload::chunk))
        .route("/api/upload/status", get(upload::status))
        .route("/api/upload/finalize", post(upload::finalize))
        // Mode A — ghi (streaming append open-ended) + phát lại (stream + Range).
        .route("/api/recordings/init", post(recordings::init))
        .route("/api/recordings/chunk", post(recordings::chunk))
        .route("/api/recordings/finalize", post(recordings::finalize))
        .route("/api/files", get(files::list))
        .route("/api/files/{id}", get(files::stream).delete(files::delete))
        .route("/api/jobs", post(jobs::create))
        .route("/api/jobs/{id}", get(jobs::get))
        .route("/api/jobs/{id}/download", get(jobs::download))
        // Chunk upload 5MB/lần → nâng body limit (mặc định axum 2MB). Không buffer cả file (append disk).
        .layer(DefaultBodyLimit::max(8 * 1024 * 1024))
        .route_layer(from_fn_with_state(state.clone(), auth::require_auth))
        // api_limit thêm SAU → chạy TRƯỚC require_auth (reject rẻ trước khi verify session).
        .route_layer(from_fn_with_state(state.clone(), rate_limit::api_limit));

    // Auth: rate-limit chặt (chống brute/amplification). Health KHÔNG giới hạn (systemd/nginx liveness).
    let auth_routes = Router::new()
        .route("/api/auth/session", post(auth::create_session))
        .route("/api/auth/logout", post(auth::logout))
        .route_layer(from_fn_with_state(state.clone(), rate_limit::auth_limit));

    let public = Router::new()
        .route("/api/health", get(health))
        .merge(auth_routes);

    Router::new()
        .merge(public)
        .merge(protected)
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .with_state(state)
}

/// Health check qua nginx — KHÔNG cần DB (deploy gate + systemd liveness).
async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}
