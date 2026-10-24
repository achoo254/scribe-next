//! Đường tới IdP của một issuer, cho `oidc::Oidc`:
//!
//! - [`Transport::Public`] (tenant chính, như trước): discovery ở `{issuer}/.well-known/openid-configuration`, mọi
//!   endpoint theo URL công khai trong discovery.
//! - [`Transport::Internal`] (tenant sổ, theo mẫu `localman-next/packages/tenancy/src/tenant-idp.ts`): discovery và
//!   JWKS đọc qua API nội bộ của IdP (`/internal/tenants/:id/openid-configuration`, `/jwks`, Basic `scribe`), token,
//!   userinfo và thu hồi gửi tới listener công khai của IdP trên loopback (`IDP_LOOPBACK_URL`) với `Host` của issuer
//!   và `X-Forwarded-Proto: https`, không theo redirect. DNS của domain (do chủ tenant giữ) không bao giờ quyết định
//!   secret client và code đi đâu; URL ngoài origin của issuer bị từ chối. Trang đăng nhập và trang đăng xuất của IdP
//!   vẫn là URL công khai trong discovery (trình duyệt mở).

use crate::tenant::internal_api::InternalApi;
use reqwest::header::HOST;
use reqwest::{Method, RequestBuilder, Url};
use std::sync::Arc;
use std::time::Duration;

const INTERNAL_TIMEOUT: Duration = Duration::from_secs(10);

pub enum Transport {
    Public,
    Internal(InternalRoute),
}

/// Đường tới IdP của một tenant sổ.
pub struct InternalRoute {
    tenant_id: String,
    api: Arc<InternalApi>,
    /// Origin của listener công khai của IdP trên loopback (`http://127.0.0.1:4500`).
    loopback: Url,
    /// `scheme://host[:port]` của issuer: endpoint phải cùng origin.
    issuer_origin: String,
    /// `host[:port]` của issuer, gửi làm `Host` để IdP chọn đúng tenant.
    issuer_host: String,
    /// Client không theo redirect.
    http: reqwest::Client,
}

/// Id tenant dùng được trong đường dẫn API nội bộ (id của auth-next: chữ, số, `_`, `-`).
pub fn valid_tenant_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

impl InternalRoute {
    pub fn new(tenant_id: &str, issuer: &str, api: Arc<InternalApi>, loopback: &str) -> Result<InternalRoute, String> {
        if !valid_tenant_id(tenant_id) {
            return Err("id tenant không hợp lệ".into());
        }
        let issuer_url = Url::parse(issuer).map_err(|_| "issuer không phải URL".to_string())?;
        let host = issuer_url.host_str().ok_or("issuer không có host")?;
        let issuer_host = match issuer_url.port() {
            Some(port) => format!("{host}:{port}"),
            None => host.to_string(),
        };
        let issuer_origin = issuer_url.origin().ascii_serialization();
        let loopback = Url::parse(loopback).map_err(|_| "IDP_LOOPBACK_URL không phải URL".to_string())?;
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| format!("không dựng được HTTP client: {e}"))?;
        Ok(InternalRoute { tenant_id: tenant_id.to_string(), api, loopback, issuer_origin, issuer_host, http })
    }
}

impl Transport {
    /// Request lấy discovery của `issuer`.
    pub fn discovery(&self, http: &reqwest::Client, issuer: &str) -> RequestBuilder {
        match self {
            Transport::Public => http.get(format!("{issuer}/.well-known/openid-configuration")).timeout(INTERNAL_TIMEOUT),
            Transport::Internal(r) => r.api.get(&format!("/internal/tenants/{}/openid-configuration", r.tenant_id), INTERNAL_TIMEOUT),
        }
    }

    /// Request lấy JWKS (`jwks_uri` của discovery với tenant chính).
    pub fn jwks(&self, http: &reqwest::Client, jwks_uri: &str) -> RequestBuilder {
        match self {
            Transport::Public => http.get(jwks_uri).timeout(INTERNAL_TIMEOUT),
            Transport::Internal(r) => r.api.get(&format!("/internal/tenants/{}/jwks", r.tenant_id), INTERNAL_TIMEOUT),
        }
    }

    /// Discovery phải là của chính issuer này khi đọc qua API nội bộ (không bao giờ dùng discovery của tenant khác).
    pub fn checks_issuer(&self) -> bool {
        matches!(self, Transport::Internal(_))
    }

    /// Request tới một endpoint của IdP (token, userinfo, thu hồi). Lỗi: URL ngoài origin của issuer.
    pub fn endpoint(&self, http: &reqwest::Client, method: Method, url: &str) -> Result<RequestBuilder, String> {
        match self {
            Transport::Public => Ok(http.request(method, url)),
            Transport::Internal(r) => {
                let target = Url::parse(url).map_err(|_| "endpoint của IdP không phải URL".to_string())?;
                if target.origin().ascii_serialization() != r.issuer_origin {
                    return Err("từ chối gọi ra ngoài IdP của tenant".into());
                }
                let mut via = r.loopback.clone();
                via.set_path(target.path());
                via.set_query(target.query());
                Ok(r.http.request(method, via).header(HOST, &r.issuer_host).header("x-forwarded-proto", "https"))
            }
        }
    }
}
