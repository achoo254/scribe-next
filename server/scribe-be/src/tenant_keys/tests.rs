//! Chọn key (ma trận), mở key tenant qua API nội bộ giả, báo key lỗi về IdP mà không đụng key server.

use super::sealed::tests::{vector, vector_key};
use super::sealed::{context, seal_for_tests, Keyring};
use super::*;
use crate::oidc_registry::OidcRegistry;
use crate::tenant::directory::tests::{directory, SECRET};
use crate::tenant::{test_tenant, Readiness, PRIMARY_ID};
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use rsa::pkcs8::DecodePublicKey;
use rsa::RsaPublicKey;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// IdP giả: `scribe-keys` theo tenant (Basic `scribe`), ghi lại mọi lần báo trạng thái; `/messages` đếm báo động gửi
/// cho người vận hành (bot chat).
struct FakeIdp {
    url: String,
    gets: Arc<AtomicUsize>,
    reports: Arc<Mutex<Vec<(String, Value)>>>,
    alerts: Arc<AtomicUsize>,
}

#[derive(Clone)]
struct Fake {
    keys: Arc<HashMap<String, Value>>,
    gets: Arc<AtomicUsize>,
    reports: Arc<Mutex<Vec<(String, Value)>>>,
    alerts: Arc<AtomicUsize>,
}

fn authorized(headers: &HeaderMap) -> bool {
    let expected = format!("Basic {}", STANDARD.encode(format!("scribe:{SECRET}")));
    headers.get("authorization").and_then(|v| v.to_str().ok()) == Some(expected.as_str())
}

async fn list(State(f): State<Fake>, headers: HeaderMap, Path(id): Path<String>) -> axum::response::Response {
    if !authorized(&headers) {
        return axum::http::StatusCode::UNAUTHORIZED.into_response();
    }
    f.gets.fetch_add(1, Ordering::SeqCst);
    match f.keys.get(&id) {
        Some(keys) => Json(json!({ "keys": keys })).into_response(),
        None => (axum::http::StatusCode::NOT_FOUND, Json(json!({ "error": "unknown_tenant" }))).into_response(),
    }
}

async fn status(
    State(f): State<Fake>,
    headers: HeaderMap,
    Path((id, vendor)): Path<(String, String)>,
    Json(body): Json<Value>,
) -> axum::http::StatusCode {
    if !authorized(&headers) {
        return axum::http::StatusCode::UNAUTHORIZED;
    }
    f.reports.lock().expect("lock").push((format!("{id}/{vendor}"), body));
    axum::http::StatusCode::NO_CONTENT
}

async fn alert(State(f): State<Fake>) -> axum::http::StatusCode {
    f.alerts.fetch_add(1, Ordering::SeqCst);
    axum::http::StatusCode::OK
}

