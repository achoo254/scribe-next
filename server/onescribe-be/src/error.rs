//! Lỗi app → HTTP response. Log chi tiết server-side, trả client message gọn.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    Unauthorized(String),
    Forbidden(String),
    /// Dùng ở Phase 3 (validate upload) — giữ sẵn cho contract lỗi 4xx.
    #[allow(dead_code)]
    BadRequest(String),
    Internal(String),
}

impl AppError {
    fn parts(&self) -> (StatusCode, &str) {
        match self {
            AppError::Unauthorized(m) => (StatusCode::UNAUTHORIZED, m),
            AppError::Forbidden(m) => (StatusCode::FORBIDDEN, m),
            AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
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
