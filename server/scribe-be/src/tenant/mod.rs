//! Tenant theo Host: một process `scribe-be` phục vụ `scribe.<domain>` của mọi tenant của hoandat-auth (quyết định ở
//! `docs/decisions/0001-tenants-by-domain.md`).
//!
//! - Host bằng host của `PUBLIC_URL` (hay loopback, cho dev và lệnh trên máy chủ) ⟹ tenant chính, id [`PRIMARY_ID`],
//!   không hỏi IdP: hành vi như trước khi có tenant (key server, gói, app di động).
//! - Host `scribe.<domain>` khác ⟹ tra [`directory::TenantDirectory`] (API nội bộ của IdP, có cache); không có ⟹ 404
//!   `UNKNOWN_TENANT`. Host khác nữa ⟹ 404.
//! - Tenant sổ chỉ chạy khi [`TenantCtx::enabled`]: IdP liệt kê `webApps.scribe` (chủ domain đã nhập key chép lời),
//!   phía Scribe sẵn sàng ([`Readiness`]) và người vận hành không tắt (`admin tenant-disable`). Chưa bật thì route
//!   cần đăng nhập và route đăng nhập trả 403 `TENANT_DISABLED`; `/api/idp` luôn trả để web hiện trang "chưa bật".
//!
//! Cô lập theo user: mọi dữ liệu Scribe khoá theo `userId`, còn `tenantId` nằm ở danh tính (`users`, phiên, thiết
//! bị, lượt đăng nhập, jti) và hàng đợi (`transcripts`, `jobs`).

pub mod admin;
pub mod answer_cache;
pub mod directory;
pub mod internal_api;
pub mod migrate;
#[cfg(test)]
mod routing_tests;

use crate::error::AppError;
use crate::state::AppState;
use answer_cache::LookupError;
use axum::extract::{Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Extension;
use mongodb::bson::{doc, DateTime, Document};
use mongodb::Database;
use std::sync::Arc;

/// Id của tenant chính (cùng id với `PRIMARY_TENANT_ID` của hoandat-auth). Dữ liệu có trước khi có tenant được gắn id
/// này lúc khởi động ([`migrate`]).
pub const PRIMARY_ID: &str = "primary";
/// Nhãn đầu của host Scribe ở mọi tenant sổ.
pub const HOST_PREFIX: &str = "scribe.";
const KV: &str = "kv";
/// Lượt hỏi IdP về host chưa có trong cache mỗi phút: theo IP người gọi và cho cả server. Host bịa ra (nginx nhận mọi
/// `scribe.*`) không làm IdP làm việc không giới hạn; host đã biết không tốn lượt nào.
const LOOKUPS_PER_IP: u32 = 30;
const LOOKUPS_TOTAL: u32 = 300;

/// Tenant của request hiện tại (gắn vào request bởi [`resolve`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantCtx {
    pub id: String,
    /// Domain của tenant (`hoandat.com`, `inetdev.io.vn`).
    pub domain: String,
    /// Issuer OIDC của tenant (`https://auth.<domain>`).
    pub issuer: String,
    /// Origin của Scribe trên tenant như trình duyệt thấy (`https://scribe.<domain>`).
    pub public_url: String,
    /// Tên tài khoản đăng nhập của tenant (`TENANT_DISPLAY_NAME` của IdP).
    pub display_name: String,
    pub primary: bool,
    /// Tenant đang phục vụ Scribe (tenant chính luôn bật).
    pub enabled: bool,
}

pub type Tenant = Arc<TenantCtx>;

/// [`PRIMARY_ID`] làm giá trị mặc định khi đọc doc (serde `default`): sau [`migrate`] mọi doc đều có `tenantId`.
pub fn primary_id() -> String {
    PRIMARY_ID.to_string()
}

/// Điều kiện phía Scribe để tenant sổ được bật, chung cho mọi tenant. Thiếu điều kiện nào thì mọi tenant sổ hiện trang
/// "chưa bật" (công tắc tắt mặc định); tenant chính không phụ thuộc.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Readiness {
    /// Đăng nhập qua IdP của tenant sổ dựng được: API nội bộ, listener loopback và khoá app `scribe-web`.
    pub oidc: bool,
    /// Mở được key AI mà chủ domain niêm phong cho Scribe (vòng khoá niêm phong). Chưa có thì không tenant sổ nào chạy:
    /// tenant sổ không bao giờ được dùng key của server.
    pub sealed_keys: bool,
}

impl Readiness {
    pub fn ready(self) -> bool {
        self.oidc && self.sealed_keys
    }
}

