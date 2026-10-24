//! Danh bạ tenant: Host → [`Tenant`]. Tenant chính tĩnh theo `PUBLIC_URL` (không hỏi IdP khi nhận request); tenant sổ
//! tra `GET {IDP_INTERNAL_API_URL}/internal/tenants/lookup?host=` (Basic `scribe`) và giữ trong [`AnswerCache`]
//! (60 giây, 404 30 giây, IdP lỗi thì dùng câu cũ).
//!
//! Tên tài khoản đăng nhập của tenant chính (`/api/idp`, giao diện gọi "Đăng nhập bằng …") vẫn hỏi nền lúc khởi động
//! rồi mỗi giờ như trước; lỗi giữ tên đúng gần nhất, chưa hỏi được lần nào thì dùng `IDP_DISPLAY_NAME`.

use super::answer_cache::AnswerCache;
use super::internal_api::{send_error, InternalApi};
use super::{host_of_url, is_disabled, is_loopback, normalize_host, Readiness, Tenant, TenantCtx, HOST_PREFIX, PRIMARY_ID};
use crate::config::Config;
use crate::idp_brand::clean_name;
use mongodb::Database;
use serde::Deserialize;
use std::sync::{Arc, RwLock};
use std::time::Duration;

const LOOKUP_TIMEOUT: Duration = Duration::from_secs(5);
const REFRESH_PRIMARY_EVERY: Duration = Duration::from_secs(60 * 60);
/// Lỗi thì hỏi lại sớm hơn: IdP khởi động lại hoặc chưa cấp quyền cho `scribe` lúc deploy.
const RETRY_PRIMARY_AFTER_ERROR: Duration = Duration::from_secs(5 * 60);

/// Câu trả lời của API tra tenant (không có gì bí mật).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lookup {
    pub id: String,
    pub domain: String,
    pub display_name: String,
    pub issuer: String,
    pub status: String,
    #[serde(default)]
    pub web_apps: WebApps,
}

#[derive(Debug, Default, Deserialize)]
pub struct WebApps {
    /// Có khi tenant đã bật Scribe ở IdP (`SCRIBE_TENANTS` và chủ domain đã nhập key chép lời).
    #[serde(default)]
    pub scribe: Option<String>,
}

/// Hỏi IdP tenant phục vụ `host`. `Ok(None)` = IdP trả 404. Lỗi trả mô tả không chứa bí mật.
pub async fn fetch_lookup(api: &InternalApi, host: &str) -> Result<Option<Lookup>, String> {
    let res = api
        .get("/internal/tenants/lookup", LOOKUP_TIMEOUT)
        .query(&[("host", host)])
        .send()
        .await
        .map_err(|e| send_error(&e))?;
    let status = res.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !status.is_success() {
        return Err(format!("IdP trả {status}"));
    }
    res.json().await.map(Some).map_err(|_| "IdP trả JSON không đúng dạng".to_string())
}

pub struct TenantDirectory {
    primary_host: String,
    primary: RwLock<Tenant>,
    internal: Option<Arc<InternalApi>>,
    /// Nơi đọc cờ `admin tenant-disable`; `None` chỉ ở test không có Mongo (không tenant nào bị tắt).
    flags: Option<Database>,
    readiness: Readiness,
    cache: AnswerCache<Tenant>,
}

/// Domain của tenant chính: host của `PUBLIC_URL` bỏ nhãn `scribe.` (`scribe.hoandat.com` ⟹ `hoandat.com`).
fn primary_domain(host: &str) -> String {
    host.strip_prefix(HOST_PREFIX).filter(|d| !d.is_empty()).unwrap_or(host).to_string()
}

impl TenantDirectory {
    pub fn new(config: &Config, flags: Option<Database>, internal: Option<Arc<InternalApi>>, readiness: Readiness) -> TenantDirectory {
        let primary_host = host_of_url(&config.public_url).unwrap_or_default();
        let primary = Arc::new(TenantCtx {
            id: PRIMARY_ID.to_string(),
            domain: primary_domain(&primary_host),
            issuer: config.oidc_issuer.trim_end_matches('/').to_string(),
            public_url: config.public_url.clone(),
            display_name: config.idp_display_name.clone(),
            primary: true,
            enabled: true,
        });
        TenantDirectory { primary_host, primary: RwLock::new(primary), internal, flags, readiness, cache: AnswerCache::new() }
    }