async fn fake_idp(keys: HashMap<String, Value>) -> FakeIdp {
    let fake = Fake { keys: Arc::new(keys), gets: Arc::default(), reports: Arc::default(), alerts: Arc::default() };
    let app = Router::new()
        .route("/internal/tenants/{id}/scribe-keys", get(list))
        .route("/internal/tenants/{id}/scribe-keys/{vendor}/status", post(status))
        .route("/messages", post(alert))
        .with_state(fake.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let url = format!("http://{}", listener.local_addr().expect("addr"));
    tokio::spawn(async move { axum::serve(listener, app).await });
    FakeIdp { url, gets: fake.gets, reports: fake.reports, alerts: fake.alerts }
}

fn vector_public() -> RsaPublicKey {
    RsaPublicKey::from_public_key_pem(vector()["publicKeySpkiPem"].as_str().expect("pub")).expect("public key")
}

/// Một key như API nội bộ trả: niêm phong `plain` cho tenant `tenant_id` (khoá của vector).
fn sealed_key(tenant_id: &str, vendor: Vendor, plain: &str, status: Option<&str>) -> Value {
    let s = seal_for_tests(&vector_public(), &context(tenant_id, vendor.code()), plain);
    let mut key = json!({
        "vendor": vendor.code(), "hint": "abcd", "updatedAt": "2026-10-24T05:00:00.000Z",
        "sealed": { "kid": "recipient", "iv": s.iv, "ciphertext": s.ciphertext, "tag": s.tag,
                    "wrappedKey": s.wrapped_key, "recipient": s.recipient },
    });
    if let Some(status) = status {
        key["status"] = json!(status);
    }
    key
}

fn temp_file(content: &str) -> String {
    let path = std::env::temp_dir().join(format!("scribe-tk-{}", uuid::Uuid::new_v4()));
    std::fs::write(&path, content).expect("ghi");
    path.to_string_lossy().into_owned()
}

/// State có key server (ElevenLabs + OpenAI), báo động trỏ về IdP giả, vòng khoá của vector.
async fn state_with(idp: &FakeIdp) -> AppState {
    let mut config = Config::for_tests();
    config.elevenlabs_key_path = Some(temp_file("sk_test_server_elevenlabs_key_0001"));
    config.openai_key_path = Some(temp_file("sk-test-server-openai-key"));
    config.notify_bot_api = idp.url.clone();
    config.notify_chat_id = Some("1".into());
    config.notify_bot_token_path = Some(temp_file("bot-token"));
    let db = crate::db::connect("mongodb://127.0.0.1:1", "unused").await.expect("client");
    let oidc = OidcRegistry::new(crate::oidc::Oidc::new(&config.oidc_issuer), "scribe-web", "test-secret".into(), None);
    let mut state = AppState::for_tests_with(config, db, directory(None, Readiness::default()), oidc);
    let api = Arc::new(InternalApi::new(&idp.url, SECRET).expect("client"));
    state.tenant_keys = Some(Arc::new(TenantKeys::new(Keyring::new(vec![vector_key()]), api)));
    state
}

fn code(e: AppError) -> &'static str {
    match e {
        AppError::Coded { code, .. } => code,
        AppError::Unavailable(_) => "UNAVAILABLE",
        _ => "OTHER",
    }
}

#[test]
fn the_choice_matrix_never_gives_a_tenant_the_server_key() {
    let t = Use::Transcribe;
    // Tenant chính: không key ⟹ Server; key riêng ⟹ Own. Key tenant không bao giờ áp ở tenant chính.
    assert_eq!(decide(true, None, &[], t).expect("ok"), Choice::Server);
    assert_eq!(decide(true, Some(Vendor::OpenAi), &[], t).expect("ok"), Choice::Own(Vendor::OpenAi));
    assert_eq!(decide(true, None, &[Vendor::ElevenLabs], t).expect("ok"), Choice::Server);
    // Tenant sổ: không key ⟹ TENANT_KEY_MISSING; key tenant ⟹ Tenant; key riêng + key tenant ⟹ Own.
    assert_eq!(code(decide(false, None, &[], t).expect_err("thiếu key")), "TENANT_KEY_MISSING");
    assert_eq!(decide(false, None, &[Vendor::Gemini], t).expect("ok"), Choice::Tenant(Vendor::Gemini));
    assert_eq!(decide(false, Some(Vendor::ElevenLabs), &[Vendor::OpenAi], t).expect("ok"), Choice::Own(Vendor::ElevenLabs));
    // Thứ tự mặc định của từng việc; Claude chỉ làm model chữ.
    assert_eq!(decide(false, None, &[Vendor::Gemini, Vendor::ElevenLabs], t).expect("ok"), Choice::Tenant(Vendor::ElevenLabs));
    assert_eq!(code(decide(false, None, &[Vendor::Anthropic], t).expect_err("Claude không chép lời")), "TENANT_KEY_MISSING");
    assert_eq!(decide(false, None, &[Vendor::Anthropic, Vendor::ElevenLabs], Use::Text).expect("ok"), Choice::Tenant(Vendor::Anthropic));
    // Mọi tổ hợp ở tenant sổ: không bao giờ ra Server.
    let sets: [&[Vendor]; 4] = [&[], &[Vendor::ElevenLabs], &[Vendor::Anthropic], &Vendor::ALL];
    for use_ in [Use::Transcribe, Use::Live, Use::Text] {
        for own in [None, Some(Vendor::OpenAi)] {
            for set in sets {
                assert_ne!(decide(false, own, set, use_).ok(), Some(Choice::Server), "{use_:?} {own:?} {set:?}");
            }
        }
    }
}

