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
/// Route đăng nhập thường (login, callback, cấu hình, làm mới, đăng xuất): đủ rộng cho văn phòng chung NAT.
const AUTH_LIMIT: u32 = 120;
/// `/api/auth/native/exchange` không cần đăng nhập mà mỗi lần gọi IdP: chặt theo IP.
const EXCHANGE_LIMIT: u32 = 10;
/// Mọi lần gọi token endpoint của IdP đi ra từ một IP server, IdP cho 60/phút/IP: giữ dưới mức đó để
/// một người gửi code rác không làm hỏng đăng nhập và làm mới của mọi người.
pub const IDP_TOKEN_BUDGET: u32 = 45;
// protected NGOẠI TRỪ /chunk (xem api_limit). Đủ cho poll (~40/phút) + thao tác người dùng; chunk
// streaming (upload lớn hàng trăm req / recording 60 req/phút) được MIỄN vì đã chặn bởi quota+size.
const API_LIMIT: u32 = 300;

struct Bucket {
    count: u32,
    start: Instant,
    window: Duration,
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
    pub fn allow(&self, key: &str, limit: u32) -> bool {
        self.allow_in(key, limit, WINDOW)
    }

    /// Như [`Self::allow`] với cửa sổ riêng (vd 10 lượt gửi chat mỗi giờ của một người).
    pub fn allow_in(&self, key: &str, limit: u32, window: Duration) -> bool {
        let now = Instant::now();
        let mut map = match self.inner.lock() {
            Ok(m) => m,
            Err(p) => p.into_inner(), // poison → vẫn dùng (rate-limit không cần chính xác tuyệt đối)
        };
        // Dọn bucket đã hết cửa sổ khi map lớn (không cần task nền). Lưu ý: dưới tải liên tục từ
        // nhiều IP thật, bucket "fresh" chưa bị dọn → map có thể tạm lớn trong lúc tải; mỗi entry
        // vài chục byte nên vẫn an toàn ở quy mô công cụ nội bộ.
        if map.len() > 10_000 {
            map.retain(|_, b| now.duration_since(b.start) <= b.window);
        }
        let b = map.entry(key.to_string()).or_insert(Bucket { count: 0, start: now, window });
        if now.duration_since(b.start) > b.window {
            b.count = 0;
            b.start = now;
            b.window = window;
        }
        b.count += 1;
        b.count <= limit
    }
}

/// Real client IP: X-Real-IP (nginx $remote_addr sau set_real_ip_from) → XFF hop đầu → "unknown".
pub(crate) fn client_ip(headers: &HeaderMap) -> String {
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
        // login/callback là điều hướng của trình duyệt: trả trang đăng nhập kèm mã thay vì JSON trần.
        let path = req.uri().path();
        if path == "/api/auth/login" || path == "/api/auth/callback" {
            return Ok(crate::auth::login_failed(&state, "busy"));
        }
        return Err(AppError::TooManyRequests("quá nhiều yêu cầu đăng nhập, thử lại sau".into()));
    }
    Ok(next.run(req).await)
}

/// Middleware cho `/api/auth/native/exchange` (route public, mỗi lần gọi tới IdP).
pub async fn exchange_limit(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let ip = client_ip(req.headers());
    if !state.rate.allow(&format!("exchange:{ip}"), EXCHANGE_LIMIT) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limiter_counts_per_key_within_the_window() {
        let rate = RateLimiter::new();
        assert!((0..3).all(|_| rate.allow_in("share:a", 3, Duration::from_secs(3600))));
        assert!(!rate.allow_in("share:a", 3, Duration::from_secs(3600)));
        assert!(rate.allow_in("share:b", 3, Duration::from_secs(3600)));
    }

    #[test]
    fn limiter_resets_the_count_when_the_window_ends() {
        let rate = RateLimiter::new();
        let window = Duration::from_millis(20);
        assert!(rate.allow_in("api:a", 1, window));
        assert!(!rate.allow_in("api:a", 1, window));
        std::thread::sleep(Duration::from_millis(30));
        assert!(rate.allow_in("api:a", 1, window));
        assert!(!rate.allow_in("api:a", 1, window));
    }

    #[test]
    fn client_ip_prefers_real_ip_then_first_forwarded_hop() {
        let mut headers = HeaderMap::new();
        assert_eq!(client_ip(&headers), "unknown");
        headers.insert("x-forwarded-for", " 203.0.113.7 , 10.0.0.1".parse().unwrap());
        assert_eq!(client_ip(&headers), "203.0.113.7");
        headers.insert("x-real-ip", " 198.51.100.2 ".parse().unwrap());
        assert_eq!(client_ip(&headers), "198.51.100.2");
    }
}