/// Host chuẩn hoá: chữ thường, bỏ cổng, bỏ dấu chấm cuối. Rỗng hay có ký tự lạ ⟹ `None`.
pub fn normalize_host(raw: &str) -> Option<String> {
    let raw = raw.trim().to_ascii_lowercase();
    let host = if let Some(rest) = raw.strip_prefix('[') {
        // IPv6 `[::1]:8787`.
        rest.split(']').next().unwrap_or_default().to_string()
    } else {
        raw.rsplit_once(':').filter(|(_, port)| port.chars().all(|c| c.is_ascii_digit())).map_or(raw.as_str(), |(h, _)| h).to_string()
    };
    let host = host.trim_end_matches('.').to_string();
    let ok = !host.is_empty() && host.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':'));
    ok.then_some(host)
}

/// Host loopback: request không qua nginx (dev qua proxy của Vite, lệnh `curl` trên máy chủ) là của tenant chính.
pub fn is_loopback(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

/// Host (đã chuẩn hoá) của một origin.
pub fn host_of_url(url: &str) -> Option<String> {
    reqwest::Url::parse(url).ok()?.host_str().and_then(normalize_host)
}

fn disabled_key(tenant_id: &str) -> String {
    format!("tenant.disabled:{tenant_id}")
}

/// Người vận hành đã tắt Scribe trên tenant này chưa (`admin tenant-disable`).
pub async fn is_disabled(db: &Database, tenant_id: &str) -> Result<bool, String> {
    db.collection::<Document>(KV)
        .count_documents(doc! { "_id": disabled_key(tenant_id) })
        .await
        .map(|n| n > 0)
        .map_err(|e| format!("đọc cờ tắt tenant: {e}"))
}

/// Bật (`false`) hay tắt (`true`) Scribe trên tenant. Trả `true` khi cờ đổi.
pub async fn set_disabled(db: &Database, tenant_id: &str, disabled: bool) -> Result<bool, String> {
    let coll = db.collection::<Document>(KV);
    let id = disabled_key(tenant_id);
    let changed = if disabled {
        let res = coll
            .update_one(doc! { "_id": &id }, doc! { "$setOnInsert": { "at": DateTime::now() } })
            .upsert(true)
            .await
            .map_err(|e| format!("ghi cờ tắt tenant: {e}"))?;
        res.upserted_id.is_some()
    } else {
        coll.delete_one(doc! { "_id": &id }).await.map_err(|e| format!("xoá cờ tắt tenant: {e}"))?.deleted_count > 0
    };
    Ok(changed)
}

fn unknown_tenant() -> AppError {
    AppError::Coded { status: StatusCode::NOT_FOUND, code: "UNKNOWN_TENANT", message: "Scribe không phục vụ domain này".into() }
}

fn tenant_disabled() -> AppError {
    AppError::Coded { status: StatusCode::FORBIDDEN, code: "TENANT_DISABLED", message: "Scribe chưa bật trên domain này".into() }
}

/// Middleware ngoài cùng của `/api` (trừ `/api/health`): Host → tenant, gắn [`Tenant`] vào request.
pub async fn resolve(State(state): State<AppState>, mut req: Request, next: Next) -> Response {
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .or_else(|| req.uri().authority().map(|a| a.to_string()));
    let ip = crate::rate_limit::client_ip(req.headers());
    let allow_lookup = || {
        state.rate.allow(&format!("tenant-lookup:{ip}"), LOOKUPS_PER_IP) && state.rate.allow("tenant-lookup", LOOKUPS_TOTAL)
    };
    match state.tenants.resolve(host.as_deref(), allow_lookup).await {
        Ok(Some(tenant)) => {
            req.extensions_mut().insert(tenant);
            next.run(req).await
        }
        Ok(None) => unknown_tenant().into_response(),
        Err(LookupError::Busy) => AppError::TooManyRequests("quá nhiều yêu cầu, thử lại sau ít phút".into()).into_response(),
        Err(LookupError::Failed(e)) => {
            tracing::warn!(host = host.as_deref().unwrap_or("-"), "tra tenant lỗi: {e}");
            AppError::Unavailable("chưa kiểm được domain này, thử lại sau ít phút".into()).into_response()
        }
    }
}

/// Route API cần tenant đang bật (route cần đăng nhập, route đăng nhập của app): 403 `TENANT_DISABLED`.
pub async fn require_enabled(Extension(tenant): Extension<Tenant>, req: Request, next: Next) -> Response {
    if tenant.enabled {
        next.run(req).await
    } else {
        tenant_disabled().into_response()
    }
}

/// Như [`require_enabled`] cho route trình duyệt mở thẳng (bắt đầu đăng nhập, callback): về trang đăng nhập của
/// tenant với `?error=not_enabled` thay vì một trang JSON.
pub async fn require_enabled_web(Extension(tenant): Extension<Tenant>, req: Request, next: Next) -> Response {
    if tenant.enabled {
        next.run(req).await
    } else {
        Redirect::to(&format!("{}/login?error=not_enabled", tenant.public_url)).into_response()
    }
}

/// Tenant sổ chưa có key cho việc này (key của server chỉ dành cho tenant chính): 403 `TENANT_KEY_MISSING`.
pub fn key_missing() -> AppError {
    AppError::Coded { status: StatusCode::FORBIDDEN, code: "TENANT_KEY_MISSING", message: "Chủ domain chưa thêm key cho việc này".into() }
}

/// Tính năng chỉ của tenant chính (liên kết chat, gửi biên bản qua bot, chép trực tiếp): tenant sổ ⟹ 403
/// `NOT_ON_TENANT`.
pub async fn primary_feature(Extension(tenant): Extension<Tenant>, req: Request, next: Next) -> Response {
    if tenant.primary {
        next.run(req).await
    } else {
        AppError::Coded { status: StatusCode::FORBIDDEN, code: "NOT_ON_TENANT", message: "Tính năng này chỉ có trên hoandat.com".into() }
            .into_response()
    }
}

/// Route chỉ có ở tenant chính (đăng nhập của app di động: app chỉ nói với tenant chính): tenant sổ ⟹ 404.
pub async fn primary_only(Extension(tenant): Extension<Tenant>, req: Request, next: Next) -> Response {
    if tenant.primary {
        next.run(req).await
    } else {
        AppError::NotFound("không có trên domain này".into()).into_response()
    }
}

#[cfg(test)]
pub(crate) fn test_tenant(id: &str, enabled: bool) -> Tenant {
    let primary = id == PRIMARY_ID;
    let domain = if primary { "hoandat.com".to_string() } else { format!("{id}.example") };
    Arc::new(TenantCtx {
        id: id.to_string(),
        issuer: format!("https://auth.{domain}"),
        public_url: format!("https://scribe.{domain}"),
        display_name: id.to_string(),
        domain,
        primary,
        enabled: primary || enabled,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_are_normalized() {
        assert_eq!(normalize_host("Scribe.Example.COM:443"), Some("scribe.example.com".into()));
        assert_eq!(normalize_host("scribe.example.com."), Some("scribe.example.com".into()));
        assert_eq!(normalize_host("127.0.0.1:8787"), Some("127.0.0.1".into()));
        assert_eq!(normalize_host("[::1]:8787"), Some("::1".into()));
        assert_eq!(normalize_host(""), None);
        assert_eq!(normalize_host("evil.com/x"), None);
        assert_eq!(normalize_host("a b"), None);
        assert!(is_loopback("localhost") && is_loopback("127.0.0.1") && is_loopback("::1"));
        assert!(!is_loopback("scribe.hoandat.com"));
        assert_eq!(host_of_url("https://Scribe.Hoandat.com"), Some("scribe.hoandat.com".into()));
        assert_eq!(host_of_url("http://localhost:5174"), Some("localhost".into()));
        assert_eq!(host_of_url("not a url"), None);
    }

    #[test]
    fn readiness_needs_every_condition() {
        assert!(!Readiness::default().ready());
        assert!(!Readiness { oidc: true, sealed_keys: false }.ready());
        assert!(!Readiness { oidc: false, sealed_keys: true }.ready());
        assert!(Readiness { oidc: true, sealed_keys: true }.ready());
    }

    /// Trên Mongo thật: `TRANSCRIPT_TEST_MONGO_URI=… TRANSCRIPT_TEST_DB_NAME=… cargo test tenant_flags -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn tenant_flags_on_real_mongo() {
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        let id = format!("ten_test_{}", uuid::Uuid::new_v4().simple());
        assert!(!is_disabled(&db, &id).await.expect("đọc"));
        assert!(set_disabled(&db, &id, true).await.expect("tắt"));
        assert!(!set_disabled(&db, &id, true).await.expect("tắt lại"), "đã tắt rồi");
        assert!(is_disabled(&db, &id).await.expect("đọc"));
        assert!(!is_disabled(&db, "ten_other").await.expect("tenant khác không bị tắt kèm"));
        assert!(set_disabled(&db, &id, false).await.expect("bật"));
        assert!(!is_disabled(&db, &id).await.expect("đọc"));
    }
}
