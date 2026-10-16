//! Client tối giản cho Bot API của chat.hoandat.com (`/me`, `/messages`, `/updates`).
//!
//! Mọi câu trả lời là envelope `{"ok": true, "result": …}` hoặc `{"ok": false, "error": {code, description,
//! retry_after}}`; rẽ nhánh theo HTTP status và `error.code`, không theo `description`. Token chỉ đi qua
//! header `Authorization`; lỗi không chứa URL hay token.

use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

/// Lỗi của một lần gọi, đủ để người gọi chọn cách xử lý.
#[derive(Debug, PartialEq)]
pub enum BotError {
    /// 401: token sai hoặc đã bị thu hồi.
    Unauthorized,
    /// 403 (`forbidden`, `no_consent`): người dùng đã chặn bot hoặc chưa bắt đầu chat với bot.
    Forbidden(String),
    /// 409: đang đặt webhook, hoặc một poll mới hơn của cùng bot vừa thay poll này.
    Conflict,
    /// 429 / 503: chờ chừng này giây rồi thử lại.
    RetryAfter(u64),
    /// Lỗi mạng hoặc trả lời không đọc được.
    Network(String),
    /// HTTP khác (vd 400 khi chữ markdown sai).
    Status(u16, String),
}

impl std::fmt::Display for BotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BotError::Unauthorized => write!(f, "bot API: token bị từ chối (401)"),
            BotError::Forbidden(code) => write!(f, "bot API: không được gửi ({code})"),
            BotError::Conflict => write!(f, "bot API: xung đột (409)"),
            BotError::RetryAfter(s) => write!(f, "bot API: bận, thử lại sau {s} giây"),
            BotError::Network(e) => write!(f, "bot API: {e}"),
            BotError::Status(status, code) => write!(f, "bot API trả {status} ({code})"),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct User {
    #[serde(default)]
    pub is_bot: bool,
    #[serde(default)]
    pub username: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Chat {
    pub id: i64,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Debug, Deserialize)]
pub struct Message {
    pub chat: Chat,
    #[serde(default)]
    pub from: Option<User>,
    #[serde(default)]
    pub text: Option<String>,
}

/// Một update; loại khác `message` bị bỏ qua (API hứa chỉ thêm loại mới).
#[derive(Debug, Deserialize)]
pub struct Update {
    pub update_id: i64,
    #[serde(default)]
    pub message: Option<Message>,
}

pub struct BotApi<'a> {
    http: &'a reqwest::Client,
    base: &'a str,
    token: &'a str,
}

/// Envelope → `result`, hoặc lỗi theo status/`error.code`.
pub fn read_envelope(status: u16, body: &str, retry_header: Option<u64>) -> Result<Value, BotError> {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    if (200..300).contains(&status) && v["ok"] == true {
        return Ok(v["result"].clone());
    }
    let code = v["error"]["code"].as_str().unwrap_or("").to_string();
    let retry = v["error"]["retry_after"].as_u64().or(retry_header);
    Err(match status {
        200..=299 => BotError::Network("trả lời không đọc được".into()),
        401 => BotError::Unauthorized,
        403 => BotError::Forbidden(code),
        409 => BotError::Conflict,
        429 | 503 => BotError::RetryAfter(retry.unwrap_or(5).clamp(1, 3600)),
        _ => BotError::Status(status, code),
    })
}

impl<'a> BotApi<'a> {
    pub fn new(http: &'a reqwest::Client, base: &'a str, token: &'a str) -> Self {
        BotApi { http, base, token }
    }

    async fn call(&self, req: reqwest::RequestBuilder, timeout: Duration) -> Result<Value, BotError> {
        let res = req
            .bearer_auth(self.token)
            .timeout(timeout)
            .send()
            .await
            .map_err(|e| BotError::Network(if e.is_timeout() { "quá thời gian chờ".into() } else { "không kết nối được".into() }))?;
        let status = res.status().as_u16();
        let retry = res.headers().get("retry-after").and_then(|h| h.to_str().ok()).and_then(|s| s.parse().ok());
        let body = res.text().await.map_err(|_| BotError::Network("đọc trả lời lỗi".into()))?;
        read_envelope(status, &body, retry)
    }

