//! Chép lời một mảnh audio (≤ 45 giây, Opus) bằng Google Gemini với key riêng của người dùng: `generateContent`, audio
//! gửi kèm (`inlineData`, base64), kết quả JSON theo `responseSchema` gồm các câu tiếng Việt và giây bắt đầu/kết thúc
//! tính từ đầu mảnh. Gemini không phân biệt người nói và mốc là ước lượng của model (giao diện nói rõ); mốc được kẹp
//! trong mảnh và xếp tăng dần.
//!
//! Key chỉ đi qua header `x-goog-api-key`; lỗi dùng chung cách đọc với model chữ (`summary::llm`), nên key hỏng hay hết
//! credit là `llm::KEY_INVALID` / `llm::NO_CREDIT`.

use super::chunking::{Sentence, Span};
use crate::config::Vendor;
use crate::summary::llm;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

/// Model Flash dùng để chép lời (model chữ mặc định của Gemini).
pub const MODEL: &str = "gemini-3.8-flash";
const HTTP_RETRIES: u32 = 3;
const TIMEOUT: Duration = Duration::from_secs(120);
/// Trần token đầu ra của một mảnh 45 giây (chữ + mốc dư dả).
const MAX_OUTPUT_TOKENS: i64 = 4_000;

const PROMPT: &str = "Chép lời tiếng Việt của đoạn audio này đúng như người nói, có dấu câu và viết hoa đầu câu. Chia \
thành từng câu; mỗi câu ghi start và end là giây (số thực) tính từ đầu đoạn audio. Không dịch, không tóm tắt, không thêm \
lời bình hay mô tả âm thanh. Không có lời nói thì trả sentences rỗng.";

fn schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["sentences"],
        "properties": {
            "sentences": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["start", "end", "text"],
                    "properties": {
                        "start": { "type": "number" },
                        "end": { "type": "number" },
                        "text": { "type": "string" },
                    },
                },
            },
        },
    })
}

#[derive(Debug, Deserialize)]
struct Draft {
    sentences: Vec<RawSentence>,
}

#[derive(Debug, Deserialize)]
struct RawSentence {
    start: f64,
    end: f64,
    text: String,
}

fn request_body(audio: &[u8]) -> Value {
    json!({
        "contents": [{ "role": "user", "parts": [
            { "inlineData": { "mimeType": "audio/ogg", "data": STANDARD.encode(audio) } },
            { "text": PROMPT },
        ] }],
        "generationConfig": {
            "temperature": 0.0,
            "maxOutputTokens": llm::output_cap(Vendor::Gemini, MAX_OUTPUT_TOKENS),
            "thinkingConfig": llm::gemini_thinking(MODEL),
            "responseMimeType": "application/json",
            "responseSchema": llm::gemini_schema(&schema()),
        },
    })
}

/// Câu của mảnh `chunk` theo mốc trong cả tệp: bỏ câu rỗng, kẹp mốc trong mảnh, xếp tăng dần.
fn place(raw: Vec<RawSentence>, chunk: Span) -> Vec<Sentence> {
    let length = (chunk.1 - chunk.0).max(0.0);
    let clamp = |x: f64| if x.is_finite() { x.clamp(0.0, length) } else { 0.0 };
    let mut out: Vec<Sentence> = raw
        .into_iter()
        .filter_map(|s| {
            let text = s.text.split_whitespace().collect::<Vec<_>>().join(" ");
            if text.is_empty() {
                return None;
            }
            let start = clamp(s.start);
            let end = clamp(s.end).max(start);
            Some(Sentence { start: chunk.0 + start, end: chunk.0 + end, text, speaker: None })
        })
        .collect();
    out.sort_by(|a, b| a.start.total_cmp(&b.start));
    out
}

/// Chép một mảnh. Trả câu với mốc trong cả tệp.
pub async fn transcribe_chunk(http: &reqwest::Client, api_key: &str, audio: &[u8], chunk: Span) -> Result<Vec<Sentence>, String> {
    transcribe_chunk_at(http, llm::GEMINI_API, api_key, audio, chunk).await
}

