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
    /// 409 — dữ liệu đã đổi so với lúc client đọc (vd đoạn vừa được chép lại).
    Conflict(String),
    /// 413 — vượt size cap / quota (red-team #3/#14).
    TooLarge(String),
    /// 429 — rate-limit theo IP (red-team #2).
    TooManyRequests(String),
    /// Lỗi kèm mã máy đọc được (`{error, code}`), cho client rẽ nhánh mà không dò câu chữ
    /// (vd 409 `CHAT_NOT_LINKED`, `NO_SUMMARY`, 403 `NO_USER_KEY`).
    Coded { status: StatusCode, code: &'static str, message: String },
    /// 503 — tạm thời không làm được (vd nhà cung cấp không cấp được token).
    Unavailable(String),
    Internal(String),
}

impl AppError {
    fn parts(&self) -> (StatusCode, &str) {
        match self {
            AppError::Unauthorized(m) => (StatusCode::UNAUTHORIZED, m),
            AppError::Forbidden(m) => (StatusCode::FORBIDDEN, m),
            AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            AppError::NotFound(m) => (StatusCode::NOT_FOUND, m),
            AppError::Conflict(m) => (StatusCode::CONFLICT, m),
            AppError::TooLarge(m) => (StatusCode::PAYLOAD_TOO_LARGE, m),
            AppError::TooManyRequests(m) => (StatusCode::TOO_MANY_REQUESTS, m),
            AppError::Coded { status, message, .. } => (*status, message),
            AppError::Unavailable(m) => (StatusCode::SERVICE_UNAVAILABLE, m),
            AppError::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        if let AppError::Coded { status, code, message } = &self {
            return (*status, Json(json!({ "error": message, "code": code }))).into_response();
        }
        let (status, msg) = self.parts();
        if status == StatusCode::INTERNAL_SERVER_ERROR {
            tracing::error!("internal error: {msg}");
            // Không rò chi tiết nội bộ ra client.
            return (status, Json(json!({ "error": "internal server error" }))).into_response();
        }
        (status, Json(json!({ "error": msg }))).into_response()
    }
}
