//! Báo chủ sản phẩm qua bot của chat.hoandat.com (quỹ free sắp cạn, trần tổng, ai đó liên tục chạm giới
//! hạn ngày, key OpenAI hỏng). Token bot nằm ở tệp `NOTIFY_BOT_TOKEN_PATH` (600), chat id là biến build
//! `NOTIFY_CHAT_ID`; bot chỉ nhắn được người đã bấm "Bắt đầu" với nó. Thiếu cấu hình thì chỉ log. Gửi
//! nền: lỗi không làm hỏng request, và không log token.

use crate::config::Config;
use crate::state::AppState;
use serde_json::json;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

/// Host của `api` và địa chỉ thay DNS cho nó theo `NOTIFY_BOT_RESOLVE` (`raw`). Chỉ nhận một địa chỉ IP: cổng lấy từ
/// URL (reqwest bỏ qua cổng của địa chỉ thay thế), nên `127.0.0.1:443` bị từ chối thay vì âm thầm bỏ cổng.
pub fn bot_resolve(api: &str, raw: Option<&str>) -> Result<Option<(String, SocketAddr)>, String> {
    let Some(raw) = raw else { return Ok(None) };
    let ip: IpAddr = raw
        .parse()
        .map_err(|_| format!("NOTIFY_BOT_RESOLVE phải là một địa chỉ IP (không kèm cổng), nhận \"{raw}\""))?;
    let url = reqwest::Url::parse(api).map_err(|_| "NOTIFY_BOT_API không phải URL".to_string())?;
    let host = url.host_str().filter(|h| h.parse::<IpAddr>().is_err()).ok_or("NOTIFY_BOT_RESOLVE cần NOTIFY_BOT_API có tên miền")?;
    Ok(Some((host.to_string(), SocketAddr::new(ip, 0))))
}

/// Client cho Bot API từ `builder` (cùng cấu hình mạng với client chung), thêm địa chỉ thay DNS nếu có.
pub fn bot_client(builder: reqwest::ClientBuilder, config: &Config) -> Result<reqwest::Client, String> {
    let builder = match bot_resolve(&config.notify_bot_api, config.notify_bot_resolve.as_deref())? {
        Some((host, addr)) => {
            tracing::info!(%host, ip = %addr.ip(), "Bot API chat: đi thẳng địa chỉ cấu hình, không phân giải DNS");
            builder.resolve(&host, addr)
        }
        None => builder,
    };
    builder.build().map_err(|e| format!("dựng client Bot API: {e}"))
}

/// Token bot đọc từ tệp `NOTIFY_BOT_TOKEN_PATH` (dùng chung cho báo động và liên kết chat). Lỗi không
/// chứa nội dung tệp.
pub async fn read_bot_token(config: &Config) -> Result<String, String> {
    let path = config.notify_bot_token_path.as_ref().ok_or("chưa cấu hình NOTIFY_BOT_TOKEN_PATH")?;
    let token = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| format!("không đọc được tệp token bot ({})", e.kind()))?;
    let token = token.trim().to_string();
    if token.is_empty() || token.chars().any(char::is_whitespace) {
        return Err("tệp token bot không đúng dạng".into());
    }
    Ok(token)
}

/// Gửi một tin nhắn, chờ kết quả (CLI `admin notify-test` và `send`).
pub async fn deliver(http: &reqwest::Client, config: &Config, text: &str) -> Result<(), String> {
    let Some(chat_id) = &config.notify_chat_id else {
        return Err("chưa cấu hình NOTIFY_BOT_TOKEN_PATH và NOTIFY_CHAT_ID".into());
    };
    let token = read_bot_token(config).await?;
    // Chat id là số; giữ dạng chuỗi nếu không đọc được số (API báo lỗi rõ ràng).
    let chat_id = chat_id.parse::<i64>().map(|n| json!(n)).unwrap_or_else(|_| json!(chat_id));
    let res = http
        .post(format!("{}/messages", config.notify_bot_api))
        .bearer_auth(&token)
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
    let (http, config) = (state.bot_http.clone(), state.config.clone());
    tokio::spawn(async move {
        if let Err(e) = deliver(&http, &config, &text).await {
            tracing::warn!(alert = %text, "không gửi được báo động: {e}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bot_host_can_be_pinned_to_an_address() {
        let api = "https://chat.hoandat.com/bot/v1";
        assert_eq!(bot_resolve(api, None), Ok(None));
        let pinned = bot_resolve(api, Some("127.0.0.1")).expect("IP");
        assert_eq!(pinned, Some(("chat.hoandat.com".to_string(), "127.0.0.1:0".parse().expect("addr"))));
        assert!(bot_resolve(api, Some("::1")).expect("IPv6").is_some());
        // Cổng lấy từ URL: địa chỉ kèm cổng hay tên miền đều bị từ chối, không âm thầm bỏ qua.
        assert!(bot_resolve(api, Some("127.0.0.1:443")).is_err());
        assert!(bot_resolve(api, Some("localhost")).is_err());
        assert!(bot_resolve("http://127.0.0.1:4600/bot/v1", Some("127.0.0.1")).is_err());
    }

    #[tokio::test]
    async fn a_pinned_client_connects_to_the_address_instead_of_dns() {
        // Host không tồn tại trong DNS: chỉ tới được listener cục bộ khi địa chỉ thay thế có hiệu lực.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let port = listener.local_addr().expect("addr").port();
        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let (mut sock, _) = listener.accept().await.expect("accept");
            let mut buf = vec![0u8; 1024];
            let n = sock.read(&mut buf).await.expect("read");
            let host_seen = String::from_utf8_lossy(&buf[..n]).contains("host: chat.scribe-test.invalid");
            let body = if host_seen { "pinned" } else { "wrong host" };
            let reply = format!("HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
            sock.write_all(reply.as_bytes()).await.expect("write");
        });
        let mut config = Config::for_tests("https://scribe.test", "https://auth.test");
        config.notify_bot_api = format!("http://chat.scribe-test.invalid:{port}/bot/v1");
        config.notify_bot_resolve = Some("127.0.0.1".into());
        let client = bot_client(reqwest::Client::builder(), &config).expect("client");
        let res = client.get(format!("{}/me", config.notify_bot_api)).send().await.expect("gửi");
        assert_eq!(res.text().await.expect("body"), "pinned");
    }
}
