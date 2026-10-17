//! ffmpeg (ghép các đoạn, nén Opus) và gọi ElevenLabs `speech-to-text` (Scribe v2) cho cả tệp hoặc cả phiên trong
//! một request: timestamp từng từ và phân biệt người nói.
//!
//! API key (của server) chỉ đi qua header `xi-api-key`: không đưa vào thông báo lỗi, log hay argv của tiến trình con.

use super::chunking;
use super::words::Word;
use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

pub const API_URL: &str = "https://api.elevenlabs.io/v1/speech-to-text";
const HTTP_RETRIES: u32 = 3;
/// Chờ phản hồi tối thiểu (giây); cộng thêm thời lượng audio vì ElevenLabs xử lý xong mới trả lời.
const HTTP_TIMEOUT_BASE_S: u64 = 300;

/// Kết quả một request: các từ (mốc trong audio đã gửi) và thời lượng ElevenLabs tính tiền, nếu có.
#[derive(Debug)]
pub struct Transcript {
    pub words: Vec<Word>,
    pub duration_s: Option<f64>,
}

/// Một hay nhiều tệp `(đường dẫn, thời lượng đo được)` theo thứ tự, nối liền nhau → Opus mono 16 kHz 32 kbps trong bộ
/// nhớ (~14 MB mỗi giờ), kèm âm lượng đỉnh (dB) đo trong cùng lần giải mã (`None` khi ffmpeg không báo). Lỗi chỉ ghi
/// log chi tiết (đường dẫn có id người dùng), người dùng thấy câu chung.
pub async fn encode(ffmpeg: &str, inputs: &[(&str, f64)]) -> Result<(Vec<u8>, Option<f64>), String> {
    if inputs.is_empty() {
        return Err("không có tệp nào để chép".into());
    }
    let mut cmd = Command::new(ffmpeg);
    cmd.args(["-hide_banner", "-nostats", "-loglevel", "info"]);
    for (path, _) in inputs {
        cmd.args(["-i", path]);
    }
    let durations: Vec<f64> = inputs.iter().map(|(_, d)| *d).collect();
    cmd.args(["-filter_complex", &concat_filter(&durations), "-map", "[a]", "-vn"])
        .args(["-ac", "1", "-ar", "16000", "-c:a", "libopus", "-b:a", "32k", "-f", "ogg", "pipe:1"]);
    let out = cmd.stdin(Stdio::null()).kill_on_drop(true).output().await.map_err(|e| {
        tracing::warn!("chạy ffmpeg lỗi: {e}");
        ENCODE_FAILED.to_string()
    })?;
    if !out.status.success() || out.stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let last = stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
        tracing::warn!("ffmpeg không nén được bản ghi: {last}");
        return Err(ENCODE_FAILED.into());
    }
    let peak = chunking::parse_max_volume(&String::from_utf8_lossy(&out.stderr));
    Ok((out.stdout, peak))
}

/// Người dùng thấy câu này khi ffmpeg hỏng (không lộ đường dẫn tệp trên server).
pub const ENCODE_FAILED: &str = "Không đọc được audio của bản ghi để chép lời.";

/// Mỗi đầu vào cắt/đệm im lặng đúng bằng thời lượng đo được (`atrim` + `apad`, mốc đặt lại từ 0) rồi nối liền và đo
/// âm lượng đỉnh: mốc đoạn sau luôn bằng tổng thời lượng đo của các đoạn trước, kể cả khi độ trễ AAC làm audio giải
/// mã dài/ngắn hơn vài chục ms mỗi đoạn (phiên 180 đoạn không bị trôi mốc).
fn concat_filter(durations: &[f64]) -> String {
    let mut graph = String::new();
    for (i, d) in durations.iter().enumerate() {
        let d = d.max(0.0);
        graph.push_str(&format!("[{i}:a:0]atrim=end={d:.3},apad=whole_dur={d:.3},asetpts=N/SR/TB[a{i}];"));
    }
    let streams: String = (0..durations.len()).map(|i| format!("[a{i}]")).collect();
    graph.push_str(&format!("{streams}concat=n={}:v=0:a=1,volumedetect[a]", durations.len()));
    graph
}

