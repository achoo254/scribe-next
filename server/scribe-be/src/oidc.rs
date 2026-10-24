//! Client OIDC của hoandat-auth cho một issuer (một tenant): discovery, JWKS (EC P-256 + RSA, chọn theo `kid`), xác
//! minh ID token và logout token, đổi code, làm mới refresh token. Scribe chỉ dùng ID token lúc đăng nhập rồi cấp phiên
//! riêng (`models::session`), không dùng access token của IdP. Đường tới IdP (công khai cho tenant chính, API nội bộ +
//! loopback cho tenant sổ) ở `oidc_transport`; mỗi tenant một `Oidc` (`oidc_registry`).

use crate::error::AppError;
use crate::oidc_transport::{InternalRoute, Transport};
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use std::collections::HashSet;
use std::time::{Duration, Instant};
use tokio::sync::{OnceCell, RwLock};

/// Không tải lại JWKS dồn dập khi gặp `kid` lạ (route đăng nhập là public).
const REFETCH_COOLDOWN: Duration = Duration::from_secs(30);
const JWKS_TTL: Duration = Duration::from_secs(3600);
/// Logout token chỉ được nhận trong 2 phút sau khi phát (hợp đồng của IdP).
const LOGOUT_TOKEN_MAX_AGE_S: i64 = 120;
const BACKCHANNEL_EVENT: &str = "http://schemas.openid.net/event/backchannel-logout";
pub const SCOPE: &str = "openid profile email";

#[derive(Debug, Deserialize)]
pub struct Discovery {
    #[serde(default)]
    pub issuer: Option<String>,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub jwks_uri: String,
    #[serde(default)]
    pub end_session_endpoint: Option<String>,
    #[serde(default)]
    pub revocation_endpoint: Option<String>,
    #[serde(default)]
    pub userinfo_endpoint: Option<String>,
}

/// Hồ sơ từ `userinfo`: hoandat-auth chỉ đưa `email`/`email_verified`/`name` ra đây với code flow, không
/// đưa vào ID token.
#[derive(Debug, Deserialize)]
struct UserInfo {
    sub: String,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    email_verified: bool,
    #[serde(default)]
    name: Option<String>,
}

/// Claim cần từ ID token.
#[derive(Debug, Deserialize)]
pub struct IdClaims {
    pub sub: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub email_verified: bool,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub nonce: Option<String>,
    /// Phiên ở IdP (có khi client bật back-channel logout).
    #[serde(default)]
    pub sid: Option<String>,
    #[serde(default)]
    aud: serde_json::Value,
    #[serde(default)]
    azp: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LogoutClaims {
    #[serde(default)]
    pub sub: Option<String>,
    #[serde(default)]
    pub sid: Option<String>,
    pub jti: String,
    pub iat: i64,
    #[serde(default)]
    pub events: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(default)]
    pub nonce: Option<serde_json::Value>,
}

/// Token nhận từ IdP. Không `Debug` để khỏi in refresh token ra log.
pub struct TokenSet {
    pub id_token: String,
    /// Access token opaque, chỉ dùng gọi `userinfo`.
    pub access_token: String,
    pub refresh_token: Option<String>,
}

#[derive(Deserialize)]
struct TokenResponse {
    id_token: Option<String>,
    #[serde(default)]
    access_token: String,
    refresh_token: Option<String>,
}

#[derive(Deserialize)]
struct TokenError {
    error: String,
}

#[derive(Default)]
struct KeyCache {
    set: Option<JwkSet>,
    fetched: Option<Instant>,
}

/// Lỗi khi đổi/làm mới token. Chỉ `invalid_grant` là IdP thật sự từ chối grant (code sai/đã dùng, máy bị
/// thu hồi); mọi lỗi khác (429, `invalid_client`, 5xx, mạng) là tạm thời hoặc cấu hình — không được coi là
/// thu hồi, kẻo xoá nhầm thiết bị.
#[derive(Debug)]
pub enum GrantError {
    Rejected(String),
    Unavailable(String),
}

pub struct Oidc {
    pub issuer: String,
    transport: Transport,
    discovery: OnceCell<Discovery>,
    keys: RwLock<KeyCache>,
}

impl Oidc {
    /// IdP của tenant chính, qua URL công khai.
    pub fn new(issuer: &str) -> Oidc {
        Oidc::with_transport(issuer, Transport::Public)
    }

