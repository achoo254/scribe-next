//! Client tối giản cho Bot API của chat.hoandat.com (`/me`, `/messages`, `/messages:editMarkup`,
//! `/updates`, `/callbacks/{id}:answer`).
//!
//! Mọi câu trả lời là envelope `{"ok": true, "result": …}` hoặc `{"ok": false, "error": {code, description,
//! retry_after}}`; rẽ nhánh theo HTTP status và `error.code`, không theo `description`. Token chỉ đi qua
//! header `Authorization`; lỗi không chứa URL hay token.

use crate::lang::Lang;
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
    /// Tên hiển thị trên chat.
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    /// Ngôn ngữ giao diện chat của người này (`vi` | `en` | `zh-CN`); chat bỏ trống khi chưa biết.
    #[serde(default)]
    pub language_code: Option<String>,
}

impl User {
    /// Ngôn ngữ giao diện chat, khi là mã Scribe biết (mã lạ coi như không biết).
    pub fn lang(&self) -> Option<Lang> {
        self.language_code.as_deref().and_then(Lang::parse)
    }
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
    /// Số thứ tự của tin trong chat (để sửa bàn phím của tin bot đã gửi).
    #[serde(default)]
    pub seq: Option<i64>,
    /// Giây Unix lúc gửi.
    #[serde(default)]
    pub date: Option<i64>,
    #[serde(default)]
    pub from: Option<User>,
    #[serde(default)]
    pub text: Option<String>,
}

/// Người dùng bấm nút inline trên tin của bot.
#[derive(Debug, Deserialize)]
pub struct Callback {
    pub id: String,
    #[serde(default)]
    pub from: Option<User>,
    /// Tin của bot có nút vừa bấm.
    #[serde(default)]
    pub message: Option<Message>,
    #[serde(default)]
    pub data: Option<String>,
}

/// Một update; loại khác `message` và `callback` bị bỏ qua (API hứa chỉ thêm loại mới).
#[derive(Debug, Deserialize)]
pub struct Update {
    pub update_id: i64,
    #[serde(default)]
    pub message: Option<Message>,
    #[serde(default)]
    pub callback: Option<Callback>,
}

/// Loại update task poll cần (đăng ký một lần, server nhớ).
const ALLOWED_UPDATES: &str = "message,callback";

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

/// Body của `POST /messages`.
fn message_body(chat_id: i64, text: &str, markdown: bool, buttons: &[(String, String)]) -> Value {
    let mut body = json!({ "chat_id": chat_id, "text": text, "disable_link_preview": true });
    if markdown {
        body["parse_mode"] = json!("markdown");
    }
    if !buttons.is_empty() {
        let row: Vec<Value> = buttons.iter().map(|(label, data)| json!({ "text": label, "callback_data": data })).collect();
        body["reply_markup"] = json!({ "inline_keyboard": [row] });
    }
    body
}

