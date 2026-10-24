//! IdP giả của tenant sổ: API nội bộ (discovery + JWKS theo id tenant, Basic `scribe`) và listener loopback (token,
//! userinfo) ghi lại `Host`, `X-Forwarded-Proto` và Basic của mỗi lượt gọi.

use super::*;
use crate::tenant::{test_tenant, PRIMARY_ID};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::engine::general_purpose::STANDARD;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use ring::rand::SystemRandom;
use ring::signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_FIXED_SIGNING};
use serde_json::{json, Value};

pub(crate) const API_SECRET: &str = "s3cret-s3cret-s3cret-s3cret-s3cret";

/// Vector của auth-next (`src/__tests__/scribe-tenant-clients.test.ts`): khoá app `scribe-web` (đã suy từ khoá chủ của
/// IdP), id tenant và secret client mong đợi.
pub(crate) const VECTOR_APP_KEY: &str = "VbUBiAD6L3K8pOUba5CoidyP18e3ns-lA2HaVmFcsoU";
pub(crate) const VECTOR_TENANT: &str = "ten_vectorTenant0001";
pub(crate) const VECTOR_SECRET: &str = "agpXVXYsBzukgRNoTqoTyMlL1VvG__WSui_Pl-Y6wTc";

/// Một lượt gọi tới loopback: `(path, Host, X-Forwarded-Proto, Authorization)`.
pub(crate) type Seen = (String, String, String, String);

#[derive(Clone)]
struct Fake {
    /// id tenant → issuer mà discovery nội bộ trả về.
    issuers: HashMap<String, String>,
    jwks: Value,
    key: Arc<EncodingKey>,
    seen: Arc<Mutex<Vec<Seen>>>,
    nonce: Arc<Mutex<String>>,
}

pub(crate) struct FakeTenantIdp {
    pub api: String,
    pub loopback: String,
    pub key: Arc<EncodingKey>,
    pub seen: Arc<Mutex<Vec<Seen>>>,
    /// `nonce` đặt vào ID token mà endpoint token trả (mặc định `n1`).
    pub nonce: Arc<Mutex<String>>,
}

fn header(headers: &HeaderMap, name: &str) -> String {
    headers.get(name).and_then(|v| v.to_str().ok()).unwrap_or_default().to_string()
}

pub(crate) fn sign(key: &EncodingKey, typ: Option<&str>, claims: Value) -> String {
    let mut h = Header::new(Algorithm::ES256);
    h.kid = Some("k1".into());
    h.typ = typ.map(str::to_string);
    encode(&h, &claims, key).expect("ký")
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// IdP giả phục vụ các tenant `issuers` (id → issuer trả trong discovery nội bộ), một khoá ký chung.
pub(crate) async fn fake_tenant_idp(issuers: &[(&str, &str)]) -> FakeTenantIdp {
    let rng = SystemRandom::new();
    let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng).expect("sinh khoá");
    let pair = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, pkcs8.as_ref(), &rng).expect("khoá");
    let public = pair.public_key().as_ref();
    let (x, y) = (URL_SAFE_NO_PAD.encode(&public[1..33]), URL_SAFE_NO_PAD.encode(&public[33..65]));
    let fake = Fake {
        issuers: issuers.iter().map(|(id, iss)| (id.to_string(), iss.to_string())).collect(),
        jwks: json!({ "keys": [{ "kty": "EC", "crv": "P-256", "kid": "k1", "alg": "ES256", "use": "sig", "x": x, "y": y }] }),
        key: Arc::new(EncodingKey::from_ec_der(pkcs8.as_ref())),
        seen: Arc::default(),
        nonce: Arc::new(Mutex::new("n1".into())),
    };
    let basic_ok = |headers: &HeaderMap| header(headers, "authorization") == format!("Basic {}", STANDARD.encode(format!("scribe:{API_SECRET}")));
    let discovery = move |State(f): State<Fake>, Path(id): Path<String>, headers: HeaderMap| async move {
        let Some(issuer) = f.issuers.get(&id).filter(|_| basic_ok(&headers)) else {
            return StatusCode::NOT_FOUND.into_response();
        };
        Json(json!({
            "issuer": issuer,
            "authorization_endpoint": format!("{issuer}/auth"),
            "token_endpoint": format!("{issuer}/token"),
            "jwks_uri": format!("{issuer}/jwks"),
            "end_session_endpoint": format!("{issuer}/session/end"),
            "userinfo_endpoint": format!("{issuer}/me"),
        }))
        .into_response()
    };
    let jwks = move |State(f): State<Fake>, Path(id): Path<String>, headers: HeaderMap| async move {
        if !f.issuers.contains_key(&id) || !basic_ok(&headers) {
            return StatusCode::NOT_FOUND.into_response();
        }
        Json(f.jwks.clone()).into_response()
    };
    let api_app = Router::new()
        .route("/internal/tenants/{id}/openid-configuration", get(discovery))
        .route("/internal/tenants/{id}/jwks", get(jwks))
        .with_state(fake.clone());
    // Loopback: IdP chọn tenant theo Host; ID token mang issuer `https://<Host>`.
    let token = |State(f): State<Fake>, headers: HeaderMap| async move {
        let host = header(&headers, "host");
        let seen = ("/token".to_string(), host.clone(), header(&headers, "x-forwarded-proto"), header(&headers, "authorization"));
        f.seen.lock().expect("khoá").push(seen);
        let nonce = f.nonce.lock().expect("khoá").clone();
        let claims = json!({ "iss": format!("https://{host}"), "aud": "scribe-web", "sub": "acc_1", "nonce": nonce,
                             "exp": now() + 300, "iat": now(), "sid": "sid-1" });
        Json(json!({ "id_token": sign(&f.key, None, claims), "access_token": "at-1" }))
    };
    let me = |State(f): State<Fake>, headers: HeaderMap| async move {
        let seen = ("/me".to_string(), header(&headers, "host"), header(&headers, "x-forwarded-proto"), header(&headers, "authorization"));
        f.seen.lock().expect("khoá").push(seen);
        Json(json!({ "sub": "acc_1", "email": "a@a.vn", "email_verified": true, "name": "A" }))
    };
    let loopback_app = Router::new().route("/token", post(token)).route("/me", get(me)).with_state(fake.clone());
    let serve = |app: Router| async move {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let origin = format!("http://{}", listener.local_addr().expect("addr"));
        tokio::spawn(async move { axum::serve(listener, app).await });
        origin
    };
    FakeTenantIdp { api: serve(api_app).await, loopback: serve(loopback_app).await, key: fake.key, seen: fake.seen, nonce: fake.nonce }
}