    /// IdP của tenant sổ, qua API nội bộ và listener loopback.
    pub fn internal(issuer: &str, route: InternalRoute) -> Oidc {
        Oidc::with_transport(issuer, Transport::Internal(route))
    }

    fn with_transport(issuer: &str, transport: Transport) -> Oidc {
        Oidc {
            issuer: issuer.trim_end_matches('/').to_string(),
            transport,
            discovery: OnceCell::new(),
            keys: RwLock::default(),
        }
    }

    /// Metadata của IdP (tải một lần; lỗi thì lần sau thử lại). Qua API nội bộ thì `issuer` phải đúng issuer này.
    pub async fn discovery(&self, http: &reqwest::Client) -> Result<&Discovery, AppError> {
        self.discovery
            .get_or_try_init(|| async {
                let resp = self
                    .transport
                    .discovery(http, &self.issuer)
                    .send()
                    .await
                    .map_err(|e| AppError::Unavailable(format!("không kết nối được máy chủ đăng nhập: {}", e.without_url())))?;
                let discovery = resp
                    .error_for_status()
                    .map_err(|e| AppError::Unavailable(format!("máy chủ đăng nhập lỗi: {e}")))?
                    .json::<Discovery>()
                    .await
                    .map_err(|e| AppError::Unavailable(format!("đọc metadata máy chủ đăng nhập: {e}")))?;
                if self.transport.checks_issuer()
                    && discovery.issuer.as_deref().map(|i| i.trim_end_matches('/')) != Some(self.issuer.as_str())
                {
                    return Err(AppError::Unavailable("metadata máy chủ đăng nhập là của issuer khác".into()));
                }
                Ok(discovery)
            })
            .await
    }

    async fn key_for(&self, http: &reqwest::Client, kid: &str) -> Result<DecodingKey, AppError> {
        {
            let cache = self.keys.read().await;
            let fresh = cache.fetched.is_some_and(|t| t.elapsed() < JWKS_TTL);
            if let Some(jwk) = cache.set.as_ref().filter(|_| fresh).and_then(|s| s.find(kid)) {
                return DecodingKey::from_jwk(jwk).map_err(|e| AppError::Internal(format!("dựng khoá: {e}")));
            }
            if cache.fetched.is_some_and(|t| t.elapsed() < REFETCH_COOLDOWN) {
                return Err(AppError::Unauthorized("không tìm thấy khoá ký".into()));
            }
        }
        let jwks_uri = self.discovery(http).await?.jwks_uri.clone();
        let mut cache = self.keys.write().await;
        // Request khác vừa tải xong trong lúc chờ khoá ghi: dùng luôn.
        if cache.fetched.is_some_and(|t| t.elapsed() < REFETCH_COOLDOWN) {
            if let Some(jwk) = cache.set.as_ref().and_then(|s| s.find(kid)) {
                return DecodingKey::from_jwk(jwk).map_err(|e| AppError::Internal(format!("dựng khoá: {e}")));
            }
            return Err(AppError::Unauthorized("không tìm thấy khoá ký".into()));
        }
        let fetched: Result<JwkSet, String> = async {
            self.transport
                .jwks(http, &jwks_uri)
                .send()
                .await
                .map_err(|e| format!("tải JWKS: {e}"))?
                .json()
                .await
                .map_err(|e| format!("đọc JWKS: {e}"))
        }
        .await;
        // Tải lỗi cũng tính là một lần tải (không gọi lại IdP mỗi request khi IdP sập) và dùng tạm bộ
        // khoá cũ nếu còn khoá `kid` này.
        cache.fetched = Some(Instant::now());
        match fetched {
            Ok(set) => cache.set = Some(set),
            Err(e) => tracing::warn!("JWKS: {e} — dùng bộ khoá đã có"),
        }
        let jwk = cache
            .set
            .as_ref()
            .and_then(|s| s.find(kid))
            .ok_or_else(|| AppError::Unauthorized("không tìm thấy khoá ký".into()))?;
        DecodingKey::from_jwk(jwk).map_err(|e| AppError::Internal(format!("dựng khoá: {e}")))
    }

