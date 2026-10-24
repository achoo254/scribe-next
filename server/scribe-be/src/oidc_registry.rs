//! Một client OIDC (`oidc::Oidc`) cho mỗi tenant. Tenant chính dùng instance đường công khai dựng lúc khởi động và
//! secret client đọc từ tệp như trước. Tenant sổ: instance dựng lười theo `tenantId` qua API nội bộ + loopback
//! (`oidc_transport`), client `scribe-web`, secret = `base64url(HMAC-SHA256(khoá app scribe-web, tenantId))` — cùng
//! cách IdP suy ra (`auth-next/src/tenants/app-client-secrets.ts`), nên Scribe chỉ giữ khoá app, không giữ secret của
//! tenant nào.
//!
//! Chế độ tenant (đăng nhập ở tenant sổ) chỉ bật khi có đủ API nội bộ (`IDP_INTERNAL_API_URL` +
//! `IDP_INTERNAL_API_SECRET_PATH`), `IDP_LOOPBACK_URL` và `SCRIBE_WEB_CLIENT_KEY_PATH`; thiếu thì tenant sổ "chưa bật".

use crate::config::Config;
use crate::error::AppError;
use crate::oidc::Oidc;
use crate::oidc_transport::InternalRoute;
use crate::tenant::internal_api::InternalApi;
use crate::tenant::TenantCtx;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use zeroize::Zeroizing;

/// Client web của Scribe ở mọi tenant sổ (hợp đồng với auth-next).
pub const TENANT_WEB_CLIENT_ID: &str = "scribe-web";
/// Giữ tối đa chừng này client tenant sổ; quá thì bỏ hết, request sau dựng lại (rẻ: discovery và JWKS tải lại).
const MAX_TENANTS: usize = 1_000;

/// Những gì cần để đăng nhập ở tenant sổ. Không `Debug`: có khoá app.
pub struct TenantAccess {
    api: Arc<InternalApi>,
    loopback: String,
    client_key: Zeroizing<String>,
}

impl TenantAccess {
    /// Đủ cấu hình thì đọc khoá app; thiếu một phần ⟹ `Ok(None)` và log một dòng (chế độ tenant tắt). Tệp khoá hỏng hay
    /// `IDP_LOOPBACK_URL` sai dạng ⟹ `Err`.
    pub fn load(config: &Config, api: Option<Arc<InternalApi>>) -> Result<Option<TenantAccess>, String> {
        let (Some(api), Some(loopback), Some(key_path)) = (api.clone(), &config.idp_loopback_url, &config.scribe_web_client_key_path) else {
            let missing: Vec<&str> = [
                ("IDP_INTERNAL_API_URL/IDP_INTERNAL_API_SECRET_PATH", api.is_none()),
                ("IDP_LOOPBACK_URL", config.idp_loopback_url.is_none()),
                ("SCRIBE_WEB_CLIENT_KEY_PATH", config.scribe_web_client_key_path.is_none()),
            ]
            .into_iter()
            .filter_map(|(name, absent)| absent.then_some(name))
            .collect();
            tracing::info!("chế độ tenant tắt (thiếu {}) — chỉ phục vụ tenant chính", missing.join(", "));
            return Ok(None);
        };
        reqwest::Url::parse(loopback).map_err(|_| "IDP_LOOPBACK_URL không phải URL".to_string())?;
        let client_key = read_client_key(key_path)?;
        Ok(Some(TenantAccess { api, loopback: loopback.clone(), client_key }))
    }
}