/// Gửi audio đã nén. Chỉ thử lại (tối đa [`HTTP_RETRIES`] lần) khi chắc chắn ElevenLabs chưa xử lý: không kết nối được,
/// hoặc ElevenLabs trả 429 (trừ hết credit) / 5xx. Mất kết nối sau khi đã gửi audio hay hết thời gian chờ thì không gửi
/// lại (có thể đã bị tính tiền) — xem [`maybe_billed`].
pub async fn transcribe(
    http: &reqwest::Client,
    url: &str,
    api_key: &str,
    audio: &[u8],
    seconds: f64,
) -> Result<Transcript, String> {
    let timeout = Duration::from_secs(HTTP_TIMEOUT_BASE_S + seconds.max(0.0) as u64);
    for attempt in 1..=HTTP_RETRIES {
        let part = Part::bytes(audio.to_vec())
            .file_name("audio.ogg")
            .mime_str("audio/ogg")
            .map_err(|e| format!("dựng multipart: {e}"))?;
        let form = Form::new()
            .text("model_id", crate::config::Provider::ElevenLabs.model())
            .text("language_code", "vi")
            .text("diarize", "true")
            .text("timestamps_granularity", "word")
            .text("tag_audio_events", "false")
            .part("file", part);
        let sent = http.post(url).header("xi-api-key", api_key).multipart(form).timeout(timeout).send().await;
        let last = attempt == HTTP_RETRIES;
        let response = match sent {
            Ok(r) => r,
            Err(e) if e.is_connect() && !last => {
                tracing::warn!(attempt, "không kết nối được ElevenLabs, thử lại");
                super::openai::backoff(attempt).await;
                continue;
            }
            Err(e) if e.is_connect() => return Err(UNREACHABLE.into()),
            Err(e) if e.is_timeout() => return Err(TIMED_OUT.into()),
            Err(_) => return Err(LOST.into()),
        };
        let status = response.status().as_u16();
        let body = match response.text().await {
            Ok(body) => body,
            Err(_) if (200..300).contains(&status) => return Err(LOST.into()),
            Err(_) => String::new(),
        };
        if (200..300).contains(&status) {
            return parse(&body);
        }
        if matches!(status, 429 | 500 | 502 | 503) && !is_quota(&body) && !last {
            tracing::warn!(attempt, status, "ElevenLabs bận, thử lại");
            super::openai::backoff(attempt).await;
            continue;
        }
        return Err(api_error(status, &body));
    }
    Err("ElevenLabs: hết số lần thử".into())
}

const UNREACHABLE: &str = "ElevenLabs: không kết nối được";
const TIMED_OUT: &str = "ElevenLabs: quá thời gian chờ phản hồi";
const LOST: &str = "ElevenLabs: mất kết nối sau khi đã gửi audio";
const UNREADABLE: &str = "ElevenLabs trả dữ liệu không đọc được";

/// Lỗi mà ElevenLabs có thể đã xử lý (và tính tiền) audio: hết thời gian chờ, mất kết nối sau khi gửi, trả 200 mà
/// không đọc được. Mọi lỗi khác là từ chối chắc chắn: không tính tiền, người gọi hoàn phút.
pub fn maybe_billed(e: &str) -> bool {
    e == TIMED_OUT || e == LOST || e == UNREADABLE
}

#[derive(Deserialize)]
struct RawResponse {
    #[serde(default)]
    words: Vec<RawWord>,
    #[serde(default)]
    audio_duration_secs: Option<f64>,
}

#[derive(Deserialize)]
struct RawWord {
    #[serde(default)]
    text: String,
    #[serde(rename = "type", default)]
    kind: String,
    start: Option<f64>,
    end: Option<f64>,
    speaker_id: Option<String>,
}