    /// ID token hợp lệ cho `client_id`, đúng `nonce` của lượt đăng nhập (`None`: token từ lần làm mới,
    /// không mang nonce).
    pub async fn verify_id_token(
        &self,
        http: &reqwest::Client,
        token: &str,
        client_id: &str,
        nonce: Option<&str>,
    ) -> Result<IdClaims, AppError> {
        let header = decode_header(token).map_err(|_| AppError::Unauthorized("ID token không hợp lệ".into()))?;
        if !matches!(header.alg, Algorithm::ES256 | Algorithm::RS256) {
            return Err(AppError::Unauthorized("ID token ký sai thuật toán".into()));
        }
        let kid = header.kid.ok_or_else(|| AppError::Unauthorized("ID token thiếu kid".into()))?;
        let key = self.key_for(http, &kid).await?;
        let mut validation = Validation::new(header.alg);
        validation.set_audience(&[client_id]);
        validation.set_issuer(&[&self.issuer]);
        validation.leeway = 60;
        let claims = decode::<IdClaims>(token, &key, &validation)
            .map_err(|_| AppError::Unauthorized("ID token không hợp lệ hoặc hết hạn".into()))?
            .claims;
        if let Some(nonce) = nonce {
            if claims.nonce.as_deref() != Some(nonce) {
                return Err(AppError::Unauthorized("ID token sai nonce".into()));
            }
        }
        // Nhiều audience thì `azp` phải là chính client này (OIDC Core 3.1.3.7).
        let many = claims.aud.as_array().is_some_and(|a| a.len() > 1);
        if many && claims.azp.as_deref() != Some(client_id) {
            return Err(AppError::Unauthorized("ID token không dành cho client này".into()));
        }
        Ok(claims)
    }

    /// Logout token của back-channel logout (OpenID Back-Channel Logout 1.0 + hợp đồng của IdP).
    pub async fn verify_logout_token(
        &self,
        http: &reqwest::Client,
        token: &str,
        client_id: &str,
    ) -> Result<LogoutClaims, AppError> {
        let bad = || AppError::BadRequest("logout token không hợp lệ".into());
        let header = decode_header(token).map_err(|_| bad())?;
        if header.alg != Algorithm::ES256 || header.typ.as_deref() != Some("logout+jwt") {
            return Err(bad());
        }
        let kid = header.kid.ok_or_else(bad)?;
        let key = self.key_for(http, &kid).await.map_err(|_| bad())?;
        let mut validation = Validation::new(Algorithm::ES256);
        validation.set_audience(&[client_id]);
        validation.set_issuer(&[&self.issuer]);
        validation.validate_exp = false;
        validation.required_spec_claims = HashSet::from(["iat".to_string(), "aud".to_string(), "iss".to_string()]);
        let claims = decode::<LogoutClaims>(token, &key, &validation).map_err(|_| bad())?.claims;
        let age = chrono::Utc::now().timestamp() - claims.iat;
        let has_event = claims.events.as_ref().is_some_and(|e| e.contains_key(BACKCHANNEL_EVENT));
        if !(-60..=LOGOUT_TOKEN_MAX_AGE_S).contains(&age) || !has_event || claims.nonce.is_some() {
            return Err(bad());
        }
        if claims.sid.is_none() && claims.sub.is_none() {
            return Err(bad());
        }
        Ok(claims)
    }

    async fn token_request(
        &self,
        http: &reqwest::Client,
        form: &[(&str, &str)],
        basic: Option<(&str, &str)>,
    ) -> Result<TokenSet, GrantError> {
        let endpoint = self
            .discovery(http)
            .await
            .map_err(|e| GrantError::Unavailable(format!("{e:?}")))?
            .token_endpoint
            .clone();
        let mut req = self
            .transport
            .endpoint(http, reqwest::Method::POST, &endpoint)
            .map_err(GrantError::Unavailable)?
            .form(form)
            .timeout(Duration::from_secs(15));
        if let Some((id, secret)) = basic {
            req = req.basic_auth(id, Some(secret));
        }
        let resp = req.send().await.map_err(|e| GrantError::Unavailable(format!("gọi token endpoint: {e}")))?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if status.is_success() {
            let tokens: TokenResponse =
                serde_json::from_str(&body).map_err(|_| GrantError::Unavailable("đọc token không được".into()))?;
            let id_token = tokens.id_token.ok_or_else(|| GrantError::Rejected("IdP không trả ID token".into()))?;
            return Ok(TokenSet { id_token, access_token: tokens.access_token, refresh_token: tokens.refresh_token });
        }
        let error = serde_json::from_str::<TokenError>(&body).map(|e| e.error).unwrap_or_default();
        if error == "invalid_grant" {
            Err(GrantError::Rejected(error))
        } else {
            Err(GrantError::Unavailable(format!("HTTP {status} {error}")))
        }
    }

