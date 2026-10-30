//! Test endpoint MCP qua HTTP thật (router đầy đủ của app) với IdP giả ký access token ES256.

use crate::config::Config;
use crate::oidc::tests::{fake_idp, now, sign, FakeIdp};
use crate::routes::build_router;
use crate::state::AppState;
use mongodb::Database;
use serde_json::{json, Value};

struct Server {
    base: String,
    idp: FakeIdp,
    http: reqwest::Client,
}

/// Database không kết nối được (chọn server hỏng nhanh): đủ cho mọi bước trước khi tool đọc dữ liệu.
async fn unreachable_db() -> Database {
    mongodb::Client::with_uri_str("mongodb://127.0.0.1:9/?serverSelectionTimeoutMS=200&connectTimeoutMS=200")
        .await
        .expect("uri")
        .database("scribe_mcp_test")
}

async fn start(db: Database) -> Server {
    let idp = fake_idp().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let base = format!("http://{}", listener.local_addr().expect("addr"));
    let state = AppState::for_tests(Config::for_tests(&base, &idp.issuer), db);
    let app = build_router(state);
    tokio::spawn(async move { axum::serve(listener, app).await });
    Server { base, idp, http: reqwest::Client::new() }
}

impl Server {
    fn token_for(&self, sub: &str, aud: &str, scope: &str) -> String {
        let claims = json!({ "iss": self.idp.issuer, "aud": aud, "sub": sub, "client_id": "mcp-client", "scope": scope,
                             "exp": now() + 300, "iat": now() });
        sign(&self.idp, Some("at+jwt"), "k1", claims)
    }

    fn token(&self, sub: &str) -> String {
        self.token_for(sub, &format!("{}/mcp", self.base), "scribe:read")
    }

    async fn post(&self, token: Option<&str>, body: String) -> reqwest::Response {
        let mut req = self.http.post(format!("{}/mcp", self.base)).header("content-type", "application/json").body(body);
        if let Some(token) = token {
            req = req.bearer_auth(token);
        }
        req.send().await.expect("gửi")
    }

    async fn rpc(&self, token: &str, method: &str, params: Value) -> Value {
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }).to_string();
        let res = self.post(Some(token), body).await;
        assert_eq!(res.status(), 200, "{method}");
        res.json().await.expect("json")
    }

    async fn tool(&self, token: &str, name: &str, arguments: Value) -> Value {
        self.rpc(token, "tools/call", json!({ "name": name, "arguments": arguments })).await["result"].clone()
    }
}

fn challenge(res: &reqwest::Response) -> String {
    res.headers().get("www-authenticate").and_then(|v| v.to_str().ok()).unwrap_or_default().to_string()
}

#[tokio::test]
async fn requests_without_a_valid_token_or_scope_are_refused() {
    let s = start(unreachable_db().await).await;
    let ping = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }).to_string();

    let res = s.post(None, ping.clone()).await;
    assert_eq!(res.status(), 401);
    let header = challenge(&res);
    assert!(header.starts_with("Bearer "), "{header}");
    assert!(header.contains(&format!("resource_metadata=\"{}/.well-known/oauth-protected-resource/mcp\"", s.base)), "{header}");
    assert!(header.contains("scope=\"scribe:read\"") && !header.contains("error="), "{header}");
    assert_eq!(res.headers().get("cache-control").and_then(|v| v.to_str().ok()), Some("no-store"));

    let wrong_aud = s.token_for("acc_1", "https://chat.example/mcp", "scribe:read");
    let res = s.post(Some(&wrong_aud), ping.clone()).await;
    assert_eq!(res.status(), 401);
    assert!(challenge(&res).contains("error=\"invalid_token\""));
    let res = s.post(Some("not-a-jwt"), ping.clone()).await;
    assert_eq!(res.status(), 401, "token rác");

    let no_scope = s.token_for("acc_1", &format!("{}/mcp", s.base), "chat:read");
    let res = s.post(Some(&no_scope), ping.clone()).await;
    assert_eq!(res.status(), 403);
    assert!(challenge(&res).contains("error=\"insufficient_scope\""));
    let body: Value = res.json().await.expect("json");
    assert_eq!(body["error"]["code"], "insufficient_scope");

    let token = s.token("acc_1");
    let foreign = s
        .http
        .post(format!("{}/mcp", s.base))
        .header("origin", "https://evil.example")
        .bearer_auth(&token)
        .body(ping.clone())
        .send()
        .await
        .expect("gửi");
    assert_eq!(foreign.status(), 403, "origin lạ");
    let own = s.http.post(format!("{}/mcp", s.base)).header("origin", &s.base).bearer_auth(&token).body(ping.clone()).send().await.expect("gửi");
    assert_eq!(own.status(), 200, "origin của chính Scribe");

    let big = format!("{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\",\"params\":{{\"x\":\"{}\"}}}}", "a".repeat(1024 * 1024));
    assert_eq!(s.post(Some(&token), big).await.status(), 413);
    assert_eq!(s.post(Some(&token), format!("[{ping}]")).await.status(), 400, "batch");
    assert_eq!(s.post(Some(&token), "{".into()).await.status(), 400, "JSON hỏng");
    let old_version = s.http.post(format!("{}/mcp", s.base)).header("mcp-protocol-version", "1999-01-01").bearer_auth(&token).body(ping).send().await.expect("gửi");
    assert_eq!(old_version.status(), 400, "phiên bản lạ");

    let get = s.http.get(format!("{}/mcp", s.base)).bearer_auth(&token).send().await.expect("gửi");
    assert_eq!(get.status(), 405);
}

