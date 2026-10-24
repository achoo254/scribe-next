//! Test router đầy đủ (`routes::build_router`) theo Host: tenant lạ, tenant chưa bật, phiên chéo tenant.

use super::directory::tests::{directory, fake_lookup, tenant_body};
use super::Readiness;
use crate::models::session::{self as sessions, Client};
use crate::state::AppState;
use serde_json::{json, Value};
use std::collections::HashMap;

/// Phục vụ router trên cổng ngẫu nhiên; trả origin.
async fn serve(state: AppState) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let origin = format!("http://{}", listener.local_addr().expect("addr"));
    let app = crate::routes::build_router(state);
    tokio::spawn(async move { axum::serve(listener, app).await });
    origin
}

fn client() -> reqwest::Client {
    reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).build().expect("client")
}

async fn get(origin: &str, host: &str, path: &str, cookie: Option<&str>) -> (u16, Option<String>, Value) {
    let mut req = client().get(format!("{origin}{path}")).header("host", host);
    if let Some(c) = cookie {
        req = req.header("cookie", c);
    }
    let res = req.send().await.expect("gửi");
    let status = res.status().as_u16();
    let location = res.headers().get("location").and_then(|v| v.to_str().ok()).map(str::to_string);
    let body = res.json::<Value>().await.unwrap_or(Value::Null);
    (status, location, body)
}

async fn lazy_db() -> mongodb::Database {
    // Các route dưới đây trả lời trước khi đụng Mongo; `with_uri_str` không nối ngay.
    crate::db::connect("mongodb://127.0.0.1:1", "unused").await.expect("client")
}

#[tokio::test]
async fn hosts_route_to_their_tenant_and_disabled_tenants_stop_before_any_session() {
    let tenants = HashMap::from([("scribe.inetdev.io.vn".to_string(), tenant_body("ten_a", "inetdev.io.vn", true))]);
    let (api, _) = fake_lookup(tenants).await;
    // Scribe chưa sẵn sàng cho tenant sổ (chưa có vòng khoá niêm phong): mọi tenant sổ là "chưa bật".
    let state = AppState::for_tests(lazy_db().await, directory(Some(&api), Readiness { oidc: true, sealed_keys: false }));
    let origin = serve(state).await;

    let (status, _, body) = get(&origin, "evil.example.com", "/api/idp", None).await;
    assert_eq!((status, body["code"].clone()), (404, json!("UNKNOWN_TENANT")));
    let (status, _, body) = get(&origin, "scribe.unknown.vn", "/api/me", None).await;
    assert_eq!((status, body["code"].clone()), (404, json!("UNKNOWN_TENANT")));

    // Tenant sổ chưa bật: /api/idp vẫn trả để web hiện trang "chưa bật".
    let (status, _, body) = get(&origin, "scribe.inetdev.io.vn", "/api/idp", None).await;
    assert_eq!(status, 200);
    assert_eq!(
        body,
        json!({ "displayName": "inetdev.io.vn login", "domain": "inetdev.io.vn", "issuer": "https://auth.inetdev.io.vn",
                "primary": false, "scribe": "disabled" })
    );
    // Route cần đăng nhập: 403 trước khi đọc phiên (Mongo không được chạm).
    let (status, _, body) = get(&origin, "scribe.inetdev.io.vn", "/api/me", Some("__Host-scribe=x")).await;
    assert_eq!((status, body["code"].clone()), (403, json!("TENANT_DISABLED")));
    // App di động chỉ ở tenant chính.
    let (status, _, _) = get(&origin, "scribe.inetdev.io.vn", "/api/auth/native/config", None).await;
    assert_eq!(status, 404);
    // Đăng nhập trên trình duyệt: về trang đăng nhập của chính tenant đó.
    let (status, location, _) = get(&origin, "scribe.inetdev.io.vn", "/api/auth/login", None).await;
    assert_eq!((status, location.as_deref()), (303, Some("https://scribe.inetdev.io.vn/login?error=not_enabled")));

    // Tenant chính không đổi: /api/idp như trước (kèm các trường mới), /api/health không qua tenant.
    let (status, _, body) = get(&origin, "scribe.hoandat.com", "/api/idp", None).await;
    assert_eq!((status, body["displayName"].clone(), body["scribe"].clone()), (200, json!("Auth Next"), json!("enabled")));
    let (status, _, body) = get(&origin, "anything.example", "/api/health", None).await;
    assert_eq!((status, body), (200, json!({ "status": "ok" })));
    // Chưa đăng nhập ở tenant chính: 401 như trước.
    let (status, _, _) = get(&origin, "scribe.hoandat.com", "/api/me", None).await;
    assert_eq!(status, 401);
}