/// Khoá app `scribe-web` trong tệp 600: một dòng, không khoảng trắng, tối thiểu 32 ký tự (base64url 43 ký tự).
fn read_client_key(path: &str) -> Result<Zeroizing<String>, String> {
    let p = std::path::Path::new(path);
    let meta = std::fs::metadata(p).map_err(|e| format!("không đọc được tệp khoá app scribe-web ({})", e.kind()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 {
            return Err("tệp khoá app scribe-web phải có quyền 600 (chỉ chủ tệp đọc được)".into());
        }
    }
    #[cfg(not(unix))]
    let _ = meta;
    let raw = Zeroizing::new(std::fs::read_to_string(p).map_err(|e| format!("không đọc được tệp khoá app scribe-web ({})", e.kind()))?);
    let key = raw.trim();
    if key.len() < 32 || key.chars().any(char::is_whitespace) {
        return Err("tệp khoá app scribe-web không đúng dạng (một dòng, tối thiểu 32 ký tự)".into());
    }
    Ok(Zeroizing::new(key.to_string()))
}

/// Secret client của một app ở tenant `tenant_id`: base64url(HMAC-SHA256(khoá app, tenant_id)), khoá app dùng nguyên
/// chuỗi (UTF-8) như IdP.
pub fn tenant_client_secret(client_key: &str, tenant_id: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(client_key.as_bytes()).expect("HMAC nhận khoá mọi độ dài");
    mac.update(tenant_id.as_bytes());
    URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
}

pub struct OidcRegistry {
    primary: Arc<Oidc>,
    primary_client_id: String,
    primary_secret: Zeroizing<String>,
    tenants: Option<TenantAccess>,
    by_tenant: Mutex<HashMap<String, Arc<Oidc>>>,
}

impl OidcRegistry {
    pub fn new(primary: Oidc, primary_client_id: &str, primary_secret: String, tenants: Option<TenantAccess>) -> OidcRegistry {
        OidcRegistry {
            primary: Arc::new(primary),
            primary_client_id: primary_client_id.to_string(),
            primary_secret: Zeroizing::new(primary_secret),
            tenants,
            by_tenant: Mutex::default(),
        }
    }

    /// IdP của tenant chính (app di động, thu hồi grant của thiết bị).
    pub fn primary(&self) -> &Arc<Oidc> {
        &self.primary
    }

    /// Đăng nhập ở tenant sổ dựng được (điều kiện `tenant::Readiness::oidc`).
    pub fn tenant_mode(&self) -> bool {
        self.tenants.is_some()
    }

    /// Client OIDC của tenant: tenant chính ⟹ instance công khai; tenant sổ ⟹ instance theo `tenantId`, dựng lại khi
    /// issuer đổi.
    pub fn get(&self, tenant: &TenantCtx) -> Result<Arc<Oidc>, AppError> {
        if tenant.primary {
            return Ok(self.primary.clone());
        }
        let access = self.tenants.as_ref().ok_or_else(|| AppError::Unavailable("Scribe chưa bật trên domain này".into()))?;
        let mut map = self.by_tenant.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(oidc) = map.get(&tenant.id).filter(|o| o.issuer == tenant.issuer.trim_end_matches('/')) {
            return Ok(oidc.clone());
        }
        let route = InternalRoute::new(&tenant.id, &tenant.issuer, access.api.clone(), &access.loopback)
            .map_err(|e| AppError::Internal(format!("dựng client OIDC tenant {}: {e}", tenant.id)))?;
        let oidc = Arc::new(Oidc::internal(&tenant.issuer, route));
        if map.len() >= MAX_TENANTS {
            map.clear();
        }
        map.insert(tenant.id.clone(), oidc.clone());
        Ok(oidc)
    }

    /// `client_id` của Scribe web ở tenant: tenant chính theo cấu hình (`scribe-web`, dev `scribe-web-dev`).
    pub fn web_client_id<'a>(&'a self, tenant: &TenantCtx) -> &'a str {
        if tenant.primary {
            &self.primary_client_id
        } else {
            TENANT_WEB_CLIENT_ID
        }
    }

    /// Secret client web của tenant (không bao giờ log).
    pub fn client_secret(&self, tenant: &TenantCtx) -> Result<Zeroizing<String>, AppError> {
        if tenant.primary {
            return Ok(self.primary_secret.clone());
        }
        let access = self.tenants.as_ref().ok_or_else(|| AppError::Unavailable("Scribe chưa bật trên domain này".into()))?;
        Ok(Zeroizing::new(tenant_client_secret(&access.client_key, &tenant.id)))
    }
}

#[cfg(test)]
pub(crate) mod tests;