#[tokio::test]
async fn protected_resource_metadata_points_at_the_idp() {
    let s = start(unreachable_db().await).await;
    for path in ["/.well-known/oauth-protected-resource", "/.well-known/oauth-protected-resource/mcp"] {
        let res = s.http.get(format!("{}{path}", s.base)).send().await.expect("gửi");
        assert_eq!(res.status(), 200, "{path}");
        assert_eq!(res.headers().get("access-control-allow-origin").and_then(|v| v.to_str().ok()), Some("*"));
        let meta: Value = res.json().await.expect("json");
        assert_eq!(meta["resource"], format!("{}/mcp", s.base));
        assert_eq!(meta["authorization_servers"], json!([s.idp.issuer]));
        assert_eq!(meta["scopes_supported"], json!(["scribe:read"]));
        assert_eq!(meta["bearer_methods_supported"], json!(["header"]));
    }
}

#[tokio::test]
async fn initialize_lists_read_only_tools_and_answers_ping() {
    let s = start(unreachable_db().await).await;
    let token = s.token("acc_1");

    let init = s.rpc(&token, "initialize", json!({ "protocolVersion": "2025-06-18", "capabilities": {},
                                                    "clientInfo": { "name": "test", "version": "1" } })).await;
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(init["result"]["capabilities"]["tools"]["listChanged"], false);
    assert_eq!(init["result"]["serverInfo"]["name"], "scribe");

    let initialized = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }).to_string();
    assert_eq!(s.post(Some(&token), initialized).await.status(), 202);

    let list = s.rpc(&token, "tools/list", json!({})).await;
    let names: Vec<&str> = list["result"]["tools"].as_array().expect("tools").iter().filter_map(|t| t["name"].as_str()).collect();
    assert_eq!(names.len(), 15);
    for expected in ["scribe_list_transcripts", "scribe_get_transcript", "scribe_export_transcript", "scribe_search", "scribe_ask", "scribe_get_usage"] {
        assert!(names.contains(&expected), "{expected}");
    }
    assert!(!names.iter().any(|n| n.contains("delete") || n.contains("upload") || n.contains("create")));

    assert_eq!(s.rpc(&token, "ping", Value::Null).await["result"], json!({}));
    assert_eq!(s.rpc(&token, "resources/list", json!({})).await["error"]["code"], -32601);
    let unknown = s.rpc(&token, "tools/call", json!({ "name": "scribe_delete_file", "arguments": {} })).await;
    assert_eq!(unknown["error"]["code"], -32602);
}