/// Sổ đăng ký có chế độ tenant với IdP giả và khoá app của vector.
pub(crate) fn registry(idp: &FakeTenantIdp) -> OidcRegistry {
    let access = TenantAccess {
        api: Arc::new(InternalApi::new(&idp.api, API_SECRET).expect("client")),
        loopback: idp.loopback.clone(),
        client_key: Zeroizing::new(VECTOR_APP_KEY.into()),
    };
    OidcRegistry::new(Oidc::new("https://auth.hoandat.com"), "scribe-web-dev", "primary-secret".into(), Some(access))
}

pub(crate) fn tenant(id: &str, domain: &str) -> TenantCtx {
    TenantCtx {
        id: id.into(),
        domain: domain.into(),
        issuer: format!("https://auth.{domain}"),
        public_url: format!("https://scribe.{domain}"),
        display_name: domain.into(),
        primary: false,
        enabled: true,
    }
}

#[test]
fn tenant_secret_matches_the_idp_vector() {
    assert_eq!(tenant_client_secret(VECTOR_APP_KEY, VECTOR_TENANT), VECTOR_SECRET);
    assert_ne!(tenant_client_secret(VECTOR_APP_KEY, "ten_other"), VECTOR_SECRET);
}

#[tokio::test]
async fn tenant_sign_in_goes_through_the_internal_api_and_loopback_with_the_tenants_host() {
    let idp = fake_tenant_idp(&[(VECTOR_TENANT, "https://auth.a.vn")]).await;
    let reg = registry(&idp);
    let a = tenant(VECTOR_TENANT, "a.vn");
    let oidc = reg.get(&a).expect("client");
    let http = reqwest::Client::new();
    let discovery = oidc.discovery(&http).await.expect("discovery nội bộ");
    // Trình duyệt vẫn mở trang đăng nhập công khai của IdP tenant.
    assert_eq!(discovery.authorization_endpoint, "https://auth.a.vn/auth");
    assert_eq!(reg.web_client_id(&a), "scribe-web");
    let secret = reg.client_secret(&a).expect("secret");
    assert_eq!(secret.as_str(), VECTOR_SECRET);
    let tokens = oidc
        .exchange_code(&http, "scribe-web", Some(&secret), "code-1", "verifier", "https://scribe.a.vn/api/auth/callback")
        .await
        .map_err(|e| format!("{e:?}"))
        .expect("đổi code");
    let claims = oidc.identify(&http, &tokens, "scribe-web", Some("n1")).await.expect("danh tính");
    assert_eq!((claims.sub.as_str(), claims.email.as_deref()), ("acc_1", Some("a@a.vn")));
    let seen = idp.seen.lock().expect("khoá").clone();
    let basic = format!("Basic {}", STANDARD.encode(format!("scribe-web:{VECTOR_SECRET}")));
    assert_eq!(seen[0], ("/token".into(), "auth.a.vn".into(), "https".into(), basic));
    assert_eq!((seen[1].0.as_str(), seen[1].1.as_str(), seen[1].2.as_str()), ("/me", "auth.a.vn", "https"));
    // Cùng tenant: cùng instance (discovery và JWKS giữ trong cache).
    assert!(Arc::ptr_eq(&oidc, &reg.get(&a).expect("lại")));
}

