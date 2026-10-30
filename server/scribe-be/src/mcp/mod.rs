//! Endpoint MCP (Model Context Protocol) cho công cụ AI: `POST /mcp`, Streamable HTTP không trạng thái, mỗi request
//! một message JSON-RPC, trả JSON (không SSE). Chỉ nhận access token của IdP (`aud` = `<PUBLIC_URL>/mcp`, scope
//! `scribe:read`), không nhận cookie phiên. Mọi tool chỉ đọc và gọi đúng handler `/api` với user của token, nên
//! quyền và hạn mức như trong app. Protected Resource Metadata (RFC 9728) ở `/.well-known/oauth-protected-resource`
//! và `/.well-known/oauth-protected-resource/mcp`. Quyết định: `docs/decisions/0002-mcp-endpoint.md`.

mod calls;
mod rpc;
mod tools;

#[cfg(test)]
mod tests;

use crate::auth::UserId;
use crate::models::user;
use crate::oidc::AccessTokenError;
use crate::state::AppState;
use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use calls::ToolError;
use rpc::{Incoming, ParseError};
use serde_json::{json, Value};

/// Scope duy nhất của resource MCP ở IdP (auth-next `SCRIBE_MCP_SCOPES`): chỉ đọc.
pub const SCOPE_READ: &str = "scribe:read";
const SCOPES_SUPPORTED: [&str; 1] = [SCOPE_READ];
const MCP_PATH: &str = "/mcp";
const METADATA_PATH: &str = "/.well-known/oauth-protected-resource";
const METADATA_RESOURCE_PATH: &str = "/.well-known/oauth-protected-resource/mcp";
const MAX_BODY_BYTES: usize = 1024 * 1024;
/// Request MCP tối đa mỗi phút của một tài khoản IdP (như Chat MCP).
const PER_MINUTE: u32 = 120;

const INSTRUCTIONS: &str = "Read-only access to the user's Scribe recordings: transcripts, summaries, translations, \
speakers, bookmarks, files, split jobs and usage. Start with scribe_list_transcripts (or scribe_search) to find a \
recording key, then read it with scribe_get_transcript. Times are seconds from the start of the recording. \
scribe_ask uses the account's question quota; the other tools are free. Nothing can be uploaded, transcribed, edited, \
shared or deleted through this server.";

pub fn router() -> Router<AppState> {
    Router::new()
        .route(MCP_PATH, post(serve).get(method_not_allowed).delete(method_not_allowed))
        .route(METADATA_PATH, get(metadata).options(metadata))
        .route(METADATA_RESOURCE_PATH, get(metadata).options(metadata))
}

/// Resource indicator (và `aud` của token): `<PUBLIC_URL>/mcp`.
fn resource(state: &AppState) -> String {
    format!("{}{MCP_PATH}", state.config.public_url)
}

fn metadata_url(state: &AppState) -> String {
    format!("{}{METADATA_RESOURCE_PATH}", state.config.public_url)
}