/// Chỉ giữ phần tử `type = word` có mốc (bỏ khoảng trắng và sự kiện âm thanh); mốc hỏng thì bỏ từ đó.
fn parse(body: &str) -> Result<Transcript, String> {
    let raw: RawResponse = serde_json::from_str(body).map_err(|_| UNREADABLE.to_string())?;
    let words = raw
        .words
        .into_iter()
        .filter(|w| w.kind == "word" && !w.text.trim().is_empty())
        .filter_map(|w| {
            let start = w.start.filter(|s| s.is_finite() && *s >= 0.0)?;
            let end = w.end.filter(|e| e.is_finite()).unwrap_or(start).max(start);
            Some(Word { text: w.text.trim().to_string(), start, end, speaker: w.speaker_id })
        })
        .collect();
    let duration_s = raw.audio_duration_secs.filter(|d| d.is_finite() && *d > 0.0);
    Ok(Transcript { words, duration_s })
}

fn is_quota(body: &str) -> bool {
    body.contains("quota_exceeded") || body.contains("insufficient_credits")
}

/// Thông báo cho người dùng. Key hỏng (401, 403) là lỗi "dừng hẳn"; hết credit/quota tháng (402
/// `insufficient_credits`, hoặc 401 kèm `quota_exceeded` ở bản cũ) là [`QUOTA_EXHAUSTED`] — chặn tới tháng sau.
pub fn api_error(status: u16, body: &str) -> String {
    if status == 402 || is_quota(body) {
        return QUOTA_EXHAUSTED.into();
    }
    if status == 401 || status == 403 {
        return KEY_INVALID.into();
    }
    // Thông điệp của ElevenLabs có thể nêu tài khoản của server: chỉ ghi log, người dùng thấy câu chung kèm mã.
    let detail = serde_json::from_str::<serde_json::Value>(body).ok();
    let code = detail.as_ref().and_then(|v| v["detail"]["code"].as_str().map(str::to_string)).unwrap_or_default();
    let message = detail
        .as_ref()
        .and_then(|v| v["detail"]["message"].as_str().map(str::to_string))
        .unwrap_or_else(|| body.chars().take(200).collect());
    tracing::warn!(status, code, "ElevenLabs từ chối bản ghi: {message}");
    if status == 400 || status == 422 {
        return "Không chép được bản ghi này (ElevenLabs từ chối dữ liệu).".into();
    }
    format!("Dịch vụ chép lời đang lỗi (HTTP {status}), thử lại sau.")
}

/// Lỗi "dừng hẳn" của key ElevenLabs: các lượt sau cũng sẽ hỏng y vậy.
pub fn is_key_broken(e: &str) -> bool {
    e == KEY_INVALID
}

