//! Access log: một dòng có cấu trúc cho mỗi request `/api` (method, path, status, thời gian, IP,
//! user) ra journal. Chunk upload/ghi thành công (≈1 request/giây khi ghi) hạ xuống debug để log
//! không bị ngập; chunk lỗi vẫn ở info/warn. `/api/health` (liveness) bỏ qua.

use crate::audit::CLIENT_IP;
use crate::auth::UserId;
use crate::rate_limit::client_ip;
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use std::time::Instant;

const QUIET_PATHS: [&str; 2] = ["/api/upload/chunk", "/api/recordings/chunk"];

pub async fn layer(req: Request, next: Next) -> Response {
    let path = req.uri().path().to_string();
    if path == "/api/health" {
        return next.run(req).await;
    }
    let method = req.method().clone();
    let ip = client_ip(req.headers());
    let started = Instant::now();

    let res = CLIENT_IP.scope(ip.clone(), next.run(req)).await;

    let status = res.status().as_u16();
    let ms = started.elapsed().as_millis() as u64;
    // `require_auth` gắn UserId vào response để log ở đây thấy được (request đã bị tiêu thụ).
    let user = res
        .extensions()
        .get::<UserId>()
        .map(|u| u.0.as_str())
        .unwrap_or("-");
    if status >= 500 {
        tracing::warn!(target: "access", %method, %path, status, ms, %ip, user, "request");
    } else if status < 400 && QUIET_PATHS.contains(&path.as_str()) {
        tracing::debug!(target: "access", %method, %path, status, ms, %ip, user, "request");
    } else {
        tracing::info!(target: "access", %method, %path, status, ms, %ip, user, "request");
    }
    res
}