#[tokio::test]
async fn choose_reads_the_tenant_keys_and_never_falls_back_to_the_server_key() {
    let keys = HashMap::from([
        ("ten_a".to_string(), json!([sealed_key("ten_a", Vendor::OpenAi, "sk-a", None)])),
        (
            "ten_b".to_string(),
            json!([
                sealed_key("ten_b", Vendor::Anthropic, "sk-ant-b", None),
                sealed_key("ten_b", Vendor::ElevenLabs, "sk_b_elevenlabs", None),
                sealed_key("ten_b", Vendor::Gemini, "gm-b", Some("invalid")),
            ]),
        ),
        ("ten_c".to_string(), json!([])),
        (PRIMARY_ID.to_string(), json!([sealed_key(PRIMARY_ID, Vendor::OpenAi, "sk-p", None)])),
    ]);
    let idp = fake_idp(keys).await;
    let state = state_with(&idp).await;
    assert!(state.transcriber.ready() && state.transcriber.text_key().is_some(), "state có key server");
    let pick = |id: &str, use_: Use| {
        let (state, tenant) = (state.clone(), test_tenant(id, true));
        async move { choose(&state, &tenant, "u1", use_).await }
    };
    // Tenant chính: key server như trước, không hỏi IdP.
    assert_eq!(pick(PRIMARY_ID, Use::Transcribe).await.expect("ok"), Choice::Server);
    assert_eq!(idp.gets.load(Ordering::SeqCst), 0);
    assert_eq!(pick("ten_a", Use::Transcribe).await.expect("ok"), Choice::Tenant(Vendor::OpenAi));
    assert_eq!(pick("ten_a", Use::Text).await.expect("ok"), Choice::Tenant(Vendor::OpenAi));
    assert_eq!(pick("ten_b", Use::Transcribe).await.expect("ok"), Choice::Tenant(Vendor::ElevenLabs));
    assert_eq!(pick("ten_b", Use::Text).await.expect("ok"), Choice::Tenant(Vendor::Anthropic));
    // Không key, IdP không biết tenant: thiếu key, dù state có key server.
    assert_eq!(code(pick("ten_c", Use::Transcribe).await.expect_err("thiếu")), "TENANT_KEY_MISSING");
    assert_eq!(code(pick("ten_gone", Use::Text).await.expect_err("thiếu")), "TENANT_KEY_MISSING");
    // Cache 60 giây: mỗi tenant một lần hỏi.
    assert_eq!(idp.gets.load(Ordering::SeqCst), 4);
    let me = me_summary(&state, &test_tenant("ten_b", true)).await;
    assert_eq!(me, json!({ "transcribe": "elevenlabs", "text": "anthropic" }));
    // Không có vòng khoá: tenant sổ không có key nào.
    let mut off = state.clone();
    off.tenant_keys = None;
    assert_eq!(code(choose(&off, &test_tenant("ten_a", true), "u1", Use::Transcribe).await.expect_err("tắt")), "TENANT_KEY_MISSING");
    // IdP không chạy và chưa có gì trong cache: 503, không phải key server.
    let mut down = state.clone();
    let api = Arc::new(InternalApi::new("http://127.0.0.1:1", SECRET).expect("client"));
    down.tenant_keys = Some(Arc::new(TenantKeys::new(Keyring::new(vec![vector_key()]), api)));
    assert_eq!(code(choose(&down, &test_tenant("ten_a", true), "u1", Use::Text).await.expect_err("IdP lỗi")), "UNAVAILABLE");
}

