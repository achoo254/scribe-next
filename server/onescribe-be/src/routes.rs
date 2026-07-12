//! Lắp router: public (health/auth) + protected (guard middleware + upload/jobs).

use crate::state::AppState;
use crate::{auth, files, jobs, recordings, upload};
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

pub fn build_router(state: AppState) -> Router {
    // Route cần đăng nhập → route_layer guard verify session cookie.
    let protected = Router::new()
        .route("/api/me", get(auth::me))
        .route("/api/upload/init", post(upload::init))
        .route("/api/upload/chunk", post(upload::chunk))
        .route("/api/upload/status", get(upload::status))
        .route("/api/upload/finalize", post(upload::finalize))
        // Mode A — ghi (streaming append open-ended) + phát lại (stream + Range).
        .route("/api/recordings/init", post(recordings::init))
        .route("/api/recordings/chunk", post(recordings::chunk))
        .route("/api/recordings/finalize", post(recordings::finalize))
        .route("/api/files", get(files::list))
        .route("/api/files/{id}", get(files::stream))
        .route("/api/jobs", post(jobs::create))
        .route("/api/jobs/{id}", get(jobs::get))
        .route("/api/jobs/{id}/download", get(jobs::download))
        // Chunk upload 5MB/lần → nâng body limit (mặc định axum 2MB). Không buffer cả file (append disk).
        .layer(DefaultBodyLimit::max(8 * 1024 * 1024))
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), auth::require_auth));

    let public = Router::new()
        .route("/api/health", get(health))
        .route("/api/auth/session", post(auth::create_session))
        .route("/api/auth/logout", post(auth::logout));

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
