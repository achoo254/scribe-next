//! Lỗi app → HTTP response. Log chi tiết server-side, trả client message gọn.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    Unauthorized(String),
    Forbidden(String),
    BadRequest(String),
    NotFound(String),
    /// 413 — vượt size cap / quota (red-team #3/#14).
    TooLarge(String),
    Internal(String),
}

impl AppError {
    fn parts(&self) -> (StatusCode, &str) {
        match self {
            AppError::Unauthorized(m) => (StatusCode::UNAUTHORIZED, m),
            AppError::Forbidden(m) => (StatusCode::FORBIDDEN, m),
            AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            AppError::NotFound(m) => (StatusCode::NOT_FOUND, m),
            AppError::TooLarge(m) => (StatusCode::PAYLOAD_TOO_LARGE, m),
            AppError::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, msg) = self.parts();
        if status == StatusCode::INTERNAL_SERVER_ERROR {
            tracing::error!("internal error: {msg}");
            // Không rò chi tiết nội bộ ra client.
            return (status, Json(json!({ "error": "internal server error" }))).into_response();
        }
        (status, Json(json!({ "error": msg }))).into_response()
    }
}