#[tokio::test]
async fn a_failing_tenant_key_is_reported_to_the_idp_and_never_touches_the_server_key() {
    let copied = sealed_key("ten_a", Vendor::Gemini, "gm-a", None);
    let keys = HashMap::from([
        ("ten_a".to_string(), json!([sealed_key("ten_a", Vendor::OpenAi, "sk-tenant-a", None)])),
        // Bản niêm phong của tenant khác chép sang: sai AAD, không mở được.
        ("ten_b".to_string(), json!([copied])),
    ]);
    let idp = fake_idp(keys).await;
    let state = state_with(&idp).await;

    let key = open(&state, "ten_a", Vendor::OpenAi).await.expect("mở được");
    assert_eq!((key.value(), key.vendor), ("sk-tenant-a", Vendor::OpenAi));
    assert_eq!(open(&state, "ten_a", Vendor::Gemini).await.err(), Some(failure_message(Vendor::Gemini)), "không có key đó");

    let message = report(&state, &key, Problem::Invalid).await;
    assert_eq!(message, "Key OpenAI của domain không dùng được — chủ domain sửa ở trang quản trị.");
    // Báo một lần cho cùng bản niêm phong.
    assert_eq!(report(&state, &key, Problem::Invalid).await, message);
    let unreadable = open(&state, "ten_b", Vendor::Gemini).await.err().expect("không mở được");
    assert_eq!(unreadable, failure_message(Vendor::Gemini));
    let reports = idp.reports.lock().expect("lock").clone();
    assert_eq!(reports.len(), 2, "{reports:?}");
    assert_eq!(reports[0].0, "ten_a/openai");
    assert_eq!(reports[0].1["status"], "invalid");
    assert_eq!(reports[0].1["recipient"], vector()["recipient"]);
    assert_eq!(reports[0].1["tag"].as_str().map(str::len), Some(22), "tag của đúng bản niêm phong");
    assert_eq!((reports[1].0.as_str(), reports[1].1["status"].as_str()), ("ten_b/gemini", Some("unreadable")));
    // Lượt sau đọc lại cache (không hỏi IdP thêm) và không dùng lại key đã lỗi.
    assert_eq!(open(&state, "ten_a", Vendor::OpenAi).await.err(), Some(message.clone()));
    assert_eq!(code(choose(&state, &test_tenant("ten_a", true), "u1", Use::Text).await.expect_err("hết key")), "TENANT_KEY_MISSING");
    assert_eq!(idp.gets.load(Ordering::SeqCst), 2);

    // Lỗi model chữ bằng key tenant: 403 TENANT_KEY_FAILED, không tạm ngưng tóm tắt.
    let text = crate::summary::TextKey::Tenant { key: open_fresh(&idp, Vendor::OpenAi).await, model: "gpt-4.1-mini".into() };
    let err = crate::summary::key_failure(&state, &text, crate::summary::llm::KEY_INVALID, crate::summary::PAUSED).await;
    assert_eq!(err.map(code), Some("TENANT_KEY_FAILED"));
    // Key server vẫn chạy, không báo động nào cho người vận hành.
    assert!(state.transcriber.ready() && !state.summaries.paused());
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(idp.alerts.load(Ordering::SeqCst), 0);
    // Đối chứng: báo động thật thì IdP giả có đếm.
    crate::notify::send(&state, "đối chứng".into());
    for _ in 0..50 {
        if idp.alerts.load(Ordering::SeqCst) > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(idp.alerts.load(Ordering::SeqCst), 1);
}

/// Key `vendor` của `ten_a` mở bằng state mới (cache riêng, chưa có dấu lỗi).
async fn open_fresh(idp: &FakeIdp, vendor: Vendor) -> TenantKey {
    open(&state_with(idp).await, "ten_a", vendor).await.expect("mở được")
}

#[tokio::test]
async fn the_worker_opens_the_key_of_the_run_tenant() {
    let keys = HashMap::from([("ten_a".to_string(), json!([sealed_key("ten_a", Vendor::ElevenLabs, "sk_tenant_a_eleven", None)]))]);
    let idp = fake_idp(keys).await;
    let state = state_with(&idp).await;
    let run = |tenant: &str, key_source: Option<&str>| {
        let mut d = mongodb::bson::doc! {
            "_id": mongodb::bson::oid::ObjectId::new(), "userId": "u1", "tenantId": tenant, "status": "running",
            "progress": 0.0, "runId": "r", "model": "scribe_v2", "updatedAt": mongodb::bson::DateTime::now(),
            "byok": "elevenlabs", "open": true,
        };
        if let Some(source) = key_source {
            d.insert("keySource", source);
        }
        mongodb::bson::from_document::<crate::models::transcript::TranscriptDoc>(d).expect("doc")
    };
    let key = crate::transcribe::open_byok(&state, &run("ten_a", Some("tenant"))).await.expect("mở").expect("có key");
    assert_eq!((key.value(), key.provider()), ("sk_tenant_a_eleven", Vendor::ElevenLabs));
    assert!(matches!(key, crate::transcribe::ByokKey::Tenant(_)));
    // Tenant khác trên doc: không có key của nó.
    assert!(crate::transcribe::open_byok(&state, &run("ten_x", Some("tenant"))).await.is_err());
    // Không `keySource`: key riêng của người dùng (tính năng tắt trong state test), không bao giờ key tenant.
    let own = crate::transcribe::open_byok(&state, &run("ten_a", None)).await.err().expect("key riêng tắt");
    assert!(own.contains("Key riêng"), "{own}");
}