    /// Danh tính của một lần đăng nhập/làm mới: xác minh ID token (`nonce` khi đăng nhập), rồi lấy email,
    /// trạng thái xác minh và tên từ `userinfo` bằng access token đi kèm. `sub` của hai nguồn phải khớp.
    pub async fn identify(
        &self,
        http: &reqwest::Client,
        tokens: &TokenSet,
        client_id: &str,
        nonce: Option<&str>,
    ) -> Result<IdClaims, AppError> {
        let mut claims = self.verify_id_token(http, &tokens.id_token, client_id, nonce).await?;
        let endpoint = self
            .discovery(http)
            .await?
            .userinfo_endpoint
            .clone()
            .ok_or_else(|| AppError::Unavailable("IdP không có userinfo endpoint".into()))?;
        let resp = self
            .transport
            .endpoint(http, reqwest::Method::GET, &endpoint)
            .map_err(AppError::Unavailable)?
            .bearer_auth(&tokens.access_token)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| AppError::Unavailable(format!("gọi userinfo: {}", e.without_url())))?;
        if !resp.status().is_success() {
            return Err(AppError::Unavailable(format!("userinfo trả {}", resp.status())));
        }
        let info: UserInfo = resp.json().await.map_err(|_| AppError::Unavailable("đọc userinfo không được".into()))?;
        if info.sub != claims.sub {
            return Err(AppError::Unauthorized("userinfo không khớp ID token".into()));
        }
        claims.email = info.email;
        claims.email_verified = info.email_verified;
        claims.name = info.name.or(claims.name);
        Ok(claims)
    }

    /// Đổi authorization code (PKCE). `secret` có khi là client confidential.
    pub async fn exchange_code(
        &self,
        http: &reqwest::Client,
        client_id: &str,
        secret: Option<&str>,
        code: &str,
        verifier: &str,
        redirect_uri: &str,
    ) -> Result<TokenSet, GrantError> {
        let mut form = vec![
            ("grant_type", "authorization_code"),
            ("code", code),
            ("code_verifier", verifier),
            ("redirect_uri", redirect_uri),
        ];
        if secret.is_none() {
            form.push(("client_id", client_id));
        }
        self.token_request(http, &form, secret.map(|s| (client_id, s))).await
    }

    /// Thu hồi refresh token của client native (đăng xuất trên app: grant không còn treo ở `/account`).
    /// Trả `true` khi IdP nhận (2xx, RFC 7009 trả 200 cả với token đã hết hiệu lực); lỗi chỉ log — người gọi
    /// quyết định có thử lại không (xoá tài khoản thử lại ở vòng retention).
    pub async fn revoke(&self, http: &reqwest::Client, client_id: &str, refresh_token: &str) -> bool {
        let Ok(discovery) = self.discovery(http).await else { return false };
        // IdP không có endpoint thu hồi: không có gì để thử lại.
        let Some(endpoint) = discovery.revocation_endpoint.clone() else { return true };
        let form = [("token", refresh_token), ("token_type_hint", "refresh_token"), ("client_id", client_id)];
        let req = match self.transport.endpoint(http, reqwest::Method::POST, &endpoint) {
            Ok(req) => req,
            Err(e) => {
                tracing::warn!("thu hồi refresh token: {e}");
                return false;
            }
        };
        match req.form(&form).timeout(Duration::from_secs(10)).send().await {
            Ok(res) if res.status().is_success() => true,
            Ok(res) => {
                tracing::warn!(status = %res.status(), "IdP từ chối thu hồi refresh token");
                false
            }
            Err(e) => {
                tracing::warn!("thu hồi refresh token ở IdP lỗi: {e}");
                false
            }
        }
    }

    /// Làm mới refresh token của client native (dùng một lần, xoay vòng).
    pub async fn refresh(&self, http: &reqwest::Client, client_id: &str, refresh_token: &str) -> Result<TokenSet, GrantError> {
        let form = [("grant_type", "refresh_token"), ("refresh_token", refresh_token), ("client_id", client_id)];
        self.token_request(http, &form, None).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::get;
    use axum::{Json, Router};
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use ring::rand::SystemRandom;
    use ring::signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_FIXED_SIGNING};
    use serde_json::json;

    /// IdP giả: discovery + JWKS với một khoá P-256 sinh lúc chạy test.
    struct FakeIdp {
        issuer: String,
        key: EncodingKey,
    }

    async fn fake_idp() -> FakeIdp {
        let rng = SystemRandom::new();
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng).expect("sinh khoá");
        let pair = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, pkcs8.as_ref(), &rng).expect("khoá");
        let public = pair.public_key().as_ref();
        let (x, y) = (URL_SAFE_NO_PAD.encode(&public[1..33]), URL_SAFE_NO_PAD.encode(&public[33..65]));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let issuer = format!("http://{}", listener.local_addr().expect("addr"));
        let meta = json!({
            "issuer": issuer,
            "authorization_endpoint": format!("{issuer}/auth"),
            "token_endpoint": format!("{issuer}/token"),
            "jwks_uri": format!("{issuer}/jwks"),
            "end_session_endpoint": format!("{issuer}/session/end"),
            "userinfo_endpoint": format!("{issuer}/me"),
        });
        // userinfo giả: token "at-1" là của acc_1, token khác là của acc_2.
        let me = |headers: axum::http::HeaderMap| async move {
            let own = headers.get("authorization").and_then(|v| v.to_str().ok()) == Some("Bearer at-1");
            let sub = if own { "acc_1" } else { "acc_2" };
            Json(json!({ "sub": sub, "email": "a@b.vn", "email_verified": true, "name": "A" }))
        };
        let jwks = json!({ "keys": [{ "kty": "EC", "crv": "P-256", "kid": "k1", "alg": "ES256", "use": "sig", "x": x, "y": y }] });
        let app = Router::new()
            .route("/.well-known/openid-configuration", get(move || async move { Json(meta) }))
            .route("/jwks", get(move || async move { Json(jwks) }))
            .route("/me", get(me));
        tokio::spawn(async move { axum::serve(listener, app).await });
        FakeIdp { issuer, key: EncodingKey::from_ec_der(pkcs8.as_ref()) }
    }

    fn sign(idp: &FakeIdp, typ: Option<&str>, kid: &str, claims: serde_json::Value) -> String {
        let mut header = Header::new(Algorithm::ES256);
        header.kid = Some(kid.into());
        header.typ = typ.map(str::to_string);
        encode(&header, &claims, &idp.key).expect("ký")
    }

    fn now() -> i64 {
        chrono::Utc::now().timestamp()
    }

    #[tokio::test]
    async fn id_tokens_are_checked_for_issuer_audience_nonce_and_expiry() {
        let idp = fake_idp().await;
        let oidc = Oidc::new(&idp.issuer);
        let http = reqwest::Client::new();
        let claims = |aud: &str, nonce: &str, exp: i64| {
            json!({ "iss": idp.issuer, "aud": aud, "sub": "acc_1", "nonce": nonce, "exp": exp, "iat": now(),
                    "email": "a@b.vn", "email_verified": true, "sid": "s1" })
        };
        let good = sign(&idp, None, "k1", claims("scribe-web", "n1", now() + 300));
        let ok = oidc.verify_id_token(&http, &good, "scribe-web", Some("n1")).await.expect("hợp lệ");
        assert_eq!((ok.sub.as_str(), ok.sid.as_deref(), ok.email_verified), ("acc_1", Some("s1"), true));

        assert!(oidc.verify_id_token(&http, &good, "scribe-web", Some("khác")).await.is_err(), "sai nonce");
        assert!(oidc.verify_id_token(&http, &good, "scribe-mobile", Some("n1")).await.is_err(), "sai aud");
        let expired = sign(&idp, None, "k1", claims("scribe-web", "n1", now() - 3600));
        assert!(oidc.verify_id_token(&http, &expired, "scribe-web", Some("n1")).await.is_err(), "hết hạn");
        let unknown_kid = sign(&idp, None, "k9", claims("scribe-web", "n1", now() + 300));
        assert!(oidc.verify_id_token(&http, &unknown_kid, "scribe-web", Some("n1")).await.is_err(), "kid lạ");
        // `alg: none` không bao giờ được nhận.
        let unsigned = format!(
            "{}.{}.",
            URL_SAFE_NO_PAD.encode(br#"{"alg":"none","kid":"k1"}"#),
            URL_SAFE_NO_PAD.encode(claims("scribe-web", "n1", now() + 300).to_string())
        );
        assert!(oidc.verify_id_token(&http, &unsigned, "scribe-web", Some("n1")).await.is_err(), "alg none");
        // Nhiều audience mà `azp` khác client: từ chối.
        let mut multi = claims("scribe-web", "n1", now() + 300);
        multi["aud"] = json!(["scribe-web", "other"]);
        multi["azp"] = json!("other");
        let multi = sign(&idp, None, "k1", multi);
        assert!(oidc.verify_id_token(&http, &multi, "scribe-web", Some("n1")).await.is_err(), "azp");
    }

    #[tokio::test]
    async fn identity_takes_email_from_userinfo_of_the_same_account() {
        let idp = fake_idp().await;
        let oidc = Oidc::new(&idp.issuer);
        let http = reqwest::Client::new();
        // Như hoandat-auth với code flow: ID token không có email.
        let id_token = sign(&idp, None, "k1", json!({ "iss": idp.issuer, "aud": "scribe-web", "sub": "acc_1", "nonce": "n1",
                                                    "exp": now() + 300, "iat": now() }));
        let tokens = |at: &str| TokenSet { id_token: id_token.clone(), access_token: at.into(), refresh_token: None };
        let who = oidc.identify(&http, &tokens("at-1"), "scribe-web", Some("n1")).await.expect("hợp lệ");
        assert_eq!((who.sub.as_str(), who.email.as_deref(), who.email_verified, who.name.as_deref()), ("acc_1", Some("a@b.vn"), true, Some("A")));
        // Access token của tài khoản khác: từ chối.
        assert!(oidc.identify(&http, &tokens("at-2"), "scribe-web", Some("n1")).await.is_err());
    }

    #[tokio::test]
    async fn logout_tokens_follow_the_backchannel_contract() {
        let idp = fake_idp().await;
        let oidc = Oidc::new(&idp.issuer);
        let http = reqwest::Client::new();
        let event = json!({ BACKCHANNEL_EVENT: {} });
        let token = |iat: i64, extra: serde_json::Value| {
            let mut c = json!({ "iss": idp.issuer, "aud": "scribe-web", "iat": iat, "jti": "j1", "sid": "s1", "events": event });
            if let (Some(c), Some(extra)) = (c.as_object_mut(), extra.as_object()) {
                c.extend(extra.clone());
            }
            c
        };
        let good = sign(&idp, Some("logout+jwt"), "k1", token(now(), json!({})));
        assert_eq!(oidc.verify_logout_token(&http, &good, "scribe-web").await.expect("hợp lệ").sid.as_deref(), Some("s1"));

        let wrong_typ = sign(&idp, Some("JWT"), "k1", token(now(), json!({})));
        assert!(oidc.verify_logout_token(&http, &wrong_typ, "scribe-web").await.is_err(), "sai typ");
        let old = sign(&idp, Some("logout+jwt"), "k1", token(now() - 600, json!({})));
        assert!(oidc.verify_logout_token(&http, &old, "scribe-web").await.is_err(), "quá 2 phút");
        let with_nonce = sign(&idp, Some("logout+jwt"), "k1", token(now(), json!({ "nonce": "x" })));
        assert!(oidc.verify_logout_token(&http, &with_nonce, "scribe-web").await.is_err(), "có nonce");
        let no_event = sign(&idp, Some("logout+jwt"), "k1", token(now(), json!({ "events": {} })));
        assert!(oidc.verify_logout_token(&http, &no_event, "scribe-web").await.is_err(), "thiếu event");
        assert!(oidc.verify_logout_token(&http, &good, "scribe-mobile").await.is_err(), "sai aud");
    }

    #[test]
    fn issuer_has_no_trailing_slash() {
        assert_eq!(Oidc::new("https://auth.hoandat.com/").issuer, "https://auth.hoandat.com");
    }
}
