//! Báo chủ sản phẩm qua Telegram (quỹ free sắp cạn, trần tổng, ai đó liên tục chạm giới hạn ngày, key
//! OpenAI hỏng). Token bot nằm ở tệp `TELEGRAM_TOKEN_PATH` (600), chat id là biến build. Thiếu cấu hình
//! thì chỉ log. Gửi nền: lỗi không làm hỏng request, và không log URL (có token).

use crate::config::Config;
use crate::state::AppState;
use serde_json::json;

/// Gửi một tin nhắn, chờ kết quả (CLI `admin notify-test` và `send`).
pub async fn deliver(http: &reqwest::Client, config: &Config, text: &str) -> Result<(), String> {
    let (Some(path), Some(chat_id)) = (&config.telegram_token_path, &config.telegram_chat_id) else {
        return Err("chưa cấu hình TELEGRAM_TOKEN_PATH và TELEGRAM_CHAT_ID".into());
    };
    let token = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| format!("không đọc được tệp token Telegram ({})", e.kind()))?;
    let url = format!("https://api.telegram.org/bot{}/sendMessage", token.trim());
    let body = json!({ "chat_id": chat_id, "text": format!("[Scribe] {text}"), "disable_web_page_preview": true });
    let res = http
        .post(&url)
        .json(&body)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("gửi Telegram lỗi: {}", e.without_url()))?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(format!("Telegram trả {}", res.status()))
    }
}

/// Gửi nền; không cấu hình hoặc lỗi thì chỉ log.
pub fn send(state: &AppState, text: String) {
    let (http, config) = (state.http.clone(), state.config.clone());
    tokio::spawn(async move {
        if let Err(e) = deliver(&http, &config, &text).await {
            tracing::warn!(alert = %text, "không báo được Telegram: {e}");
        }
    });
}