pub const KEY_INVALID: &str = "ElevenLabs: API key của server không hợp lệ, bị thu hồi hoặc thiếu quyền";
/// ElevenLabs báo tài khoản hết credit của tháng (gói Starter không có tính thêm theo dùng).
pub const QUOTA_EXHAUSTED: &str = "ElevenLabs: tài khoản của server đã hết quota tháng";

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::post;
    use axum::Router;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[test]
    fn concat_filter_pins_every_input_to_its_measured_length() {
        assert_eq!(
            concat_filter(&[60.0, 29.5]),
            "[0:a:0]atrim=end=60.000,apad=whole_dur=60.000,asetpts=N/SR/TB[a0];\
             [1:a:0]atrim=end=29.500,apad=whole_dur=29.500,asetpts=N/SR/TB[a1];\
             [a0][a1]concat=n=2:v=0:a=1,volumedetect[a]"
        );
    }

    #[test]
    fn only_lost_or_timed_out_requests_may_be_billed() {
        for e in [TIMED_OUT, LOST, UNREADABLE] {
            assert!(maybe_billed(e), "{e}");
        }
        for e in [UNREACHABLE, KEY_INVALID, QUOTA_EXHAUSTED, "Dịch vụ chép lời đang lỗi (HTTP 500), thử lại sau."] {
            assert!(!maybe_billed(e), "{e}");
        }
    }

    /// Thời lượng bản ghép bằng đúng tổng thời lượng đo, kể cả khi một đầu vào dài hơn (bị cắt) hay ngắn hơn (được
    /// đệm). Cần ffmpeg (bỏ qua nếu máy không có).
    #[tokio::test]
    async fn merged_audio_is_exactly_the_sum_of_measured_lengths() {
        let dir = std::env::temp_dir().join(format!("scribe-concat-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("tạo thư mục");
        let make = |name: &str, seconds: &str| {
            let path = dir.join(name);
            std::process::Command::new("ffmpeg")
                .args(["-v", "error", "-y", "-f", "lavfi", "-i", &format!("sine=frequency=440:duration={seconds}")])
                .args(["-c:a", "aac", "-b:a", "64k"])
                .arg(&path)
                .status()
                .ok()
                .filter(|s| s.success())
                .map(|_| path.to_string_lossy().to_string())
        };
        let (Some(a), Some(b)) = (make("a.m4a", "3.0"), make("b.m4a", "2.0")) else {
            eprintln!("bỏ qua: không có ffmpeg");
            return;
        };
        // Đo nói đoạn a dài 2,5 giây (cắt bớt) và b dài 2,6 giây (đệm thêm): bản ghép phải dài 5,1 giây.
        let (audio, peak) = encode("ffmpeg", &[(&a, 2.5), (&b, 2.6)]).await.expect("nén");
        assert!(peak.is_some_and(|p| p > -20.0), "{peak:?}");
        let out = dir.join("out.ogg");
        std::fs::write(&out, audio).expect("ghi");
        let probe = std::process::Command::new("ffprobe")
            .args(["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0"])
            .arg(&out)
            .output()
            .expect("ffprobe");
        let seconds: f64 = String::from_utf8_lossy(&probe.stdout).trim().parse().expect("thời lượng");
        assert!((seconds - 5.1).abs() < 0.03, "{seconds}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_keeps_timed_words_only() {
        let body = r#"{"language_code":"vie","text":"Xin chào.","audio_duration_secs":12.5,"words":[
            {"text":"Xin","type":"word","start":0.1,"end":0.3,"speaker_id":"speaker_0","logprob":-0.1},
            {"text":" ","type":"spacing","start":0.3,"end":0.35,"speaker_id":"speaker_0","logprob":0},
            {"text":"chào.","type":"word","start":0.35,"end":0.8,"speaker_id":"speaker_0","logprob":-0.2},
            {"text":"(cười)","type":"audio_event","start":1.0,"end":1.5,"speaker_id":"speaker_1","logprob":0},
            {"text":"mất","type":"word","start":null,"end":null,"speaker_id":null,"logprob":0}
        ]}"#;
        let t = parse(body).expect("đọc được");
        assert_eq!(t.words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(), vec!["Xin", "chào."]);
        assert_eq!(t.words[0].speaker.as_deref(), Some("speaker_0"));
        assert_eq!(t.duration_s, Some(12.5));
        assert!(parse("không phải json").is_err());
        assert_eq!(parse(r#"{"words":[]}"#).expect("rỗng").duration_s, None);
    }

    #[test]
    fn maps_elevenlabs_errors_for_people() {
        let invalid = r#"{"detail":{"type":"authentication_error","code":"invalid_api_key","message":"bad"}}"#;
        assert_eq!(api_error(401, invalid), KEY_INVALID);
        assert_eq!(api_error(403, "{}"), KEY_INVALID);
        // Hết credit tháng không phải key hỏng: chặn tới tháng sau, không ngắt tới khi khởi động lại.
        assert_eq!(api_error(402, r#"{"detail":{"code":"insufficient_credits"}}"#), QUOTA_EXHAUSTED);
        let legacy = r#"{"detail":{"status":"quota_exceeded","message":"You have 12 credits remaining, while 26 credits are required"}}"#;
        assert_eq!(api_error(401, legacy), QUOTA_EXHAUSTED);
        assert!(is_key_broken(&api_error(401, invalid)) && !is_key_broken(QUOTA_EXHAUSTED));
        let other = r#"{"detail":{"code":"system_busy","message":"workspace acme busy"}}"#;
        assert!(!api_error(429, other).contains("acme"));
        assert!(!is_key_broken(&api_error(500, "{}")));
        assert_eq!(api_error(422, "{}"), "Không chép được bản ghi này (ElevenLabs từ chối dữ liệu).");
    }

    /// ElevenLabs giả trên cổng cục bộ: kiểm header, trường multipart; lần đầu trả 429 rồi 200.
    #[tokio::test]
    async fn posts_the_documented_fields_and_retries_when_busy() {
        let calls = Arc::new(AtomicU32::new(0));
        let seen = calls.clone();
        let handler = move |headers: HeaderMap, body: axum::body::Bytes| {
            let seen = seen.clone();
            async move {
                assert_eq!(headers.get("xi-api-key").and_then(|v| v.to_str().ok()), Some("k-test"));
                let body = String::from_utf8_lossy(&body);
                for (name, value) in [
                    ("model_id", "scribe_v2"),
                    ("language_code", "vi"),
                    ("diarize", "true"),
                    ("timestamps_granularity", "word"),
                    ("tag_audio_events", "false"),
                ] {
                    assert!(body.contains(&format!("name=\"{name}\"\r\n\r\n{value}\r\n")), "thiếu {name}={value}");
                }
                assert!(body.contains("name=\"file\"; filename=\"audio.ogg\"") && body.contains("ogg!"));
                assert!(!body.contains("num_speakers") && !body.contains("keyterms"));
                if seen.fetch_add(1, Ordering::SeqCst) == 0 {
                    return (StatusCode::TOO_MANY_REQUESTS, r#"{"detail":{"code":"rate_limit_exceeded"}}"#.to_string());
                }
                let body = r#"{"text":"Chào.","audio_duration_secs":2.0,"words":[
                    {"text":"Chào.","type":"word","start":0.2,"end":0.6,"speaker_id":"speaker_0","logprob":0}]}"#;
                (StatusCode::OK, body.to_string())
            }
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let url = format!("http://{}/v1/speech-to-text", listener.local_addr().expect("addr"));
        let app = Router::new().route("/v1/speech-to-text", post(handler));
        tokio::spawn(async move { axum::serve(listener, app).await });

        let t = transcribe(&reqwest::Client::new(), &url, "k-test", b"ogg!", 2.0).await.expect("chép");
        assert_eq!(calls.load(Ordering::SeqCst), 2, "429 được thử lại");
        assert_eq!(t.words.len(), 1);
        assert_eq!(t.duration_s, Some(2.0));
    }

    /// Key hỏng: không thử lại, trả lỗi "dừng hẳn".
    #[tokio::test]
    async fn invalid_key_stops_at_once() {
        let calls = Arc::new(AtomicU32::new(0));
        let seen = calls.clone();
        let handler = move || {
            let seen = seen.clone();
            async move {
                seen.fetch_add(1, Ordering::SeqCst);
                (StatusCode::UNAUTHORIZED, r#"{"detail":{"code":"invalid_api_key","message":"Invalid API key"}}"#)
            }
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let url = format!("http://{}/v1/speech-to-text", listener.local_addr().expect("addr"));
        tokio::spawn(async move { axum::serve(listener, Router::new().route("/v1/speech-to-text", post(handler))).await });
        let e = transcribe(&reqwest::Client::new(), &url, "k-bad", b"ogg!", 1.0).await.expect_err("phải lỗi");
        assert_eq!(e, KEY_INVALID);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
