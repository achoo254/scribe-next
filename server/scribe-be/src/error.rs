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
    /// 403 kèm số — hết phút chép lời (`QUOTA_EXCEEDED`, `DAILY_LIMIT`, `IP_LIMIT`), hạn mức chung
    /// (`CAP_REACHED`) hoặc tài khoản còn mới (`ACCOUNT_TOO_NEW`); `retry_at` = lúc mở lại (RFC 3339).
    Quota { code: &'static str, message: String, used_minutes: i64, limit_minutes: i64, retry_at: Option<String> },
    /// 503 — tính năng tạm ngưng (vd key OpenAI của server hỏng).
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
            AppError::Quota { message, .. } => (StatusCode::FORBIDDEN, message),
            AppError::Unavailable(m) => (StatusCode::SERVICE_UNAVAILABLE, m),
            AppError::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        if let AppError::Quota { code, message, used_minutes, limit_minutes, retry_at } = &self {
            let body = json!({
                "error": message,
                "code": code,
                "usedMinutes": used_minutes,
                "limitMinutes": limit_minutes,
                "retryAt": retry_at,
            });
            return (StatusCode::FORBIDDEN, Json(body)).into_response();
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
