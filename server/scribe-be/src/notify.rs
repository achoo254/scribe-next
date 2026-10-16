//! Báo chủ sản phẩm qua bot của chat.hoandat.com (quỹ free sắp cạn, trần tổng, ai đó liên tục chạm giới
//! hạn ngày, key OpenAI hỏng). Token bot nằm ở tệp `NOTIFY_BOT_TOKEN_PATH` (600), chat id là biến build
//! `NOTIFY_CHAT_ID`; bot chỉ nhắn được người đã bấm "Bắt đầu" với nó. Thiếu cấu hình thì chỉ log. Gửi
//! nền: lỗi không làm hỏng request, và không log token.

use crate::config::Config;
use crate::state::AppState;
use serde_json::json;
use std::time::Duration;

/// Gửi một tin nhắn, chờ kết quả (CLI `admin notify-test` và `send`).
pub async fn deliver(http: &reqwest::Client, config: &Config, text: &str) -> Result<(), String> {
    let (Some(path), Some(chat_id)) = (&config.notify_bot_token_path, &config.notify_chat_id) else {
        return Err("chưa cấu hình NOTIFY_BOT_TOKEN_PATH và NOTIFY_CHAT_ID".into());
    };
    let token = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| format!("không đọc được tệp token bot ({})", e.kind()))?;
    // Chat id là số; giữ dạng chuỗi nếu không đọc được số (API báo lỗi rõ ràng).
    let chat_id = chat_id.parse::<i64>().map(|n| json!(n)).unwrap_or_else(|_| json!(chat_id));
    let res = http
        .post(format!("{}/messages", config.notify_bot_api))
        .bearer_auth(token.trim())
        .json(&json!({ "chat_id": chat_id, "text": format!("[Scribe] {text}") }))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("gửi tin bot lỗi: {}", e.without_url()))?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(format!("bot API trả {}", res.status()))
    }
}

/// Gửi nền; không cấu hình hoặc lỗi thì chỉ log.
pub fn send(state: &AppState, text: String) {
    let (http, config) = (state.http.clone(), state.config.clone());
    tokio::spawn(async move {
        if let Err(e) = deliver(&http, &config, &text).await {
            tracing::warn!(alert = %text, "không gửi được báo động: {e}");
        }
    });
}