/// Id lần bấm đặt thẳng vào path: chỉ nhận ký tự an toàn trong URL, tối đa 64 (như spec).
fn valid_callback_id(id: &str) -> bool {
    (1..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.')
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
        self.send_with_buttons(chat_id, text, markdown, &[]).await
    }

    /// Gửi tin kèm một hàng nút inline `(nhãn, callback_data)`; rỗng = không có nút.
    pub async fn send_with_buttons(&self, chat_id: i64, text: &str, markdown: bool, buttons: &[(String, String)]) -> Result<(), BotError> {
        let body = message_body(chat_id, text, markdown, buttons);
        let req = self.http.post(format!("{}/messages", self.base)).json(&body);
        self.call(req, Duration::from_secs(15)).await.map(|_| ())
    }

    /// Gỡ bàn phím inline của một tin bot đã gửi (không gửi `reply_markup` = gỡ).
    pub async fn clear_buttons(&self, chat_id: i64, seq: i64) -> Result<(), BotError> {
        let req = self.http.post(format!("{}/messages:editMarkup", self.base)).json(&json!({ "chat_id": chat_id, "seq": seq }));
        self.call(req, Duration::from_secs(15)).await.map(|_| ())
    }

    /// Trả lời một lần bấm nút (client hiện `text` dạng thông báo ngắn). Id lạ dạng thì bỏ qua.
    pub async fn answer_callback(&self, callback_id: &str, text: &str) -> Result<(), BotError> {
        if !valid_callback_id(callback_id) {
            return Err(BotError::Status(400, "callback_id".into()));
        }
        let req = self
            .http
            .post(format!("{}/callbacks/{callback_id}:answer", self.base))
            .json(&json!({ "text": text.chars().take(200).collect::<String>() }));
        self.call(req, Duration::from_secs(10)).await.map(|_| ())
    }

    /// Long poll: chờ tối đa `wait_s` giây; update có `update_id >= offset`. Trả từng phần tử thô để một
    /// update sai dạng không làm hỏng cả lô (người gọi đọc từng cái).
    pub async fn updates(&self, offset: i64, wait_s: u64) -> Result<Vec<Value>, BotError> {
        let req = self.http.get(format!("{}/updates", self.base)).query(&[
            ("offset", offset.to_string()),
            ("timeout", wait_s.to_string()),
            ("allowed_updates", ALLOWED_UPDATES.to_string()),
        ]);
        let result = self.call(req, Duration::from_secs(wait_s + 15)).await?;
        serde_json::from_value(result).map_err(|_| BotError::Network("`/updates` không phải danh sách".into()))
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
    fn buttons_go_in_one_inline_row() {
        let body = message_body(12, "Xác nhận?", false, &[("Có".into(), "link:t".into()), ("Huỷ".into(), "cancel:t".into())]);
        assert_eq!(body["reply_markup"], json!({ "inline_keyboard": [[
            { "text": "Có", "callback_data": "link:t" },
            { "text": "Huỷ", "callback_data": "cancel:t" },
        ]] }));
        assert!(body.get("parse_mode").is_none());
        let plain = message_body(12, "x", true, &[]);
        assert!(plain.get("reply_markup").is_none() && plain["parse_mode"] == "markdown");
        assert!(valid_callback_id("cb_1-2.x") && !valid_callback_id("a/b") && !valid_callback_id(""));
    }

    #[test]
    fn callbacks_carry_the_pressed_message() {
        let raw = json!({ "update_id": 9, "callback": { "id": "cb1", "data": "link:t",
            "from": { "id": 3, "is_bot": false, "name": "A" },
            "message": { "chat": { "id": 12, "type": "private" }, "seq": 40, "date": 1 } } });
        let u: Update = serde_json::from_value(raw).expect("đọc update");
        let cb = u.callback.expect("callback");
        assert_eq!((cb.id.as_str(), cb.data.as_deref()), ("cb1", Some("link:t")));
        let m = cb.message.expect("tin");
        assert_eq!((m.chat.id, m.seq), (12, Some(40)));
    }

    #[test]
    fn senders_carry_their_chat_language_when_known() {
        let from = |extra: Value| {
            let mut raw = json!({ "id": 3, "is_bot": false, "name": "A" });
            raw.as_object_mut().expect("object").extend(extra.as_object().expect("object").clone());
            serde_json::from_value::<User>(raw).expect("đọc user")
        };
        assert_eq!(from(json!({ "language_code": "zh-CN" })).lang(), Some(Lang::Zh));
        assert_eq!(from(json!({ "language_code": "en" })).lang(), Some(Lang::En));
        assert_eq!(from(json!({ "language_code": "vi" })).lang(), Some(Lang::Vi));
        assert_eq!(from(json!({})).lang(), None, "chat chưa biết thì bỏ trống");
        assert_eq!(from(json!({ "language_code": "fr" })).lang(), None);
        assert_eq!(from(json!({ "language_code": null })).lang(), None);
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
