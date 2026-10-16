//! ffmpeg (tìm khoảng lặng, cắt + nén Opus từng mảnh) và gọi OpenAI `audio/transcriptions`.
//!
//! API key (của server) chỉ đi qua header `Authorization`: không đưa vào thông báo lỗi, log hay argv
//! của tiến trình con.

use super::chunking::{self, Span};
use reqwest::multipart::{Form, Part};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

pub const MODEL: &str = "gpt-4o-mini-transcribe";
const API_URL: &str = "https://api.openai.com/v1/audio/transcriptions";
/// Mảnh có dấu hiệu lặp thì gọi lại với temperature cao hơn.
const RETRY_TEMPS: [&str; 3] = ["0", "0.4", "0.8"];
const HTTP_RETRIES: u32 = 3;
/// Âm lượng dưới ngưỡng này trong ít nhất `SILENCE_MIN_S` giây là khoảng lặng.
const SILENCE_NOISE: &str = "-35dB";
const SILENCE_MIN_S: &str = "0.4";

/// Vùng có tiếng nói của cả file, qua `silencedetect` (ffmpeg chỉ giải mã, không ghi gì ra đĩa).
pub async fn speech_spans(ffmpeg: &str, path: &str, duration: f64) -> Result<Vec<Span>, String> {
    let filter = format!("silencedetect=noise={SILENCE_NOISE}:d={SILENCE_MIN_S}");
    let out = Command::new(ffmpeg)
        .args(["-hide_banner", "-nostats", "-i", path, "-vn", "-af", &filter, "-f", "null", "-"])
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| format!("chạy ffmpeg: {e}"))?;
    let stderr = String::from_utf8_lossy(&out.stderr);
    if !out.status.success() {
        return Err(format!("ffmpeg không đọc được bản ghi: {}", last_line(&stderr)));
    }
    let silences = chunking::parse_silencedetect(&stderr, duration);
    Ok(chunking::speech_from_silences(&silences, duration))
}

/// Mảnh `[a, z)` → Opus mono 16 kHz 32 kbps trong bộ nhớ (~180 KB cho 45 giây), kèm âm lượng đỉnh (dB)
/// đo trong cùng lần giải mã (`None` khi ffmpeg không báo).
pub async fn encode_chunk(ffmpeg: &str, path: &str, chunk: Span) -> Result<(Vec<u8>, Option<f64>), String> {
    let (start, length) = (format!("{:.3}", chunk.0), format!("{:.3}", chunk.1 - chunk.0));
    let out = Command::new(ffmpeg)
        .args(["-hide_banner", "-nostats", "-loglevel", "info", "-ss", &start, "-t", &length])
        .args(["-i", path, "-vn", "-af", "volumedetect", "-ac", "1", "-ar", "16000", "-c:a", "libopus", "-b:a", "32k"])
        .args(["-f", "ogg", "pipe:1"])
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| format!("chạy ffmpeg: {e}"))?;
    if !out.status.success() || out.stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("ffmpeg không nén được mảnh: {}", last_line(&stderr)));
    }
    let peak = chunking::parse_max_volume(&String::from_utf8_lossy(&out.stderr));
    Ok((out.stdout, peak))
}

fn last_line(text: &str) -> &str {
    text.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim()
}

/// Chép một mảnh; chữ có dấu hiệu lặp thì thử lại với temperature cao hơn. Trả (chữ, số lần gửi đã
/// được OpenAI xử lý) — mỗi lần là một lần bị tính tiền.
pub async fn transcribe_chunk(
    http: &reqwest::Client,
    api_key: &str,
    audio: Vec<u8>,
    seconds: f64,
) -> Result<(String, u32), String> {
    let mut first = None;
    let mut posts = 0;
    for temperature in RETRY_TEMPS {
        let text = post(http, api_key, &audio, temperature).await?;
        posts += 1;
        if !chunking::looks_looped(&text, seconds) {
            return Ok((text, posts));
        }
        first.get_or_insert(text);
    }
    // Lần nào cũng có vẻ lặp: giữ bản temperature 0 (ít bịa nhất) thay vì bản 0,8.
    Ok((first.unwrap_or_default(), posts))
}

