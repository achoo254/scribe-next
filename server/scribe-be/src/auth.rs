//! Đăng nhập qua hoandat-auth (OIDC) và phiên của Scribe.
//!
//! Web: backend là BFF. `/api/auth/login` chuyển tới IdP (Code + PKCE, `state`/`nonce`/verifier lưu ở
//! `auth_states`), `/api/auth/callback` đổi code bằng client confidential, xác minh ID token, ghép user,
//! tạo phiên và cookie HttpOnly. Không token nào của IdP tới JavaScript.
//!
//! iPhone: app tự sinh PKCE và `nonce`, mở trang đăng nhập bằng `ASWebAuthenticationSession`, gửi `code`,
//! verifier và `nonce` tới `/api/auth/native/exchange`. Refresh token IdP của thiết bị ở lại server (mã
//! hoá); app giữ token thiết bị của Scribe để làm mới phiên khi đang ghi nền.
//!
//! Phiên lưu trong Mongo (`models::session`), thu hồi được: đăng xuất, back-channel logout của IdP, máy
//! bị thu hồi ở `/account` (lần làm mới sau nhận `invalid_grant`).

use crate::audit;
use crate::error::AppError;
use crate::models::session::{self as sessions, Client};
use crate::models::{plan, usage, user};
use crate::oidc::{GrantError, SCOPE};
use crate::rate_limit::IDP_TOKEN_BUDGET;
use crate::session::{open, random_token, seal};
use crate::state::AppState;
use axum::extract::{Form, Query, Request, State};
use axum::http::{header, HeaderValue};
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use axum::{Extension, Json};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use mongodb::bson::doc;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Cookie gắn lượt đăng nhập web với trình duyệt đã bắt đầu nó. Trên https là `__Host-` (Path=/, không
/// Domain): subdomain khác của hoandat.com (vd tunnel do người dùng tự phục vụ) không ghi đè được.
fn login_cookie_name(state: &AppState) -> &'static str {
    if state.config.public_url.starts_with("https://") {
        "__Host-scribe_login"
    } else {
        "scribe_login"
    }
}
/// Phiên vừa kiểm còn hiệu lực bao lâu trong RAM; đăng xuất/thu hồi xoá ngay khỏi cache.
const CACHE_TTL: Duration = Duration::from_secs(30);

/// User id gắn vào request sau khi middleware kiểm phiên.
#[derive(Clone, Debug)]
pub struct UserId(pub String);

/// Phiên đã kiểm gần đây: băm token → (user, lúc kiểm).
#[derive(Default)]
pub struct SessionCache {
    entries: Mutex<HashMap<String, (String, Instant)>>,
}

impl SessionCache {
    fn get(&self, id: &str) -> Option<String> {
        let entries = self.entries.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        entries.get(id).filter(|(_, at)| at.elapsed() < CACHE_TTL).map(|(u, _)| u.clone())
    }
    fn put(&self, id: String, user: String) {
        let mut entries = self.entries.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        entries.retain(|_, (_, at)| at.elapsed() < CACHE_TTL);
        entries.insert(id, (user, Instant::now()));
    }
    pub fn forget(&self, ids: &[String]) {
        let mut entries = self.entries.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        for id in ids {
            entries.remove(id);
        }
    }
}

fn cookie_name(state: &AppState) -> &'static str {
    if state.config.public_url.starts_with("https://") {
        "__Host-scribe"
    } else {
        // Dev local http: `__Host-` bắt buộc Secure, iOS simulator không gửi cookie Secure qua http.
        "scribe"
    }
}

fn session_cookie(state: &AppState, value: String, ttl_secs: i64) -> Cookie<'static> {
    Cookie::build((cookie_name(state), value))
        .http_only(true)
        .secure(state.config.public_url.starts_with("https://"))
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time::Duration::seconds(ttl_secs))
        .build()
}