/// Phiên của tenant A gửi tới host của tenant B (đã bật) ⟹ 401 và phiên A còn nguyên; ở host A thì dùng được.
/// `cargo test cross_tenant_session -- --ignored`.
#[tokio::test]
#[ignore]
async fn cross_tenant_session_is_rejected_on_real_mongo() {
    let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
    let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
    let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
    let tag = uuid::Uuid::new_v4().simple().to_string();
    let (ten_a, ten_b) = (format!("ten_a_{tag}"), format!("ten_b_{tag}"));
    let (host_a, host_b) = (format!("scribe.a-{tag}.vn"), format!("scribe.b-{tag}.vn"));
    let tenants = HashMap::from([
        (host_a.clone(), tenant_body(&ten_a, &format!("a-{tag}.vn"), true)),
        (host_b.clone(), tenant_body(&ten_b, &format!("b-{tag}.vn"), true)),
    ]);
    let (api, _) = fake_lookup(tenants).await;
    let state = AppState::for_tests(db.clone(), directory(Some(&api), Readiness { oidc: true, sealed_keys: true }));
    let (user_a, _) = crate::models::user::find_or_link(&db, &ten_a, "acc_same", &format!("a-{tag}@x.vn"), None).await.expect("user A");
    let (token, _) = sessions::create(&db, &ten_a, &user_a, Client::Web, None, None).await.expect("phiên A");
    let origin = serve(state).await;
    let cookie = format!("__Host-scribe={token}");

    let (status, _, _) = get(&origin, &host_b, "/api/me/usage", Some(&cookie)).await;
    assert_eq!(status, 401, "phiên A ở host B");
    let (status, _, body) = get(&origin, &host_a, "/api/me/usage", Some(&cookie)).await;
    assert_eq!(status, 200, "phiên A ở host A: {body}");
    // Đã vào cache ở host A: host B vẫn bị từ chối, và host chính cũng vậy.
    assert_eq!(get(&origin, &host_b, "/api/me/usage", Some(&cookie)).await.0, 401);
    assert_eq!(get(&origin, "scribe.hoandat.com", "/api/me/usage", Some(&cookie)).await.0, 401);
    assert!(sessions::find(&db, &ten_a, &token).await.expect("tìm").is_some(), "phiên A không bị xoá");

    crate::account::purge_user(&db, &std::env::temp_dir().to_string_lossy(), &user_a).await.expect("dọn");
}

/// Hai tenant sổ đã bật (đủ điều kiện phía Scribe) với IdP giả: tenant A `ten_a` ở `a.vn`, tenant B `ten_b` ở `b.vn`.
async fn two_enabled_tenants(db: mongodb::Database) -> (String, crate::oidc_registry::tests::FakeTenantIdp) {
    use crate::oidc_registry::tests::{fake_tenant_idp, registry};
    let tenants = HashMap::from([
        ("scribe.a.vn".to_string(), tenant_body("ten_a", "a.vn", true)),
        ("scribe.b.vn".to_string(), tenant_body("ten_b", "b.vn", true)),
    ]);
    let (lookup, _) = fake_lookup(tenants).await;
    let idp = fake_tenant_idp(&[("ten_a", "https://auth.a.vn"), ("ten_b", "https://auth.b.vn")]).await;
    let dir = directory(Some(&lookup), Readiness { oidc: true, sealed_keys: true });
    let state = AppState::for_tests_with(crate::config::Config::for_tests(), db, dir, registry(&idp));
    (serve(state).await, idp)
}

fn logout_token(idp: &crate::oidc_registry::tests::FakeTenantIdp, issuer: &str, jti: &str) -> String {
    let event = json!({ "http://schemas.openid.net/event/backchannel-logout": {} });
    let claims = json!({ "iss": issuer, "aud": "scribe-web", "iat": chrono::Utc::now().timestamp(), "jti": jti,
                         "sid": "sid-1", "events": event });
    crate::oidc_registry::tests::sign(&idp.key, Some("logout+jwt"), claims)
}

async fn backchannel(origin: &str, host: &str, token: &str) -> u16 {
    let res = client()
        .post(format!("{origin}/api/auth/backchannel-logout"))
        .header("host", host)
        .form(&[("logout_token", token)])
        .send()
        .await
        .expect("gửi");
    res.status().as_u16()
}

