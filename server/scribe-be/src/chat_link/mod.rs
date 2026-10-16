//! Liên kết chat.hoandat.com: người dùng mở deep link `https://chat.hoandat.com/b/<bot>?start=<token>`,
//! bấm "Gửi /start", task long-poll ([`poll`]) nhận `/start <token>`, bot hỏi lại kèm tài khoản Scribe
//! (email che bớt) và nút "Xác nhận liên kết"; bấm nút mới gắn chat riêng đó với tài khoản ([`flow`]).
//! Biên bản chỉ gửi tới chat riêng đó (`summary::share`). Không thêm endpoint public nào.
//!
//! Liên kết chỉ bật khi task poll chạy (`CHAT_LINK_POLL` và có token bot): không poll thì không ai đọc
//! `/start`, nên `POST /api/chat/link` trả 503 `CHAT_LINK_DISABLED` và `GET` báo `enabled: false`.

pub mod bot;
pub mod flow;
pub mod poll;

use crate::audit;
use crate::auth::UserId;
use crate::error::AppError;
use crate::models::chat_link;
use crate::notify;
use crate::state::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::{Extension, Json};
use bot::BotApi;
use mongodb::bson::doc;
use serde_json::{json, Value};
use std::sync::OnceLock;
use std::time::Duration;

/// Một người tạo tối đa chừng này đường dẫn liên kết mỗi giờ.
const LINKS_PER_HOUR: u32 = 20;

/// Username của bot: biến build `CHAT_BOT_USERNAME`, không có thì hỏi `/bot/v1/me` một lần rồi nhớ.
#[derive(Default)]
pub struct BotProfile {
    username: OnceLock<String>,
}

/// Username dùng được trong đường dẫn (chữ, số, `_`).
fn valid_username(name: &str) -> bool {
    !name.is_empty() && name.len() <= 64 && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

impl BotProfile {
    async fn username(&self, state: &AppState) -> Result<String, AppError> {
        if let Some(name) = state.config.chat_bot_username.clone().or_else(|| self.username.get().cloned()) {
            return Ok(name);
        }
        let token = notify::read_bot_token(&state.config).await.map_err(|e| {
            tracing::warn!("liên kết chat: {e}");
            AppError::Unavailable("Liên kết chat chưa sẵn sàng.".into())
        })?;
        let name = BotApi::new(&state.http, &state.config.notify_bot_api, &token).username().await.map_err(|e| {
            tracing::warn!("liên kết chat: hỏi username bot lỗi: {e}");
            AppError::Unavailable("Không liên lạc được chat.hoandat.com, thử lại sau.".into())
        })?;
        if !valid_username(&name) {
            tracing::error!("liên kết chat: username bot không dùng được trong đường dẫn");
            return Err(AppError::Unavailable("Liên kết chat chưa sẵn sàng.".into()));
        }
        Ok(self.username.get_or_init(|| name).clone())
    }
}

/// Lúc khởi động: hỏi trước username bot (nền, lỗi chỉ log) để lần liên kết đầu không phải chờ.
pub fn warm_up(state: AppState) {
    if state.config.chat_bot_username.is_some() || state.config.notify_bot_token_path.is_none() {
        return;
    }
    tokio::spawn(async move {
        if state.chat_bot.username(&state).await.is_ok() {
            tracing::info!("liên kết chat: đã đọc username bot");
        }
    });
}

/// Origin của chat từ gốc Bot API (`https://chat.hoandat.com/bot/v1` → `https://chat.hoandat.com`).
fn chat_origin(bot_api: &str) -> &str {
    bot_api.trim_end_matches('/').trim_end_matches("/bot/v1")
}

/// Deep link mở hộp "Gửi /start" của bot với token.
pub fn deep_link(bot_api: &str, username: &str, token: &str) -> String {
    format!("{}/b/{username}?start={token}", chat_origin(bot_api))
}

/// Máy chủ này có nhận liên kết mới không (task poll có chạy).
fn link_enabled(config: &crate::config::Config) -> bool {
    config.chat_link_poll && config.notify_bot_token_path.is_some()
}

/// Body của `GET /api/chat/link`.
fn status_json(linked: Option<&chat_link::Linked>, enabled: bool) -> Value {
    json!({
        "linked": linked.is_some(),
        "linkedAt": linked.and_then(|l| l.linked_at).and_then(|at| at.try_to_rfc3339_string().ok()),
        "enabled": enabled,
    })
}

/// `GET /api/chat/link` — `{linked, linkedAt, enabled}`.
pub async fn status(State(state): State<AppState>, Extension(user): Extension<UserId>) -> Result<Json<Value>, AppError> {
    let linked = chat_link::linked(&state.db, &user.0).await?;
    Ok(Json(status_json(linked.as_ref(), link_enabled(&state.config))))
}

/// `POST /api/chat/link` — token một lần (hạn 10 phút) và đường dẫn để mở trên chat.hoandat.com.
pub async fn create(State(state): State<AppState>, Extension(user): Extension<UserId>) -> Result<Json<Value>, AppError> {
    if !link_enabled(&state.config) {
        return Err(AppError::Coded {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "CHAT_LINK_DISABLED",
            message: "Liên kết chat chưa được bật trên máy chủ này.".into(),
        });
    }
    if !state.rate.allow_in(&format!("chat-link:{}", user.0), LINKS_PER_HOUR, Duration::from_secs(3600)) {
        return Err(AppError::TooManyRequests("tạo đường dẫn liên kết quá nhiều lần, thử lại sau".into()));
    }
    let username = state.chat_bot.username(&state).await?;
    let token = chat_link::create_token(&state.db, &user.0).await?;
    let expires = chrono::Utc::now() + chrono::Duration::minutes(chat_link::TOKEN_TTL_MINUTES);
    Ok(Json(json!({
        "url": deep_link(&state.config.notify_bot_api, &username, &token),
        "expiresAt": expires.to_rfc3339(),
    })))
}

/// `DELETE /api/chat/link` — gỡ liên kết.
pub async fn remove(State(state): State<AppState>, Extension(user): Extension<UserId>) -> Result<Json<Value>, AppError> {
    if chat_link::unlink(&state.db, &user.0, None).await? {
        audit::record(&state, Some(&user.0), "chat.unlink", doc! { "reason": "user" });
    }
    Ok(Json(json!({ "linked": false })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_links_point_at_the_chat_host() {
        assert_eq!(
            deep_link("https://chat.hoandat.com/bot/v1", "scribe_bot", "tok-1"),
            "https://chat.hoandat.com/b/scribe_bot?start=tok-1"
        );
        assert_eq!(deep_link("http://localhost:8080/bot/v1/", "b", "t"), "http://localhost:8080/b/b?start=t");
        assert!(valid_username("Scribe_bot1"));
        assert!(!valid_username("bad/name") && !valid_username(""));
    }

    #[test]
    fn status_reports_link_time_and_whether_linking_is_on() {
        assert_eq!(status_json(None, false), json!({ "linked": false, "linkedAt": null, "enabled": false }));
        let linked = chat_link::Linked { chat_id: 12, linked_at: Some(mongodb::bson::DateTime::from_millis(0)) };
        assert_eq!(status_json(Some(&linked), true), json!({ "linked": true, "linkedAt": "1970-01-01T00:00:00Z", "enabled": true }));
    }
}