    /// Username của bot.
    pub async fn username(&self) -> Result<String, BotError> {
        let me = self.call(self.http.get(format!("{}/me", self.base)), Duration::from_secs(10)).await?;
        let user: User = serde_json::from_value(me).map_err(|_| BotError::Network("`/me` sai dạng".into()))?;
        user.username.filter(|u| !u.is_empty()).ok_or_else(|| BotError::Network("bot chưa có username".into()))
    }

    /// Gửi tin; `markdown` = `parse_mode: markdown` (xem `format.md` của Bot API).
    pub async fn send(&self, chat_id: i64, text: &str, markdown: bool) -> Result<(), BotError> {
        let mut body = json!({ "chat_id": chat_id, "text": text, "disable_link_preview": true });
        if markdown {
            body["parse_mode"] = json!("markdown");
        }
        let req = self.http.post(format!("{}/messages", self.base)).json(&body);
        self.call(req, Duration::from_secs(15)).await.map(|_| ())
    }

    /// Long poll: chờ tối đa `wait_s` giây; update có `update_id >= offset`.
    pub async fn updates(&self, offset: i64, wait_s: u64) -> Result<Vec<Update>, BotError> {
        let req = self
            .http
            .get(format!("{}/updates", self.base))
            .query(&[("offset", offset.to_string()), ("timeout", wait_s.to_string())]);
        let result = self.call(req, Duration::from_secs(wait_s + 15)).await?;
        serde_json::from_value(result).map_err(|_| BotError::Network("`/updates` sai dạng".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelopes_map_to_errors_by_status_and_code() {
        assert_eq!(read_envelope(200, r#"{"ok":true,"result":[1]}"#, None), Ok(json!([1])));
        assert_eq!(read_envelope(401, "{}", None), Err(BotError::Unauthorized));
        let blocked = r#"{"ok":false,"error":{"code":"forbidden","description":"blocked"}}"#;
        assert_eq!(read_envelope(403, blocked, None), Err(BotError::Forbidden("forbidden".into())));
        assert_eq!(read_envelope(409, r#"{"ok":false,"error":{"code":"conflict"}}"#, None), Err(BotError::Conflict));
        let limited = r#"{"ok":false,"error":{"code":"rate_limited","retry_after":7}}"#;
        assert_eq!(read_envelope(429, limited, Some(3)), Err(BotError::RetryAfter(7)));
        assert_eq!(read_envelope(503, "", Some(2)), Err(BotError::RetryAfter(2)));
        assert_eq!(read_envelope(503, "", None), Err(BotError::RetryAfter(5)));
        assert_eq!(read_envelope(400, r#"{"ok":false,"error":{"code":"bad_request"}}"#, None), Err(BotError::Status(400, "bad_request".into())));
        assert!(matches!(read_envelope(200, "html", None), Err(BotError::Network(_))));
    }

    #[test]
    fn updates_ignore_unknown_kinds() {
        let raw = json!([
            { "update_id": 5, "message": { "chat": { "id": 12, "type": "private" }, "seq": 1, "date": 0,
              "from": { "id": 3, "is_bot": false, "name": "A" }, "text": "/start abc" } },
            { "update_id": 6, "reaction": { "seq": 1 } },
        ]);
        let updates: Vec<Update> = serde_json::from_value(raw).expect("đọc update");
        assert_eq!(updates.len(), 2);
        let m = updates[0].message.as_ref().expect("message");
        assert_eq!((m.chat.id, m.chat.kind.as_str(), m.text.as_deref()), (12, "private", Some("/start abc")));
        assert!(updates[1].message.is_none());
    }
}
