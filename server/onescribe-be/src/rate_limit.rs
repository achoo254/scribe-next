//! Rate-limit theo IP (fixed window in-memory). Red-team #2: đọc `X-Real-IP` do nginx set từ
//! `$remote_addr` (sau `set_real_ip_from` CF) — nginx LUÔN ghi đè header này, client KHÔNG giả được
//! kể cả khi bypass Cloudflare (lúc đó $remote_addr = IP thật của kẻ tấn công).
//!
//! Lưu ý: .65 shared → KHÔNG firewall :80 chỉ-cho-CF được → origin vẫn reachable trực tiếp, nhưng
//! rate-limit theo IP thật vẫn hiệu lực. Đây là tuyến phòng thủ BỔ SUNG; phòng thủ chính vẫn là
//! quota + size cap + audit-log.

use crate::error::AppError;
use crate::state::AppState;
use axum::extract::{Request, State};
use axum::http::HeaderMap;
use axum::middleware::Next;
use axum::response::Response;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const WINDOW: Duration = Duration::from_secs(60);
const AUTH_LIMIT: u32 = 20; // /api/auth/* — login/logout
// protected NGOẠI TRỪ /chunk (xem api_limit). Đủ cho poll (~40/phút) + thao tác người dùng; chunk
// streaming (upload lớn hàng trăm req / recording 60 req/phút) được MIỄN vì đã chặn bởi quota+size.
const API_LIMIT: u32 = 300;

struct Bucket {
    count: u32,
    start: Instant,
}

#[derive(Default)]
pub struct RateLimiter {
    inner: Mutex<HashMap<String, Bucket>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self::default()
    }

    /// true = cho phép; false = vượt ngưỡng trong cửa sổ. Reset đếm khi hết cửa sổ.
    fn allow(&self, key: &str, limit: u32) -> bool {
        let now = Instant::now();
        let mut map = match self.inner.lock() {
            Ok(m) => m,
            Err(p) => p.into_inner(), // poison → vẫn dùng (rate-limit không cần chính xác tuyệt đối)
        };
        // Dọn bucket đã hết cửa sổ khi map lớn (không cần task nền). Lưu ý: dưới tải liên tục từ
        // nhiều IP thật, bucket "fresh" chưa bị dọn → map có thể tạm lớn trong lúc tải; mỗi entry
        // vài chục byte nên vẫn an toàn ở quy mô công cụ nội bộ.
        if map.len() > 10_000 {
            map.retain(|_, b| now.duration_since(b.start) <= WINDOW);
        }
        let b = map.entry(key.to_string()).or_insert(Bucket { count: 0, start: now });
        if now.duration_since(b.start) > WINDOW {
            b.count = 0;
            b.start = now;
        }
        b.count += 1;
        b.count <= limit
    }
}

/// Real client IP: X-Real-IP (nginx $remote_addr sau set_real_ip_from) → XFF hop đầu → "unknown".
fn client_ip(headers: &HeaderMap) -> String {
    if let Some(ip) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        return ip.trim().to_string();
    }
    if let Some(xff) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = xff.split(',').next() {
            return first.trim().to_string();
        }
    }
    "unknown".to_string()
}

/// Middleware chặt cho `/api/auth/*` (chống brute/amplification login).
pub async fn auth_limit(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let ip = client_ip(req.headers());
    if !state.rate.allow(&format!("auth:{ip}"), AUTH_LIMIT) {
        return Err(AppError::TooManyRequests("quá nhiều yêu cầu đăng nhập, thử lại sau".into()));
    }
    Ok(next.run(req).await)
}

/// Middleware chung cho protected API. MIỄN `/chunk` (upload/recording streaming) — 1 upload 2GB =
/// hàng trăm chunk hợp lệ, recording = 60 chunk/phút; đã chặn bởi quota + size cap nên rate-limit
/// theo-request sẽ chặn nhầm. Các route còn lại (init/finalize/jobs/files/me) ngưỡng API_LIMIT.
pub async fn api_limit(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    if req.uri().path().ends_with("/chunk") {
        return Ok(next.run(req).await);
    }
    let ip = client_ip(req.headers());
    if !state.rate.allow(&format!("api:{ip}"), API_LIMIT) {
        return Err(AppError::TooManyRequests("quá nhiều yêu cầu, thử lại sau".into()));
    }
    Ok(next.run(req).await)
}