async fn post(http: &reqwest::Client, api_key: &str, audio: &[u8], temperature: &str) -> Result<String, String> {
    for attempt in 1..=HTTP_RETRIES {
        let part = Part::bytes(audio.to_vec())
            .file_name("chunk.ogg")
            .mime_str("audio/ogg")
            .map_err(|e| format!("dựng multipart: {e}"))?;
        let form = Form::new()
            .text("model", MODEL)
            .text("language", "vi")
            .text("response_format", "json")
            .text("temperature", temperature.to_string())
            .part("file", part);
        let sent = http
            .post(API_URL)
            .bearer_auth(api_key)
            .multipart(form)
            .timeout(Duration::from_secs(180))
            .send()
            .await;
        let last = attempt == HTTP_RETRIES;
        let response = match sent {
            Ok(r) => r,
            // Lỗi mạng: không in `e` (có thể chứa URL, không chứa key) cho gọn, chỉ báo loại lỗi.
            Err(e) if !last => {
                tracing::warn!(attempt, timeout = e.is_timeout(), "gọi OpenAI lỗi mạng, thử lại");
                backoff(attempt).await;
                continue;
            }
            Err(e) if e.is_timeout() => return Err("OpenAI: quá thời gian chờ phản hồi".into()),
            Err(_) => return Err("OpenAI: không kết nối được".into()),
        };
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        if (200..300).contains(&status) {
            let json: serde_json::Value =
                serde_json::from_str(&body).map_err(|_| "OpenAI trả dữ liệu không đọc được".to_string())?;
            return Ok(json["text"].as_str().unwrap_or("").trim().to_string());
        }
        let quota = is_quota(&body);
        if matches!(status, 429 | 500 | 502 | 503) && !quota && !last {
            backoff(attempt).await;
            continue;
        }
        return Err(api_error(status, &body));
    }
    Err("OpenAI: hết số lần thử".into())
}

async fn backoff(attempt: u32) {
    tokio::time::sleep(Duration::from_secs(3 * u64::from(attempt))).await;
}

fn is_quota(body: &str) -> bool {
    body.contains("insufficient_quota") || body.contains("credit_balance_exhausted")
}

/// Thông báo cho người dùng. Lỗi key/credit là lỗi "dừng hẳn": các file sau cũng sẽ hỏng y như vậy.
pub fn api_error(status: u16, body: &str) -> String {
    if status == 401 {
        return KEY_INVALID.into();
    }
    if is_quota(body) {
        return NO_CREDIT.into();
    }
    // Thông điệp của OpenAI có thể nêu tổ chức/dự án của tài khoản server: chỉ ghi log, người dùng thấy
    // câu chung kèm mã.
    let message = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(str::to_string))
        .unwrap_or_else(|| body.chars().take(200).collect());
    tracing::warn!(status, "OpenAI từ chối mảnh: {message}");
    if status == 400 {
        return "Không chép được đoạn audio này (OpenAI từ chối dữ liệu).".into();
    }
    format!("Dịch vụ chép lời đang lỗi (HTTP {status}), thử lại sau.")
}

/// Lỗi của key server (chỉ ghi log/audit cho quản trị; người dùng thấy thông báo tạm ngưng chung).
pub const KEY_INVALID: &str = "OpenAI: API key của server không hợp lệ hoặc đã bị thu hồi";
pub const NO_CREDIT: &str = "OpenAI: tài khoản của server đã hết credit";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_openai_errors_for_people() {
        assert_eq!(api_error(401, "{}"), KEY_INVALID);
        let quota = r#"{"error":{"code":"insufficient_quota","message":"You exceeded"}}"#;
        assert_eq!(api_error(429, quota), NO_CREDIT);
        let other = r#"{"error":{"message":"Rate limit reached in organization org-abc"}}"#;
        assert!(!api_error(429, other).contains("org-abc"));
        assert_eq!(api_error(400, "{}"), "Không chép được đoạn audio này (OpenAI từ chối dữ liệu).");
    }
}
