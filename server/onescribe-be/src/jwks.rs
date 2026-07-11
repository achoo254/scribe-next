//! Fetch + cache JWKS securetoken (Firebase) và verify Firebase ID token (RS256).

use crate::config::{firebase_issuer, FIREBASE_JWKS_URL, FIREBASE_PROJECT_ID};
use crate::error::AppError;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Claims cần từ Firebase ID token.
#[derive(Debug, Deserialize)]
pub struct FirebaseClaims {
    pub sub: String, // Firebase UID
    pub email: String,
    pub email_verified: bool,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Deserialize)]
struct Jwk {
    kid: String,
    n: String,
    e: String,
}

#[derive(Deserialize)]
struct JwkSet {
    keys: Vec<Jwk>,
}

/// Chặn refetch dồn dập khi token có kid lạ trên endpoint public (DoS amplification): không gọi
/// lại Google trong khoảng này nếu cache vẫn còn hạn TTL mà kid không có.
const REFETCH_COOLDOWN: Duration = Duration::from_secs(30);

/// Cache khoá công khai (kid → (n,e) base64url) + hạn theo Cache-Control + mốc fetch gần nhất.
#[derive(Default)]
struct CacheInner {
    keys: HashMap<String, (String, String)>,
    expires_at: Option<Instant>,
    last_fetch: Option<Instant>,
}

pub struct JwksCache {
    inner: RwLock<CacheInner>,
}

impl JwksCache {
    pub fn new() -> Self {
        JwksCache { inner: RwLock::new(CacheInner::default()) }
    }

    /// Verify Firebase ID token đủ RS256 + aud + iss + exp + email_verified.
    pub async fn verify_id_token(
        &self,
        http: &reqwest::Client,
        token: &str,
    ) -> Result<FirebaseClaims, AppError> {
        let header = decode_header(token)
            .map_err(|_| AppError::Unauthorized("token header không hợp lệ".into()))?;
        let kid = header
            .kid
            .ok_or_else(|| AppError::Unauthorized("token thiếu kid".into()))?;

        let (n, e) = self.key_for(http, &kid).await?;
        let key = DecodingKey::from_rsa_components(&n, &e)
            .map_err(|e| AppError::Internal(format!("dựng key: {e}")))?;

        let mut val = Validation::new(Algorithm::RS256);
        val.set_audience(&[FIREBASE_PROJECT_ID]);
        val.set_issuer(&[firebase_issuer()]);
        val.leeway = 60;

        let data = decode::<FirebaseClaims>(token, &key, &val)
            .map_err(|_| AppError::Unauthorized("token không hợp lệ hoặc hết hạn".into()))?;

        if !data.claims.email_verified {
            return Err(AppError::Forbidden("email chưa xác minh".into()));
        }
        Ok(data.claims)
    }

    /// Lấy (n,e) cho kid: dùng cache nếu còn hạn + có kid; nếu thiếu/hết hạn → refetch.
    async fn key_for(
        &self,
        http: &reqwest::Client,
        kid: &str,
    ) -> Result<(String, String), AppError> {
        {
            let inner = self.inner.read().await;
            let now = Instant::now();
            let ttl_fresh = inner.expires_at.map(|t| t > now).unwrap_or(false);
            if ttl_fresh {
                if let Some(v) = inner.keys.get(kid) {
                    return Ok(v.clone());
                }
                // Cache còn hạn nhưng kid lạ: chỉ refetch nếu đã qua cooldown → chặn attacker gửi
                // kid ngẫu nhiên ép BE gọi Google mỗi request (endpoint /api/auth/session public).
                let cooled = inner
                    .last_fetch
                    .map(|t| now.duration_since(t) < REFETCH_COOLDOWN)
                    .unwrap_or(false);
                if cooled {
                    return Err(AppError::Unauthorized("không tìm thấy khoá cho kid".into()));
                }
            }
        }
        // Cache hết hạn TTL, hoặc kid miss và đã qua cooldown → refetch (key rotation → không hard-fail).
        self.refetch(http).await?;
        let inner = self.inner.read().await;
        inner
            .keys
            .get(kid)
            .cloned()
            .ok_or_else(|| AppError::Unauthorized("không tìm thấy khoá cho kid".into()))
    }

    async fn refetch(&self, http: &reqwest::Client) -> Result<(), AppError> {
        let resp = http
            .get(FIREBASE_JWKS_URL)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("fetch JWKS: {e}")))?;
        let ttl = cache_max_age(resp.headers());
        let set: JwkSet = resp
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("parse JWKS: {e}")))?;
        let now = Instant::now();
        let mut inner = self.inner.write().await;
        inner.keys = set.keys.into_iter().map(|k| (k.kid, (k.n, k.e))).collect();
        inner.expires_at = Some(now + ttl);
        inner.last_fetch = Some(now);
        Ok(())
    }
}

/// Đọc max-age từ Cache-Control (fallback 1h) — Google xoay khoá định kỳ.
fn cache_max_age(headers: &reqwest::header::HeaderMap) -> Duration {
    let secs = headers
        .get(reqwest::header::CACHE_CONTROL)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| {
            s.split(',')
                .find_map(|p| p.trim().strip_prefix("max-age=").and_then(|n| n.parse::<u64>().ok()))
        })
        .unwrap_or(3600);
    Duration::from_secs(secs.max(60))
}