/// Mỗi lần gọi token endpoint của IdP trừ vào ngân sách chung của server (IdP giới hạn theo IP).
fn take_idp_budget(state: &AppState) -> Result<(), AppError> {
    if state.rate.allow("idp-token", IDP_TOKEN_BUDGET) {
        Ok(())
    } else {
        Err(AppError::Unavailable("máy chủ đăng nhập đang bận, thử lại sau ít phút".into()))
    }
}

fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn callback_url(state: &AppState) -> String {
    format!("{}/api/auth/callback", state.config.public_url)
}

/// Kết thúc lượt đăng nhập web có lỗi: về trang đăng nhập kèm mã ngắn (trang tự dịch ra câu).
pub(crate) fn login_failed(state: &AppState, code: &str) -> Response {
    Redirect::to(&format!("{}/login?error={code}", state.config.public_url)).into_response()
}

/// Kiểm danh tính từ IdP rồi ghép user. Email chưa xác minh hoặc ngoài allowlist thì từ chối.
async fn admit(state: &AppState, claims: &crate::oidc::IdClaims) -> Result<String, AppError> {
    let email = claims.email.as_deref().filter(|e| !e.is_empty()).filter(|_| claims.email_verified);
    let Some(email) = email else {
        audit::record(state, None, "auth.login_denied", doc! { "reason": "email_unverified" });
        return Err(AppError::Forbidden("email chưa được xác minh".into()));
    };
    if !state.config.email_allowed(email) {
        audit::record(state, None, "auth.login_denied", doc! { "email": email });
        return Err(AppError::Forbidden("email không có quyền truy cập".into()));
    }
    let (user_id, linked) = user::find_or_link(&state.db, &claims.sub, email, claims.name.as_deref()).await?;
    if linked {
        audit::record(state, Some(&user_id), "auth.link", doc! { "email": email });
    }
    if user::find_profile(&state.db, &user_id).await?.is_some_and(|p| p.blocked) {
        audit::record(state, Some(&user_id), "auth.login_denied", doc! { "reason": "blocked" });
        return Err(AppError::Forbidden("tài khoản đã bị khoá".into()));
    }
    Ok(user_id)
}

// MARK: Web

/// `GET /api/auth/login` — bắt đầu đăng nhập web: lưu lượt, chuyển tới IdP.
pub async fn login(State(state): State<AppState>, jar: CookieJar) -> Result<(CookieJar, Redirect), AppError> {
    let discovery = state.oidc.discovery(&state.http).await?;
    let (login_state, verifier, nonce) = (random_token(), random_token(), random_token());
    sessions::save_state(&state.db, &login_state, &verifier, &nonce).await?;
    let mut url = reqwest::Url::parse(&discovery.authorization_endpoint)
        .map_err(|e| AppError::Internal(format!("authorization endpoint: {e}")))?;
    url.query_pairs_mut()
        .append_pair("client_id", &state.config.oidc_web_client_id)
        .append_pair("response_type", "code")
        .append_pair("scope", SCOPE)
        .append_pair("redirect_uri", &callback_url(&state))
        .append_pair("state", &login_state)
        .append_pair("nonce", &nonce)
        .append_pair("code_challenge", &pkce_challenge(&verifier))
        .append_pair("code_challenge_method", "S256");
    let cookie = Cookie::build((login_cookie_name(&state), login_state))
        .http_only(true)
        .secure(state.config.public_url.starts_with("https://"))
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time::Duration::minutes(10))
        .build();
    Ok((jar.add(cookie), Redirect::to(url.as_str())))
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