    pub fn primary(&self) -> Tenant {
        match self.primary.read() {
            Ok(t) => t.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    fn set_primary_name(&self, name: String) {
        let next = Arc::new(TenantCtx { display_name: name, ..(*self.primary()).clone() });
        match self.primary.write() {
            Ok(mut t) => *t = next,
            Err(poisoned) => *poisoned.into_inner() = next,
        }
    }

    /// Tenant phục vụ `raw_host` (header Host). `Ok(None)`: không phải host của Scribe ở tenant nào.
    pub async fn resolve(&self, raw_host: Option<&str>) -> Result<Option<Tenant>, String> {
        let Some(host) = raw_host.and_then(normalize_host) else { return Ok(None) };
        if host == self.primary_host || is_loopback(&host) {
            return Ok(Some(self.primary()));
        }
        if !host.starts_with(HOST_PREFIX) || host.len() == HOST_PREFIX.len() || self.internal.is_none() {
            return Ok(None);
        }
        self.cache.get(&host, || self.load(&host)).await
    }

    async fn load(&self, host: &str) -> Result<Option<Tenant>, String> {
        let Some(api) = &self.internal else { return Ok(None) };
        let Some(lookup) = fetch_lookup(api, host).await? else { return Ok(None) };
        let domain = lookup.domain.trim().to_ascii_lowercase();
        // Chỉ host của `PUBLIC_URL` là tenant chính; host phải đúng `scribe.<domain>` của tenant IdP trả về.
        if lookup.id.is_empty() || lookup.id == PRIMARY_ID || format!("{HOST_PREFIX}{domain}") != host {
            tracing::warn!(host, tenant = %lookup.id, "IdP trả tenant không khớp host — coi như không có");
            return Ok(None);
        }
        let disabled = match &self.flags {
            Some(db) => is_disabled(db, &lookup.id).await?,
            None => false,
        };
        let public_url = format!("https://{host}");
        let listed = lookup.web_apps.scribe.as_deref().map(|u| u.trim_end_matches('/')) == Some(public_url.as_str());
        let enabled = listed && lookup.status == "active" && self.readiness.ready() && !disabled;
        Ok(Some(Arc::new(TenantCtx {
            display_name: clean_name(&lookup.display_name).unwrap_or_else(|| domain.clone()),
            id: lookup.id,
            issuer: lookup.issuer.trim_end_matches('/').to_string(),
            domain,
            public_url,
            primary: false,
            enabled,
        })))
    }
}

/// Lúc khởi động: có API nội bộ thì chạy task nền hỏi tên tenant chính ngay rồi mỗi giờ.
pub fn start_primary_refresh(directory: Arc<TenantDirectory>) {
    let Some(api) = directory.internal.clone() else {
        tracing::info!("tên IdP: chưa cấu hình API nội bộ — dùng tên mặc định");
        return;
    };
    tokio::spawn(async move {
        let host = directory.primary_host.clone();
        loop {
            let name = match fetch_lookup(&api, &host).await {
                Ok(Some(lookup)) => clean_name(&lookup.display_name).ok_or_else(|| "IdP trả displayName không dùng được".to_string()),
                Ok(None) => Err("IdP trả 404 Not Found".to_string()),
                Err(e) => Err(e),
            };
            let current = directory.primary().display_name.clone();
            let wait = match name {
                Ok(name) => {
                    if name != current {
                        tracing::info!(name = %name, "tên IdP cập nhật");
                        directory.set_primary_name(name);
                    }
                    REFRESH_PRIMARY_EVERY
                }
                Err(e) => {
                    tracing::warn!(current = %current, "tên IdP: {e} — giữ tên đang dùng");
                    RETRY_PRIMARY_AFTER_ERROR
                }
            };
            tokio::time::sleep(wait).await;
        }
    });
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use axum::extract::Query;
    use axum::http::{HeaderMap, StatusCode};
    use axum::response::IntoResponse;
    use axum::routing::get;
    use axum::Router;
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    pub const SECRET: &str = "s3cret-s3cret-s3cret-s3cret-s3cret";

    /// API nội bộ giả: chỉ nhận đúng Basic `scribe:<SECRET>`, trả tenant theo host từ `tenants`; đếm số lần hỏi.
    pub async fn fake_lookup(tenants: HashMap<String, Value>) -> (String, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        let handler = move |headers: HeaderMap, Query(q): Query<HashMap<String, String>>| {
            let (tenants, counter) = (tenants.clone(), counter.clone());
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                let expected = format!("Basic {}", STANDARD.encode(format!("scribe:{SECRET}")));
                if headers.get("authorization").and_then(|v| v.to_str().ok()) != Some(expected.as_str()) {
                    return (StatusCode::UNAUTHORIZED, "{}").into_response();
                }
                match q.get("host").and_then(|h| tenants.get(h)) {
                    Some(body) => axum::Json(body.clone()).into_response(),
                    None => (StatusCode::NOT_FOUND, axum::Json(json!({ "error": "unknown_tenant" }))).into_response(),
                }
            }
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let api = format!("http://{}", listener.local_addr().expect("addr"));
        let app = Router::new().route("/internal/tenants/lookup", get(handler));
        tokio::spawn(async move { axum::serve(listener, app).await });
        (api, calls)
    }

    pub fn tenant_body(id: &str, domain: &str, scribe: bool) -> Value {
        let mut web_apps = json!({ "localman": format!("https://localman.{domain}") });
        if scribe {
            web_apps["scribe"] = json!(format!("https://scribe.{domain}"));
        }
        json!({ "id": id, "domain": domain, "displayName": format!("  {domain} login "), "issuer": format!("https://auth.{domain}"),
                "status": "active", "webApps": web_apps, "ownerSubs": [] })
    }

    /// Config tối thiểu: tenant chính ở `https://scribe.hoandat.com`.
    pub fn directory(api: Option<&str>, readiness: Readiness) -> TenantDirectory {
        let config = crate::config::Config::for_tests();
        let internal = api.map(|a| Arc::new(InternalApi::new(a, SECRET).expect("client")));
        TenantDirectory::new(&config, None, internal, readiness)
    }

    #[tokio::test]
    async fn primary_host_and_loopback_never_ask_the_idp() {
        let (api, calls) = fake_lookup(HashMap::new()).await;
        let dir = directory(Some(&api), Readiness::default());
        for host in ["scribe.hoandat.com", "SCRIBE.hoandat.com:443", "127.0.0.1:8787", "localhost:5173"] {
            let t = dir.resolve(Some(host)).await.expect("ok").expect("tenant chính");
            assert!(t.primary && t.enabled && t.id == PRIMARY_ID, "{host}");
            assert_eq!((t.domain.as_str(), t.public_url.as_str()), ("hoandat.com", "https://scribe.hoandat.com"));
        }
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        // Host không phải `scribe.` của tenant nào: 404 mà không hỏi IdP.
        for host in ["evil.example.com", "auth.hoandat.com", "scribe.", ""] {
            assert!(dir.resolve(Some(host)).await.expect("ok").is_none(), "{host}");
        }
        assert!(dir.resolve(None).await.expect("ok").is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn tenants_come_from_the_idp_and_stay_off_until_scribe_is_ready() {
        let tenants = HashMap::from([
            ("scribe.inetdev.io.vn".to_string(), tenant_body("ten_a", "inetdev.io.vn", true)),
            ("scribe.no-key.vn".to_string(), tenant_body("ten_b", "no-key.vn", false)),
            // IdP trả tenant chính cho host khác: không bao giờ là tenant chính.
            ("scribe.alias.vn".to_string(), tenant_body(PRIMARY_ID, "alias.vn", true)),
            // Domain không khớp host.
            ("scribe.mismatch.vn".to_string(), tenant_body("ten_c", "other.vn", true)),
        ]);
        let (api, calls) = fake_lookup(tenants).await;
        let off = directory(Some(&api), Readiness { oidc: true, sealed_keys: false });
        let a = off.resolve(Some("scribe.inetdev.io.vn")).await.expect("ok").expect("tenant");
        assert_eq!(
            (a.id.as_str(), a.domain.as_str(), a.issuer.as_str(), a.display_name.as_str(), a.primary, a.enabled),
            ("ten_a", "inetdev.io.vn", "https://auth.inetdev.io.vn", "inetdev.io.vn login", false, false)
        );
        assert_eq!(a.public_url, "https://scribe.inetdev.io.vn");
        // Lần hai trong 60 giây: từ cache.
        off.resolve(Some("scribe.inetdev.io.vn")).await.expect("ok");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(off.resolve(Some("scribe.unknown.vn")).await.expect("ok").is_none());
        assert!(off.resolve(Some("scribe.alias.vn")).await.expect("ok").is_none());
        assert!(off.resolve(Some("scribe.mismatch.vn")).await.expect("ok").is_none());
        // Không có API nội bộ: không tenant sổ nào.
        let none = directory(None, Readiness { oidc: true, sealed_keys: true });
        assert!(none.resolve(Some("scribe.inetdev.io.vn")).await.expect("ok").is_none());
    }

    #[tokio::test]
    async fn an_idp_failure_with_nothing_cached_is_an_error() {
        let dir = directory(Some("http://127.0.0.1:1"), Readiness::default());
        let err = dir.resolve(Some("scribe.inetdev.io.vn")).await.expect_err("IdP không chạy");
        assert!(!err.contains(SECRET), "lỗi không lộ bí mật");
        // Sai bí mật: IdP trả 401, không phải 404.
        let (api, _) = fake_lookup(HashMap::new()).await;
        let wrong = InternalApi::new(&api, "other-secret-other-secret-other-se").expect("client");
        assert_eq!(fetch_lookup(&wrong, "scribe.x.vn").await.expect_err("401"), "IdP trả 401 Unauthorized");
    }

    #[test]
    fn primary_domain_drops_the_scribe_label() {
        assert_eq!(primary_domain("scribe.hoandat.com"), "hoandat.com");
        assert_eq!(primary_domain("localhost"), "localhost");
    }
}
