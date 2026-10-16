//! Task nền long-poll `GET {NOTIFY_BOT_API}/updates`: nhận `/start <token>` từ deep link để gắn chat
//! riêng của người gửi với tài khoản Scribe đã tạo token.
//!
//! Chỉ chạy khi bật `CHAT_LINK_POLL` và có token bot: dev và prod dùng chung token, mà mỗi bot chỉ có một
//! poll (poll mới làm poll cũ nhận 409), nên backend dev mặc định không poll. Offset lưu trong Mongo (`kv`)
//! sau mỗi update: khởi động lại không xử lý lại; update có `update_id` dưới offset bị bỏ (bỏ trùng).

use super::bot::{BotApi, BotError, Update};
use crate::audit;
use crate::models::{chat_link, user};
use crate::notify;
use crate::state::AppState;
use mongodb::bson::doc;
use std::time::Duration;

const WAIT_S: u64 = 50;

const LINKED: &str = "Đã liên kết với Scribe. Biên bản bạn gửi từ Scribe sẽ tới chat này.";
const GUIDE: &str = "Liên kết không hợp lệ hoặc đã hết hạn. Mở Scribe, chọn \"Liên kết chat\" rồi bấm lại đường dẫn mới (đường dẫn dùng được 10 phút).";
const BLOCKED: &str = "Tài khoản Scribe này đang bị khoá nên không liên kết được.";
const LATER: &str = "Scribe đang bận, thử lại đường dẫn sau ít phút.";

/// `/start` trong tin nhắn.
#[derive(Debug, PartialEq)]
pub enum Start<'a> {
    /// `/start` không kèm tham số.
    Bare,
    Token(&'a str),
}

/// Đọc lệnh `/start [tham số]` (cả dạng `/start@ten_bot`). Tham số theo luật deep link: 1–64 ký tự
/// `A-Z a-z 0-9 _ -`; sai dạng thì coi như không có token hợp lệ (`Bare`). Tin khác lệnh → `None`.
pub fn parse_start(text: &str) -> Option<Start<'_>> {
    let rest = text.trim().strip_prefix("/start")?;
    let rest = match rest.strip_prefix('@') {
        Some(addressed) => addressed.split_once(char::is_whitespace).map_or("", |(_, r)| r),
        None if rest.is_empty() || rest.starts_with(char::is_whitespace) => rest,
        None => return None, // "/startx"
    };
    let param = rest.trim();
    let valid = (1..=64).contains(&param.len()) && param.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    Some(if valid { Start::Token(param) } else { Start::Bare })
}

/// Chờ trước lần thử lại thứ `failures` sau lỗi mạng: 1, 2, 4 … tối đa 60 giây.
pub fn backoff_secs(failures: u32) -> u64 {
    2u64.pow(failures.saturating_sub(1).min(6)).min(60)
}

/// Bật task nếu cấu hình cho phép (không chặn khởi động).
pub fn start(state: AppState) {
    if !state.config.chat_link_poll {
        tracing::info!("liên kết chat: không poll bot (CHAT_LINK_POLL tắt)");
        return;
    }
    if state.config.notify_bot_token_path.is_none() {
        tracing::warn!("liên kết chat: CHAT_LINK_POLL bật nhưng thiếu NOTIFY_BOT_TOKEN_PATH, không poll");
        return;
    }
    // Panic khi xử lý một update không được giết hẳn việc liên kết: chạy lại sau 30 giây.
    tokio::spawn(async move {
        loop {
            match tokio::spawn(run(state.clone())).await {
                Ok(()) => return,
                Err(e) => {
                    tracing::error!("liên kết chat: task poll panic, chạy lại sau 30 giây: {e}");
                    tokio::time::sleep(Duration::from_secs(30)).await;
                }
            }
        }
    });
}

