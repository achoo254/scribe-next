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
    let (status, _, body) = get(&origin, "scribe.inetdev.io.vn", "/api/auth/native/config", None).await;
    assert_eq!((status, body["code"].clone()), (403, json!("TENANT_DISABLED")));
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