/// Trên Mongo thật: `TRANSCRIPT_TEST_MONGO_URI=… TRANSCRIPT_TEST_DB_NAME=… cargo test -p scribe-be mcp -- --ignored`.
/// Tool đọc đúng bản ghi của user trong token, không thấy dữ liệu của người khác; tài khoản IdP chưa vào Scribe nhận
/// `scribe_account_required`. User giả riêng, tự dọn.
#[tokio::test]
#[ignore]
async fn tools_read_only_the_callers_recordings_on_real_mongo() {
    use crate::models::file::{self, FileDoc, Owner};
    use crate::models::plan::Plan;
    use crate::models::{transcript, transcript_doc, user};
    use crate::transcribe::chunking::Sentence;
    use mongodb::bson::{doc, oid::ObjectId, Document};

    let _sweep = crate::db::WHOLE_DB_SWEEP.lock().await;
    let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
    let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
    let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
    user::ensure_indexes(&db).await.expect("index users");
    transcript_doc::ensure_indexes(&db).await.expect("index transcript_docs");

    let sub = format!("selftest-mcp-{}", uuid::Uuid::new_v4().simple());
    let (user_id, _) = user::find_or_link(&db, &sub, "mcp-selftest@example.com", Some("MCP"), None).await.expect("user");
    let other = format!("selftest-mcp-other-{}", uuid::Uuid::new_v4().simple());
    let (mine, theirs) = (ObjectId::new(), ObjectId::new());
    for (owner, id) in [(user_id.as_str(), mine), (other.as_str(), theirs)] {
        let f = FileDoc::new_recording(Owner { user_id: owner, plan: Plan::Free }, "Họp tuần", "/nonexistent", 10, 60.0, false, id);
        file::create(&db, f).await.expect("tạo file");
        let charge = crate::models::usage::Charge { user_id: owner.into(), month: "test".into(), free_pool: false, day: None };
        transcript::enqueue(&db, id, owner, "run-1", "m", transcript::Billing::Server(&charge), 0).await.expect("enqueue");
        let s = |start: f64, text: &str| Sentence { start, end: start + 1.0, text: text.into(), speaker: None };
        transcript::finish_done(&db, id, "run-1", "", &[s(1.0, "Chào cả nhà."), s(5.0, "Bàn hợp đồng.")]).await.expect("xong");
    }

    let s = start(db.clone()).await;
    let token = s.token(&sub);
    let key = format!("f:{}", mine.to_hex());

    let usage = s.tool(&token, "scribe_get_usage", json!({})).await;
    assert_eq!(usage["isError"], false, "{usage}");
    assert_eq!(usage["structuredContent"]["storage"]["usageBytes"], 10);
    assert_eq!(usage["structuredContent"]["plan"]["name"], "free");

    let files = s.tool(&token, "scribe_list_files", json!({})).await;
    let items = files["structuredContent"]["items"].as_array().expect("items").clone();
    assert_eq!(items.len(), 1, "chỉ file của chính user");
    assert_eq!(items[0]["transcriptKey"], key.as_str());

    let page = s.tool(&token, "scribe_get_transcript", json!({ "key": key, "limit": 1 })).await;
    let page = &page["structuredContent"];
    assert_eq!(page["sentences"], json!([{ "n": 0, "fileId": mine.to_hex(), "at": 1.0, "end": 2.0, "speaker": null, "text": "Chào cả nhà." }]));
    assert_eq!((page["total"].clone(), page["hasMore"].clone(), page["nextOffset"].clone()), (json!(2), json!(true), json!(1)));

    let export = s.tool(&token, "scribe_export_transcript", json!({ "key": key, "format": "txt" })).await;
    assert_eq!(export["isError"], false, "{export}");
    assert!(export["content"][0]["text"].as_str().is_some_and(|t| t.contains("Bàn hợp đồng.")), "{export}");

    let foreign = s.tool(&token, "scribe_get_file", json!({ "file_id": theirs.to_hex() })).await;
    assert_eq!((foreign["isError"].clone(), foreign["structuredContent"]["error"]["code"].clone()), (json!(true), json!("not_found")));
    let foreign = s.tool(&token, "scribe_get_transcript", json!({ "key": format!("f:{}", theirs.to_hex()) })).await;
    assert_eq!((foreign["isError"].clone(), foreign["structuredContent"]["error"]["code"].clone()), (json!(true), json!("not_found")));

    let bad = s.tool(&token, "scribe_get_transcript", json!({ "key": key, "extra": 1 })).await;
    assert_eq!(bad["structuredContent"]["error"]["code"], "invalid_argument");

    let stranger = s.token(&format!("selftest-mcp-none-{}", uuid::Uuid::new_v4().simple()));
    let refused = s.tool(&stranger, "scribe_list_transcripts", json!({})).await;
    assert_eq!((refused["isError"].clone(), refused["structuredContent"]["error"]["code"].clone()), (json!(true), json!("scribe_account_required")));

    for owner in [user_id.as_str(), other.as_str()] {
        for coll in ["files", "transcripts", "transcript_sentences", "transcript_docs"] {
            db.collection::<Document>(coll).delete_many(doc! { "userId": owner }).await.expect("dọn");
        }
    }
    let oid = ObjectId::parse_str(&user_id).expect("id");
    db.collection::<Document>("users").delete_one(doc! { "_id": oid }).await.expect("dọn user");
}