/// Chỉ trả về khi không đọc được token (cấu hình sai): khởi động lại cũng vô ích.
async fn run(state: AppState) {
    let token = match notify::read_bot_token(&state.config).await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("liên kết chat: {e}, không poll");
            return;
        }
    };
    let api = BotApi::new(&state.http, &state.config.notify_bot_api, &token);
    let mut offset = loop {
        match chat_link::load_offset(&state.db).await {
            Ok(o) => break o,
            Err(e) => {
                tracing::warn!("liên kết chat: đọc offset lỗi, thử lại: {e:?}");
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
        }
    };
    tracing::info!(offset, "liên kết chat: bắt đầu long-poll bot");
    let mut failures = 0;
    loop {
        match api.updates(offset, WAIT_S).await {
            Ok(updates) => {
                failures = 0;
                for update in updates {
                    if update.update_id < offset {
                        continue; // đã xử lý (giao lại)
                    }
                    handle(&state, &api, &update).await;
                    offset = update.update_id + 1;
                    if let Err(e) = chat_link::save_offset(&state.db, offset).await {
                        tracing::warn!(offset, "liên kết chat: lưu offset lỗi: {e:?}");
                    }
                }
            }
            Err(BotError::Conflict) => {
                tracing::warn!("liên kết chat: bot API trả 409 (đang đặt webhook hoặc một backend khác đang poll), chờ 60 giây");
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
            Err(BotError::Unauthorized) => {
                tracing::error!("liên kết chat: token bot bị từ chối, chờ 5 phút");
                tokio::time::sleep(Duration::from_secs(300)).await;
            }
            Err(BotError::RetryAfter(s)) => tokio::time::sleep(Duration::from_secs(s)).await,
            Err(e) => {
                failures += 1;
                let wait = backoff_secs(failures);
                tracing::warn!(failures, wait, "liên kết chat: {e}");
                tokio::time::sleep(Duration::from_secs(wait)).await;
            }
        }
    }
}

/// Một update: chỉ `/start` trong chat riêng từ người thật. Lỗi chỉ log (offset vẫn tiến: token đã bị
/// dùng hoặc không hợp lệ, xử lý lại cũng không đổi kết quả).
async fn handle(state: &AppState, api: &BotApi<'_>, update: &Update) {
    let Some(msg) = &update.message else { return };
    if msg.chat.kind != "private" || msg.from.as_ref().is_none_or(|f| f.is_bot) {
        return;
    }
    let Some(start) = msg.text.as_deref().and_then(parse_start) else { return };
    let reply = match start {
        Start::Bare => GUIDE,
        Start::Token(token) => link(state, token, msg.chat.id).await,
    };
    if let Err(e) = api.send(msg.chat.id, reply, false).await {
        tracing::warn!(chat = msg.chat.id, "liên kết chat: trả lời /start lỗi: {e}");
    }
}

async fn link(state: &AppState, token: &str, chat_id: i64) -> &'static str {
    let user_id = match chat_link::consume_token(&state.db, token).await {
        Ok(Some(u)) => u,
        Ok(None) => return GUIDE,
        Err(e) => {
            tracing::warn!("liên kết chat: dùng token lỗi: {e:?}");
            return LATER;
        }
    };
    match user::find_profile(&state.db, &user_id).await {
        Ok(Some(p)) if p.blocked => return BLOCKED,
        Ok(Some(_)) => {}
        Ok(None) => return GUIDE,
        Err(e) => {
            tracing::warn!("liên kết chat: đọc user lỗi: {e:?}");
            return LATER;
        }
    }
    match chat_link::link(&state.db, &user_id, chat_id).await {
        Ok(true) => {
            audit::record(state, Some(&user_id), "chat.link", doc! { "chatId": chat_id });
            LINKED
        }
        Ok(false) => GUIDE,
        Err(e) => {
            tracing::warn!("liên kết chat: ghi liên kết lỗi: {e:?}");
            LATER
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_commands_are_parsed_strictly() {
        let token = "Ab_9-".repeat(8);
        assert_eq!(parse_start(&format!("/start {token}")), Some(Start::Token(&token)));
        assert_eq!(parse_start(&format!("  /start   {token}  ")), Some(Start::Token(&token)));
        assert_eq!(parse_start(&format!("/start@scribe_bot {token}")), Some(Start::Token(&token)));
        assert_eq!(parse_start("/start"), Some(Start::Bare));
        assert_eq!(parse_start("/start@scribe_bot"), Some(Start::Bare));
        assert_eq!(parse_start("/start có dấu"), Some(Start::Bare));
        assert_eq!(parse_start(&format!("/start {}", "a".repeat(65))), Some(Start::Bare));
        assert_eq!(parse_start("/startx abc"), None);
        assert_eq!(parse_start("xin chào"), None);
    }

    #[test]
    fn network_backoff_grows_to_a_minute() {
        let waits: Vec<u64> = (1..=9).map(backoff_secs).collect();
        assert_eq!(waits, vec![1, 2, 4, 8, 16, 32, 60, 60, 60]);
    }
}