#[tokio::test]
async fn a_tenant_only_trusts_its_own_idp_and_has_no_mobile_sign_in() {
    let (origin, idp) = two_enabled_tenants(lazy_db().await).await;
    // Logout token do IdP ký cho tenant A gửi tới host B: bị từ chối trước khi đụng phiên nào.
    assert_eq!(backchannel(&origin, "scribe.b.vn", &logout_token(&idp, "https://auth.a.vn", "j-a")).await, 400);
    // App di động: chỉ tenant chính.
    let res = client().post(format!("{origin}/api/auth/native/exchange")).header("host", "scribe.a.vn").json(&json!({})).send().await.expect("gửi");
    assert_eq!(res.status().as_u16(), 404);
    assert_eq!(get(&origin, "scribe.a.vn", "/api/auth/native/config", None).await.0, 404);
    // `/api/idp` của tenant đã bật.
    let (_, _, body) = get(&origin, "scribe.a.vn", "/api/idp", None).await;
    assert_eq!((body["scribe"].clone(), body["issuer"].clone()), (json!("enabled"), json!("https://auth.a.vn")));
}

/// Đăng nhập web trọn vòng ở tenant A qua IdP của A (API nội bộ + loopback giả): phiên dùng được ở A, không ở B hay
/// tenant chính; back-channel của A gửi tới B không xoá gì, gửi tới A thì xoá. `cargo test web_sign_in_on_a_tenant
/// -- --ignored`.
#[tokio::test]
#[ignore]
async fn web_sign_in_on_a_tenant_on_real_mongo() {
    let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
    let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
    let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
    crate::models::user::ensure_indexes(&db).await.expect("index");
    let (origin, idp) = two_enabled_tenants(db.clone()).await;

    let res = client().get(format!("{origin}/api/auth/login")).header("host", "scribe.a.vn").send().await.expect("login");
    assert_eq!(res.status().as_u16(), 303);
    let location = reqwest::Url::parse(res.headers()["location"].to_str().expect("location")).expect("url");
    assert_eq!(location.origin().ascii_serialization(), "https://auth.a.vn", "trang đăng nhập của IdP tenant A");
    let query: HashMap<String, String> = location.query_pairs().into_owned().collect();
    assert_eq!(query["client_id"], "scribe-web");
    assert_eq!(query["redirect_uri"], "https://scribe.a.vn/api/auth/callback");
    let login_cookie = res.headers()["set-cookie"].to_str().expect("cookie").split(';').next().expect("cặp").to_string();
    assert!(login_cookie.starts_with("__Host-scribe_login="));
    *idp.nonce.lock().expect("khoá") = query["nonce"].clone();

    let callback = format!("{origin}/api/auth/callback?code=c1&state={}", query["state"]);
    let res = client().get(callback).header("host", "scribe.a.vn").header("cookie", &login_cookie).send().await.expect("callback");
    assert_eq!(res.status().as_u16(), 303);
    assert_eq!(res.headers()["location"], "https://scribe.a.vn/");
    let session = res
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|c| c.starts_with("__Host-scribe="))
        .and_then(|c| c.split(';').next())
        .expect("cookie phiên")
        .to_string();

    assert_eq!(get(&origin, "scribe.a.vn", "/api/me/usage", Some(&session)).await.0, 200);
    assert_eq!(get(&origin, "scribe.b.vn", "/api/me/usage", Some(&session)).await.0, 401);
    assert_eq!(get(&origin, "scribe.hoandat.com", "/api/me/usage", Some(&session)).await.0, 401);
    let user = crate::models::user::find_id_by_sub(&db, "ten_a", "acc_1").await.expect("tìm").expect("user của tenant A");
    assert!(crate::models::user::find_id_by_sub(&db, "ten_b", "acc_1").await.expect("tìm").is_none());

    let jti = format!("j-{}", uuid::Uuid::new_v4());
    assert_eq!(backchannel(&origin, "scribe.b.vn", &logout_token(&idp, "https://auth.a.vn", &jti)).await, 400);
    assert_eq!(get(&origin, "scribe.a.vn", "/api/me/usage", Some(&session)).await.0, 200, "B không xoá được phiên của A");
    assert_eq!(backchannel(&origin, "scribe.a.vn", &logout_token(&idp, "https://auth.a.vn", &jti)).await, 200);
    assert_eq!(get(&origin, "scribe.a.vn", "/api/me/usage", Some(&session)).await.0, 401, "đã đăng xuất ở IdP của A");

    crate::account::purge_user(&db, &std::env::temp_dir().to_string_lossy(), &user).await.expect("dọn");
}