/// RFC 9728: công khai, cache được, đọc được từ mọi origin (client tìm nó trước khi đăng nhập).
async fn metadata(State(state): State<AppState>, method: Method) -> Response {
    let cors = [(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"), (header::ACCESS_CONTROL_ALLOW_METHODS, "GET, OPTIONS")];
    if method == Method::OPTIONS {
        return (StatusCode::NO_CONTENT, cors).into_response();
    }
    let body = json!({
        "resource": resource(&state),
        "authorization_servers": [state.oidc.issuer],
        "scopes_supported": SCOPES_SUPPORTED,
        "bearer_methods_supported": ["header"],
        "resource_name": "Scribe",
    });
    (cors, [(header::CACHE_CONTROL, "public, max-age=300")], Json(body)).into_response()
}

fn http_error(status: StatusCode, code: &str, message: &str) -> Response {
    (status, Json(json!({ "error": { "code": code, "message": message } }))).into_response()
}

/// Server không trạng thái: không có luồng SSE (GET) hay phiên để đóng (DELETE).
async fn method_not_allowed() -> Response {
    let mut res = http_error(StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed", "send JSON-RPC messages with POST");
    res.headers_mut().insert(header::ALLOW, HeaderValue::from_static("POST"));
    res
}

/// 401/403 kèm challenge Bearer trỏ tới metadata (RFC 6750 §3, RFC 9728 §5.1).
fn challenge(state: &AppState, status: StatusCode, oauth_error: Option<(&str, &str)>) -> Response {
    let mut params = Vec::new();
    if let Some((error, description)) = oauth_error {
        params.push(format!("error=\"{error}\""));
        params.push(format!("error_description=\"{description}\""));
    }
    params.push(format!("scope=\"{}\"", SCOPES_SUPPORTED.join(" ")));
    params.push(format!("resource_metadata=\"{}\"", metadata_url(state)));
    let (code, message) = match oauth_error {
        Some((error, description)) => (error, description),
        None => ("unauthenticated", "an access token from the sign-in server is required"),
    };
    let mut res = http_error(status, code, message);
    match HeaderValue::from_str(&format!("Bearer {}", params.join(", "))) {
        Ok(value) => {
            res.headers_mut().insert(header::WWW_AUTHENTICATE, value);
        }
        Err(_) => tracing::error!("PUBLIC_URL không đặt được vào WWW-Authenticate"),
    }
    res
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    let token = token.trim();
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty()).then_some(token)
}

/// Request từ trình duyệt chỉ được đến từ chính web Scribe; client MCP ngoài trình duyệt không gửi `Origin`.
fn origin_allowed(state: &AppState, origin: &HeaderValue) -> bool {
    origin.to_str().is_ok_and(|o| o.eq_ignore_ascii_case(&state.config.public_url))
}

async fn serve(State(state): State<AppState>, headers: HeaderMap, body: Body) -> Response {
    let mut res = handle(&state, &headers, body).await;
    res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

async fn handle(state: &AppState, headers: &HeaderMap, body: Body) -> Response {
    if headers.get(header::ORIGIN).is_some_and(|origin| !origin_allowed(state, origin)) {
        return http_error(StatusCode::FORBIDDEN, "origin_rejected", "origin not allowed");
    }
    let Some(token) = bearer(headers) else {
        return challenge(state, StatusCode::UNAUTHORIZED, None);
    };
    let claims = match state.oidc.verify_access_token(&state.http, token, &resource(state)).await {
        Ok(claims) => claims,
        Err(AccessTokenError::Unavailable) => {
            let mut res = http_error(StatusCode::SERVICE_UNAVAILABLE, "auth_unavailable", "sign-in is not available yet; retry shortly");
            res.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from_static("5"));
            return res;
        }
        Err(AccessTokenError::Invalid) => {
            return challenge(state, StatusCode::UNAUTHORIZED, Some(("invalid_token", "the access token is invalid or expired")));
        }
    };
    if !claims.has_scope(SCOPE_READ) {
        return challenge(state, StatusCode::FORBIDDEN, Some(("insufficient_scope", "the token carries no Scribe scope")));
    }
    if !state.rate.allow(&format!("mcp:{}", claims.sub), PER_MINUTE) {
        let mut res = http_error(StatusCode::TOO_MANY_REQUESTS, "rate_limited", "too many MCP requests; slow down");
        res.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from_static("1"));
        return res;
    }
    let version = headers.get("mcp-protocol-version").map(|v| v.to_str().unwrap_or_default());
    if version.is_some_and(|v| !rpc::PROTOCOL_VERSIONS.contains(&v)) {
        return http_error(StatusCode::BAD_REQUEST, "unsupported_protocol_version", "unsupported MCP-Protocol-Version");
    }
    let declared = headers.get(header::CONTENT_LENGTH).and_then(|v| v.to_str().ok()?.parse::<usize>().ok());
    if declared.is_some_and(|len| len > MAX_BODY_BYTES) {
        return http_error(StatusCode::PAYLOAD_TOO_LARGE, "payload_too_large", "a message is at most 1 MiB");
    }
    let Ok(bytes) = axum::body::to_bytes(body, MAX_BODY_BYTES).await else {
        return http_error(StatusCode::PAYLOAD_TOO_LARGE, "payload_too_large", "a message is at most 1 MiB (or the body was cut off)");
    };
    let (id, method, params) = match rpc::parse(&bytes) {
        Ok(Incoming::Request { id, method, params }) => (id, method, params),
        Ok(Incoming::Accepted) => return StatusCode::ACCEPTED.into_response(),
        Err(ParseError::Batch) => {
            let reply = rpc::failure(&Value::Null, rpc::INVALID_REQUEST, "send one JSON-RPC message per request");
            return (StatusCode::BAD_REQUEST, Json(reply)).into_response();
        }
        Err(ParseError::Invalid) => {
            let reply = rpc::failure(&Value::Null, rpc::PARSE_ERROR, "the body must be exactly one JSON-RPC 2.0 message object");
            return (StatusCode::BAD_REQUEST, Json(reply)).into_response();
        }
    };
    let (reply, user) = dispatch(state, &claims.sub, &id, &method, &params).await;
    let mut res = Json(reply).into_response();
    if let Some(user) = user {
        // Cho access log (lớp ngoài) biết request thuộc user nào.
        res.extensions_mut().insert(user);
    }
    res
}

async fn dispatch(state: &AppState, sub: &str, id: &Value, method: &str, params: &Value) -> (Value, Option<UserId>) {
    match method {
        "initialize" => {
            let version = rpc::negotiate(params["protocolVersion"].as_str());
            (rpc::success(id, initialize_result(version)), None)
        }
        "ping" => (rpc::success(id, json!({})), None),
        "tools/list" => (rpc::success(id, json!({ "tools": tools::definitions() })), None),
        "tools/call" => {
            let Some(name) = params["name"].as_str() else {
                return (rpc::failure(id, rpc::INVALID_PARAMS, "params.name is required"), None);
            };
            if !tools::is_known(name) {
                return (rpc::failure(id, rpc::INVALID_PARAMS, &format!("unknown tool: {name}")), None);
            }
            let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);
            if !(arguments.is_object() || arguments.is_null()) {
                return (rpc::failure(id, rpc::INVALID_PARAMS, "params.arguments must be an object"), None);
            }
            match caller(state, sub).await {
                Ok(user) => {
                    let outcome = calls::call(state, &user, name, arguments).await;
                    (rpc::success(id, calls::result(outcome)), Some(user))
                }
                Err(e) => (rpc::success(id, calls::result(Err(e))), None),
            }
        }
        _ => (rpc::failure(id, rpc::METHOD_NOT_FOUND, &format!("method not found: {method}")), None),
    }
}