/// `GET /api/auth/callback` — IdP trả code: đổi, xác minh, ghép user, tạo phiên.
pub async fn callback(State(state): State<AppState>, jar: CookieJar, Query(q): Query<CallbackQuery>) -> Response {
    let name = login_cookie_name(&state);
    let expected = jar.get(name).map(|c| c.value().to_string());
    let jar = jar.remove(Cookie::build(name).path("/").build());
    if let Some(error) = q.error {
        let code = if error == "access_denied" { "denied" } else { "idp" };
        return (jar, login_failed(&state, code)).into_response();
    }
    let (Some(code), Some(login_state)) = (q.code, q.state) else {
        return (jar, login_failed(&state, "invalid")).into_response();
    };
    // Lượt phải do chính trình duyệt này bắt đầu (chống CSRF đăng nhập).
    if expected.as_deref() != Some(login_state.as_str()) {
        return (jar, login_failed(&state, "expired")).into_response();
    }
    match finish_web_login(&state, &code, &login_state).await {
        Ok((token, max_age)) => {
            let jar = jar.add(session_cookie(&state, token, max_age));
            (jar, Redirect::to(&format!("{}/", state.config.public_url))).into_response()
        }
        Err(AppError::Forbidden(m)) => {
            let code = if m.contains("xác minh") { "unverified" } else { "forbidden" };
            (jar, login_failed(&state, code)).into_response()
        }
        Err(AppError::Unavailable(_)) => (jar, login_failed(&state, "idp")).into_response(),
        Err(e) => {
            tracing::warn!("đăng nhập web lỗi: {e:?}");
            (jar, login_failed(&state, "failed")).into_response()
        }
    }
}

async fn finish_web_login(state: &AppState, code: &str, login_state: &str) -> Result<(String, i64), AppError> {
    let saved = sessions::take_state(&state.db, login_state)
        .await?
        .ok_or_else(|| AppError::Unauthorized("lượt đăng nhập đã hết hạn".into()))?;
    let client_id = &state.config.oidc_web_client_id;
    take_idp_budget(state)?;
    let tokens = state
        .oidc
        .exchange_code(&state.http, client_id, Some(&state.oidc_web_secret), code, &saved.code_verifier, &callback_url(state))
        .await
        .map_err(grant_error)?;
    let claims = state.oidc.verify_id_token(&state.http, &tokens.id_token, client_id, Some(&saved.nonce)).await?;
    let user_id = admit(state, &claims).await?;
    let (token, _) = sessions::create(&state.db, &user_id, Client::Web, claims.sid.as_deref(), None).await?;
    audit::record(state, Some(&user_id), "auth.login", doc! { "client": "web" });
    // Cookie sống tới trần trượt 7 ngày; hạn thật do server giữ (`sessions.expiresAt`, trượt 12 giờ).
    Ok((token, sessions::WEB_MAX_AGE_S))
}

/// Chi tiết lỗi của IdP chỉ ghi log; người dùng nhận câu ngắn.
fn grant_error(e: GrantError) -> AppError {
    match e {
        GrantError::Rejected(m) => {
            tracing::info!("IdP từ chối grant: {m}");
            AppError::Unauthorized("đăng nhập không còn hiệu lực, đăng nhập lại".into())
        }
        GrantError::Unavailable(m) => {
            tracing::warn!("IdP lỗi: {m}");
            AppError::Unavailable("máy chủ đăng nhập không phản hồi, thử lại sau".into())
        }
    }
}

// MARK: iPhone