async fn transcribe_chunk_at(http: &reqwest::Client, base: &str, api_key: &str, audio: &[u8], chunk: Span) -> Result<Vec<Sentence>, String> {
    let url = format!("{base}/v1beta/models/{MODEL}:generateContent");
    let body = request_body(audio);
    for attempt in 1..=HTTP_RETRIES {
        let last = attempt == HTTP_RETRIES;
        let sent = http.post(&url).header("x-goog-api-key", api_key).json(&body).timeout(TIMEOUT).send().await;
        let response = match sent {
            Ok(r) => r,
            Err(e) if e.is_connect() && !last => {
                super::openai::backoff(attempt).await;
                continue;
            }
            Err(e) if e.is_timeout() => return Err("Gemini: quá thời gian chờ phản hồi".into()),
            Err(_) => return Err("Gemini: không kết nối được".into()),
        };
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        if (200..300).contains(&status) {
            let completion = llm::parse_response::<Draft>(Vendor::Gemini, &text)?;
            return completion.draft.map(|d| place(d.sentences, chunk)).map_err(|e| format!("Gemini: {e}"));
        }
        if llm::retryable(Vendor::Gemini, status, &text) && !last {
            super::openai::backoff(attempt).await;
            continue;
        }
        return Err(llm::api_error(Vendor::Gemini, status, &text));
    }
    Err("Gemini: hết số lần thử".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::post;
    use axum::Router;

    #[test]
    fn request_sends_inline_audio_and_a_gemini_schema() {
        let body = request_body(b"ogg!");
        let parts = &body["contents"][0]["parts"];
        assert_eq!(parts[0]["inlineData"]["mimeType"], "audio/ogg");
        assert_eq!(parts[0]["inlineData"]["data"], STANDARD.encode(b"ogg!"));
        assert!(parts[1]["text"].as_str().is_some_and(|t| t.contains("tiếng Việt")));
        let schema = &body["generationConfig"]["responseSchema"];
        assert_eq!(schema["properties"]["sentences"]["items"]["type"], "OBJECT");
        assert!(schema.get("additionalProperties").is_none());
    }

    #[test]
    fn sentences_are_clamped_into_the_chunk_and_sorted() {
        let raw = vec![
            RawSentence { start: 12.0, end: 20.0, text: " Câu hai. ".into() },
            RawSentence { start: 1.0, end: 0.5, text: "Câu một.".into() },
            RawSentence { start: 50.0, end: 90.0, text: "Ngoài mảnh.".into() },
            RawSentence { start: 3.0, end: 4.0, text: "  ".into() },
        ];
        let out = place(raw, (60.0, 100.0));
        let got: Vec<(f64, f64, &str)> = out.iter().map(|s| (s.start, s.end, s.text.as_str())).collect();
        assert_eq!(got, vec![(61.0, 61.0, "Câu một."), (72.0, 80.0, "Câu hai."), (100.0, 100.0, "Ngoài mảnh.")]);
        assert!(out.iter().all(|s| s.speaker.is_none()));
    }

    /// Gemini giả: kiểm header key, trả câu theo schema; key sai ⟹ lỗi key của model chữ.
    #[tokio::test]
    async fn posts_audio_and_maps_errors() {
        let handler = |headers: HeaderMap| async move {
            if headers.get("x-goog-api-key").and_then(|v| v.to_str().ok()) != Some("gm-test-key-0123456789") {
                let body = r#"{"error":{"code":400,"message":"API key not valid.","details":[{"reason":"API_KEY_INVALID"}]}}"#;
                return (StatusCode::BAD_REQUEST, body.to_string());
            }
            let content = json!({ "sentences": [{ "start": 0.2, "end": 1.4, "text": "Xin chào." }] }).to_string();
            let reply = json!({
                "candidates": [{ "finishReason": "STOP", "content": { "parts": [{ "text": content }] } }],
                "usageMetadata": { "promptTokenCount": 1500, "candidatesTokenCount": 20 },
            });
            (StatusCode::OK, reply.to_string())
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base = format!("http://{}", listener.local_addr().expect("addr"));
        let path = format!("/v1beta/models/{MODEL}:generateContent");
        tokio::spawn(async move { axum::serve(listener, Router::new().route(&path, post(handler))).await });
        let http = reqwest::Client::new();
        let out = transcribe_chunk_at(&http, &base, "gm-test-key-0123456789", b"ogg!", (10.0, 40.0)).await.expect("chép");
        assert_eq!(out.len(), 1);
        assert_eq!((out[0].start, out[0].text.as_str()), (10.2, "Xin chào."));
        let e = transcribe_chunk_at(&http, &base, "gm-test-wrong-0000000", b"ogg!", (0.0, 1.0)).await.expect_err("key sai");
        assert_eq!(e, llm::KEY_INVALID);
    }
}
