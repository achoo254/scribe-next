//! Lắp router: public (health/auth) + protected (guard middleware).

use crate::auth;
use crate::state::AppState;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

pub fn build_router(state: AppState) -> Router {
    // Route cần đăng nhập → route_layer guard verify session cookie.
    let protected = Router::new()
        .route("/api/me", get(auth::me))
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

/// Health check qua nginx — KHÔNG cần DB (dùng cho deploy gate + systemd liveness).
async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}