/// `GET /api/auth/native/config` — tham số để app tự dựng yêu cầu đăng nhập (PKCE do app sinh và giữ).
pub async fn native_config(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    let discovery = state.oidc.discovery(&state.http).await?;
    Ok(Json(json!({
        "authorizationEndpoint": discovery.authorization_endpoint,
        "clientId": state.config.oidc_mobile_client_id,
        "redirectUri": state.config.oidc_mobile_redirect_uri,
        "scope": SCOPE,
    })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeExchange {
    code: String,
    code_verifier: String,
    nonce: String,
}

/// `POST /api/auth/native/exchange` — đổi code của app (client public, redirect cố định trong cấu hình).
pub async fn native_exchange(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<NativeExchange>,
) -> Result<(CookieJar, Json<Value>), AppError> {
    if body.code_verifier.len() < 43 || body.nonce.len() < 16 {
        return Err(AppError::BadRequest("thiếu PKCE hoặc nonce".into()));
    }
    let client_id = &state.config.oidc_mobile_client_id;
    take_idp_budget(&state)?;
    let tokens = state
        .oidc
        .exchange_code(&state.http, client_id, None, &body.code, &body.code_verifier, &state.config.oidc_mobile_redirect_uri)
        .await
        .map_err(grant_error)?;
    let claims = state.oidc.verify_id_token(&state.http, &tokens.id_token, client_id, Some(&body.nonce)).await?;
    let refresh = tokens
        .refresh_token
        .ok_or_else(|| AppError::Internal("IdP không cấp refresh token cho app".into()))?;
    let user_id = admit(&state, &claims).await?;
    let (device_token, device_id) = sessions::create_device(&state.db, &user_id, &seal(&state.session_key, &refresh)).await?;
    let (token, _) = sessions::create(&state.db, &user_id, Client::Mobile, None, Some(&device_id)).await?;
    audit::record(&state, Some(&user_id), "auth.login", doc! { "client": "mobile" });
    let email = claims.email.unwrap_or_default();
    Ok((
        jar.add(session_cookie(&state, token, sessions::MOBILE_TTL_S)),
        Json(json!({ "email": email, "deviceToken": device_token })),
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceBody {
    device_token: String,
}

/// `POST /api/auth/native/refresh` — phiên mới cho thiết bị khi IdP còn công nhận máy và tài khoản vẫn
/// được phép. Mỗi thiết bị chỉ một lần làm mới chạy cùng lúc (refresh token IdP dùng một lần): lần trùng
/// nhận 409 và app chờ rồi dùng cookie lần kia cấp; thiết bị không còn thì 401.
pub async fn native_refresh(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<DeviceBody>,
) -> Result<(CookieJar, Json<Value>), AppError> {
    let device = match sessions::lock_device(&state.db, &body.device_token).await? {
        sessions::DeviceLock::Locked(device) => device,
        sessions::DeviceLock::Busy => return Err(AppError::Conflict("đang làm mới phiên, thử lại sau giây lát".into())),
        sessions::DeviceLock::Missing => {
            return Err(AppError::Unauthorized("máy đã bị đăng xuất, đăng nhập lại".into()));
        }
    };
    let revoke = |state: &AppState, device: &sessions::Device| {
        let (state, device_id) = (state.clone(), device.id.clone());
        async move {
            let removed = sessions::delete_device(&state.db, &device_id).await?;
            state.session_cache.forget(&removed);
            Ok::<(), AppError>(())
        }
    };
    let Some(refresh) = open(&state.session_key, &device.refresh_sealed) else {
        revoke(&state, &device).await?;
        return Err(AppError::Unauthorized("phiên của máy không còn dùng được, đăng nhập lại".into()));
    };
    if let Err(e) = take_idp_budget(&state) {
        sessions::unlock_device(&state.db, &device, None).await?;
        return Err(e);
    }
    let client_id = &state.config.oidc_mobile_client_id;
    let tokens = match state.oidc.refresh(&state.http, client_id, &refresh).await {
        Ok(tokens) => tokens,
        Err(GrantError::Rejected(reason)) => {
            // Máy bị thu hồi ở /account hoặc grant hết hạn: bỏ thiết bị, app phải đăng nhập lại.
            revoke(&state, &device).await?;
            audit::record(&state, Some(&device.user_id), "auth.device_revoked", doc! { "reason": reason });
            return Err(AppError::Unauthorized("máy đã bị đăng xuất, đăng nhập lại".into()));
        }
        Err(e) => {
            sessions::unlock_device(&state.db, &device, None).await?;
            return Err(grant_error(e));
        }
    };
    // Ghi token mới TRƯỚC mọi kiểm tra khác: token cũ đã chết ở IdP.
    let sealed = tokens.refresh_token.as_deref().map(|r| seal(&state.session_key, r));
    sessions::unlock_device(&state.db, &device, sealed.as_deref()).await?;

    // Tài khoản vẫn phải là của máy này và vẫn được phép (allowlist có thể đã đổi).
    let claims = state.oidc.verify_id_token(&state.http, &tokens.id_token, client_id, None).await?;
    let owner = user::find_id_by_sub(&state.db, &claims.sub).await?;
    if owner.as_deref() != Some(device.user_id.as_str()) || admit(&state, &claims).await.is_err() {
        revoke(&state, &device).await?;
        audit::record(&state, Some(&device.user_id), "auth.device_revoked", doc! { "reason": "not_allowed" });
        return Err(AppError::Unauthorized("tài khoản không còn được phép, đăng nhập lại".into()));
    }
    let (token, _) = sessions::create(&state.db, &device.user_id, Client::Mobile, None, Some(&device.id)).await?;
    Ok((jar.add(session_cookie(&state, token, sessions::MOBILE_TTL_S)), Json(json!({ "ok": true }))))
}

// MARK: Đăng xuất

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LogoutBody {
    #[serde(default)]
    device_token: Option<String>,
}

/// `POST /api/auth/logout` — xoá phiên (và thiết bị nếu app gửi kèm). Web nhận URL đăng xuất ở IdP.
pub async fn logout(State(state): State<AppState>, jar: CookieJar, body: Option<Json<LogoutBody>>) -> (CookieJar, Json<Value>) {
    let name = cookie_name(&state);
    let mut user_id = None;
    if let Some(token) = jar.get(name).map(|c| c.value().to_string()) {
        if let Ok(Some(s)) = sessions::find(&state.db, &token).await {
            user_id = Some(s.user_id);
        }
        if let Ok(id) = sessions::delete(&state.db, &token).await {
            state.session_cache.forget(&[id]);
        }
    }
    if let Some(device_token) = body.and_then(|Json(b)| b.device_token) {
        let device_id = crate::session::hash_token(&device_token);
        // Thu hồi grant ở IdP để máy không còn treo ở /account.
        if let Ok(Some(sealed)) = sessions::device_refresh(&state.db, &device_id).await {
            if let Some(refresh) = open(&state.session_key, &sealed) {
                state.oidc.revoke(&state.http, &state.config.oidc_mobile_client_id, &refresh).await;
            }
        }
        if let Ok(removed) = sessions::delete_device(&state.db, &device_id).await {
            state.session_cache.forget(&removed);
        }
    }
    audit::record(&state, user_id.as_deref(), "auth.logout", doc! {});
    let end_session = state.oidc.discovery(&state.http).await.ok().and_then(|d| d.end_session_endpoint.clone()).map(|end| {
        let mut url = reqwest::Url::parse(&end).ok();
        if let Some(u) = url.as_mut() {
            u.query_pairs_mut()
                .append_pair("client_id", &state.config.oidc_web_client_id)
                .append_pair("post_logout_redirect_uri", &format!("{}/", state.config.public_url));
        }
        url.map(|u| u.to_string())
    });
    let jar = jar.remove(Cookie::build(name).path("/").build());
    (jar, Json(json!({ "ok": true, "endSessionUrl": end_session.flatten() })))
}

#[derive(Deserialize)]
pub struct BackchannelForm {
    logout_token: String,
}

/// `POST /api/auth/backchannel-logout` — IdP báo phiên ở IdP đã kết thúc: xoá phiên web tương ứng.
pub async fn backchannel_logout(State(state): State<AppState>, Form(form): Form<BackchannelForm>) -> Response {
    let claims =
        match state.oidc.verify_logout_token(&state.http, &form.logout_token, &state.config.oidc_web_client_id).await {
            Ok(c) => c,
            Err(_) => return (axum::http::StatusCode::BAD_REQUEST, "invalid logout token").into_response(),
        };
    match sessions::seen_jti(&state.db, &claims.jti).await {
        Ok(true) => return no_store_ok(),
        Ok(false) => {}
        Err(e) => {
            tracing::error!("back-channel logout lỗi: {e:?}");
            return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "error").into_response();
        }
    }
    let removed = match (&claims.sid, &claims.sub) {
        (Some(sid), _) => sessions::delete_web_by_idp_sid(&state.db, sid).await,
        (None, Some(sub)) => match user::find_id_by_sub(&state.db, sub).await {
            Ok(Some(user_id)) => sessions::delete_by_user(&state.db, &user_id, Some(Client::Web)).await,
            Ok(None) => Ok(Vec::new()),
            Err(e) => Err(e),
        },
        (None, None) => Ok(Vec::new()),
    };
    match removed {
        Ok(ids) => {
            state.session_cache.forget(&ids);
            if let Err(e) = sessions::record_jti(&state.db, &claims.jti).await {
                tracing::warn!("ghi jti lỗi (phiên đã xoá): {e:?}");
            }
            audit::record(&state, None, "auth.backchannel_logout", doc! { "sessions": ids.len() as i64 });
            no_store_ok()
        }
        Err(e) => {
            tracing::error!("back-channel logout lỗi: {e:?}");
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "error").into_response()
        }
    }
}

fn no_store_ok() -> Response {
    let mut res = axum::http::StatusCode::OK.into_response();
    res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

// MARK: Phiên trên mỗi request

/// `GET /api/me` — trả thông tin user hiện tại (yêu cầu đã qua middleware auth).
pub async fn me(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
) -> Result<Json<Value>, AppError> {
    let profile = user::find_profile(&state.db, &user.0)
        .await?
        .ok_or_else(|| AppError::Unauthorized("user không tồn tại".into()))?;
    let can_transcribe = state.config.transcribe_allowed(&profile.email);
    let transcription = if can_transcribe {
        let now = chrono::Utc::now();
        let month = usage::month_key(now);
        let used = usage::month_usage(&state.db, &user.0, &month).await?;
        let plan = plan::effective(profile.plan.as_deref(), profile.plan_until, mongodb::bson::DateTime::now());
        let limits = state.config.transcribe_limits;
        let pool_full = plan == plan::Plan::Free && used.free_pool_seconds >= limits.free_pool_seconds;
        json!({
            "plan": plan.name(),
            "planUntil": profile.plan_until.and_then(|d| d.try_to_rfc3339_string().ok()),
            "usedMinutes": usage::used_minutes(used.user_seconds),
            "limitMinutes": plan.limit_seconds(&limits) / 60,
            "resetsAt": usage::next_reset(now).to_rfc3339(),
            "capReached": pool_full || used.total_seconds >= limits.monthly_cap_seconds,
            "paused": !state.transcriber.ready(),
        })
    } else {
        Value::Null
    };
    Ok(Json(json!({ "email": profile.email, "canTranscribe": can_transcribe, "transcription": transcription })))
}

/// Middleware: cookie phiên → phiên còn hạn trong Mongo (cache 30 giây) → gắn `UserId`. Thiếu/sai → 401.
pub async fn require_auth(
    State(state): State<AppState>,
    jar: CookieJar,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = jar
        .get(cookie_name(&state))
        .map(|c| c.value().to_string())
        .ok_or_else(|| AppError::Unauthorized("chưa đăng nhập".into()))?;
    let id = crate::session::hash_token(&token);
    let user_id = match state.session_cache.get(&id) {
        Some(user_id) => user_id,
        None => {
            let session = sessions::find(&state.db, &token)
                .await?
                .ok_or_else(|| AppError::Unauthorized("phiên đã hết hạn, đăng nhập lại".into()))?;
            // Allowlist có thể đã đổi từ lúc đăng nhập (build mới), tài khoản có thể vừa bị khoá: kiểm lại
            // mỗi lần cache hết hạn.
            let profile = user::find_profile(&state.db, &session.user_id).await?;
            let allowed = profile.is_some_and(|p| !p.blocked && state.config.email_allowed(&p.email));
            if !allowed {
                let removed = sessions::delete_by_user(&state.db, &session.user_id, None).await?;
                state.session_cache.forget(&removed);
                return Err(AppError::Unauthorized("tài khoản không còn được phép".into()));
            }
            state.session_cache.put(session.id, session.user_id.clone());
            session.user_id
        }
    };
    req.extensions_mut().insert(UserId(user_id.clone()));
    let mut res = next.run(req).await;
    // Cho access log (lớp ngoài) biết request thuộc user nào.
    res.extensions_mut().insert(UserId(user_id));
    Ok(res)
}