#[tokio::test]
async fn another_issuer_is_never_trusted() {
    // `ten_b` ở IdP có issuer auth.b.vn; discovery nội bộ của `ten_x` trả issuer của tenant khác.
    let idp = fake_tenant_idp(&[("ten_a", "https://auth.a.vn"), ("ten_b", "https://auth.b.vn"), ("ten_x", "https://auth.b.vn")]).await;
    let reg = registry(&idp);
    let http = reqwest::Client::new();
    assert!(reg.get(&tenant("ten_x", "x.vn")).expect("client").discovery(&http).await.is_err(), "discovery của issuer khác");

    // ID token do IdP ký cho tenant A (cùng khoá ký) không qua được ở tenant B: sai issuer.
    let a_token = sign(&idp.key, None, json!({ "iss": "https://auth.a.vn", "aud": "scribe-web", "sub": "acc_1", "nonce": "n1",
                                                "exp": now() + 300, "iat": now() }));
    let a = reg.get(&tenant("ten_a", "a.vn")).expect("A");
    let b = reg.get(&tenant("ten_b", "b.vn")).expect("B");
    assert!(a.verify_id_token(&http, &a_token, "scribe-web", Some("n1")).await.is_ok());
    assert!(b.verify_id_token(&http, &a_token, "scribe-web", Some("n1")).await.is_err(), "token của A ở B");
    // Logout token của A không qua được ở B.
    let event = json!({ "http://schemas.openid.net/event/backchannel-logout": {} });
    let logout = sign(&idp.key, Some("logout+jwt"), json!({ "iss": "https://auth.a.vn", "aud": "scribe-web", "iat": now(),
                                                             "jti": "j1", "sid": "sid-1", "events": event }));
    assert!(a.verify_logout_token(&http, &logout, "scribe-web").await.is_ok());
    assert!(b.verify_logout_token(&http, &logout, "scribe-web").await.is_err(), "logout token của A ở B");

    // Issuer của tenant đổi: dựng lại client.
    let mut moved = tenant("ten_a", "a.vn");
    moved.issuer = "https://auth.a2.vn".into();
    assert!(!Arc::ptr_eq(&a, &reg.get(&moved).expect("A mới")));
}

#[tokio::test]
async fn the_primary_tenant_keeps_its_public_client_and_file_secret() {
    let idp = fake_tenant_idp(&[]).await;
    let reg = registry(&idp);
    let primary = test_tenant(PRIMARY_ID, true);
    assert!(Arc::ptr_eq(&reg.get(&primary).expect("chính"), reg.primary()));
    assert_eq!(reg.web_client_id(&primary), "scribe-web-dev");
    assert_eq!(reg.client_secret(&primary).expect("secret").as_str(), "primary-secret");
    // Không có chế độ tenant: tenant sổ không dựng được client.
    let off = OidcRegistry::new(Oidc::new("https://auth.hoandat.com"), "scribe-web", "s".into(), None);
    assert!(!off.tenant_mode() && off.get(&tenant("ten_a", "a.vn")).is_err() && off.client_secret(&tenant("ten_a", "a.vn")).is_err());
}

#[test]
fn tenant_access_needs_every_setting_and_a_well_formed_key() {
    let mut config = Config::for_tests();
    let api = Some(Arc::new(InternalApi::new("http://127.0.0.1:1", API_SECRET).expect("client")));
    assert!(TenantAccess::load(&config, api.clone()).expect("thiếu cấu hình").is_none());
    let dir = std::env::temp_dir().join(format!("scribe-app-key-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("thư mục");
    let path = dir.join("key");
    std::fs::write(&path, format!("{VECTOR_APP_KEY}\n")).expect("ghi");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).expect("600");
    }
    config.idp_loopback_url = Some("http://127.0.0.1:4500".into());
    config.scribe_web_client_key_path = Some(path.to_string_lossy().into_owned());
    assert!(TenantAccess::load(&config, None).expect("thiếu API nội bộ").is_none());
    let access = TenantAccess::load(&config, api.clone()).expect("đủ").expect("bật");
    assert_eq!(access.client_key.as_str(), VECTOR_APP_KEY);
    std::fs::write(&path, "short").expect("ghi");
    assert!(TenantAccess::load(&config, api).is_err(), "khoá hỏng");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn internal_route_refuses_calls_outside_the_issuer() {
    let api = Arc::new(InternalApi::new("http://127.0.0.1:1", API_SECRET).expect("client"));
    let route = InternalRoute::new("ten_a", "https://auth.a.vn", api.clone(), "http://127.0.0.1:4500").expect("route");
    let transport = crate::oidc_transport::Transport::Internal(route);
    let http = reqwest::Client::new();
    assert!(transport.endpoint(&http, reqwest::Method::POST, "https://evil.example/token").is_err());
    assert!(transport.endpoint(&http, reqwest::Method::POST, "http://auth.a.vn/token").is_err(), "khác scheme");
    let req = transport.endpoint(&http, reqwest::Method::POST, "https://auth.a.vn/token?x=1").expect("cùng issuer").build().expect("build");
    assert_eq!(req.url().as_str(), "http://127.0.0.1:4500/token?x=1");
    assert_eq!(req.headers().get("host").and_then(|v| v.to_str().ok()), Some("auth.a.vn"));
    assert!(InternalRoute::new("ten/../x", "https://auth.a.vn", api, "http://127.0.0.1:4500").is_err(), "id lạ");
}
