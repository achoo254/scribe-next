//! Auth Google qua Firebase: verify Firebase ID token → session cookie httpOnly của BE.
//!
//! FE (Firebase JS SDK) `signInWithPopup` → gửi ID token lên `POST /api/auth/session`. BE verify
//! (RS256 securetoken JWKS + aud/iss/exp + email_verified), allowlist, upsert user, cấp session.

use crate::audit;
use crate::error::AppError;
use crate::models::user;
use crate::session;
use crate::state::AppState;
use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;
use axum::{Extension, Json};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::Deserialize;
use mongodb::bson::doc;
use serde_json::{json, Value};

const SESSION_COOKIE: &str = "session";

/// User id gắn vào request sau khi middleware verify session.
#[derive(Clone, Debug)]
pub struct UserId(pub String);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRequest {
    id_token: String,
}

/// `POST /api/auth/session` — đổi Firebase ID token lấy session cookie httpOnly.
pub async fn create_session(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<SessionRequest>,
) -> Result<(CookieJar, Json<Value>), AppError> {
    let claims = state.jwks.verify_id_token(&state.http, &body.id_token).await?;

    // Allowlist chỉ so khớp SAU khi đã xác nhận email_verified (đã check trong verify_id_token).
    if !state.config.email_allowed(&claims.email) {
        tracing::warn!(email = %claims.email, "login từ chối bởi allowlist");
        audit::record(&state, None, "auth.login_denied", doc! { "email": &claims.email });
        return Err(AppError::Forbidden("email không có quyền truy cập".into()));
    }
    // Audit-log mọi email đăng nhập (chế độ mở — user chốt cho mọi tài khoản Google).
    tracing::info!(email = %claims.email, "login thành công");

    let user_id =
        user::upsert_and_get_id(&state.db, &claims.sub, &claims.email, claims.name.as_deref())
            .await?;
    audit::record(&state, Some(&user_id), "auth.login", doc! { "email": &claims.email });
    let token = session::issue(&state.session_key, &user_id, state.config.session_ttl_secs)?;
    let cookie = build_session_cookie(token, state.config.session_ttl_secs);
    Ok((jar.add(cookie), Json(json!({ "email": claims.email }))))
}

/// `POST /api/auth/logout` — xoá session cookie (FE gọi kèm Firebase `signOut()`).
pub async fn logout(State(state): State<AppState>, jar: CookieJar) -> (CookieJar, Json<Value>) {
    // Ai đăng xuất: đọc session nếu còn hợp lệ (cookie hết hạn thì vẫn xoá, chỉ không biết user).
    let user_id = jar
        .get(SESSION_COOKIE)
        .and_then(|c| session::verify(&state.session_key, c.value()).ok());
    audit::record(&state, user_id.as_deref(), "auth.logout", doc! {});
    let removal = Cookie::build((SESSION_COOKIE, "")).path("/").build();
    (jar.remove(removal), Json(json!({ "ok": true })))
}

/// `GET /api/me` — trả thông tin user hiện tại (yêu cầu đã qua middleware auth).
pub async fn me(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
) -> Result<Json<Value>, AppError> {
    let email = user::find_email_by_id(&state.db, &user.0)
        .await?
        .ok_or_else(|| AppError::Unauthorized("user không tồn tại".into()))?;
    Ok(Json(json!({ "email": email })))
}

/// Middleware guard: verify session cookie → gắn `UserId` vào request. Thiếu/sai → 401.
pub async fn require_auth(
    State(state): State<AppState>,
    jar: CookieJar,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = jar
        .get(SESSION_COOKIE)
        .map(|c| c.value().to_string())
        .ok_or_else(|| AppError::Unauthorized("chưa đăng nhập".into()))?;
    let user_id = session::verify(&state.session_key, &token)?;
    req.extensions_mut().insert(UserId(user_id.clone()));
    let mut res = next.run(req).await;
    // Cho access log (lớp ngoài) biết request thuộc user nào.
    res.extensions_mut().insert(UserId(user_id));
    Ok(res)
}

/// Cookie session: HttpOnly + Secure + SameSite=Lax (cùng origin qua nginx). TTL khớp JWT exp.
fn build_session_cookie(value: String, ttl_secs: i64) -> Cookie<'static> {
    Cookie::build((SESSION_COOKIE, value))
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time::Duration::seconds(ttl_secs))
        .build()
}