fn initialize_result(version: &str) -> Value {
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": "scribe", "title": "Scribe", "version": env!("CARGO_PKG_VERSION") },
        "instructions": INSTRUCTIONS,
    })
}

/// User Scribe của tài khoản IdP `sub`, với cùng điều kiện mà phiên web kiểm (`auth::require_auth`): chưa khoá,
/// không đang xoá, email còn được phép.
async fn caller(state: &AppState, sub: &str) -> Result<UserId, ToolError> {
    let internal = |e| {
        tracing::error!("MCP đọc user lỗi: {e:?}");
        ToolError::internal()
    };
    let required = || {
        ToolError::new(
            "scribe_account_required",
            format!(
                "this account has never signed in to Scribe; open {} and sign in once, then retry",
                state.config.public_url
            ),
        )
    };
    let id = user::find_id_by_sub(&state.db, sub).await.map_err(internal)?.ok_or_else(required)?;
    let profile = user::find_profile(&state.db, &id).await.map_err(internal)?.ok_or_else(required)?;
    if profile.deleting {
        return Err(ToolError::new("account_pending_deletion", "this Scribe account is being deleted"));
    }
    if profile.blocked {
        return Err(ToolError::new("account_blocked", "this Scribe account is blocked"));
    }
    if !state.config.email_allowed(&profile.email) {
        return Err(ToolError::new("account_not_allowed", "this account is not allowed to use Scribe"));
    }
    Ok(UserId(id))
}
