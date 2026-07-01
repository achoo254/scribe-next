// Meeting live-transcribe (Phase 3): chân audio WebView2 → Rust → sidecar streaming.
//
// G6 probe (Phase 1 gate) giữ lại làm chẩn đoán round-trip JS→Rust→JS.
//
// MeetingTranscriber: spawn CÙNG sidecar transcriber với cờ `--stream` (tiến trình RIÊNG với batch
// Mode C → [C2] batch blocking KHÔNG chặn caption). Nhận PCM theo track từ frontend → forward xuống
// sidecar; đọc caption sidecar phát ra → emit `meeting_caption_msg` (kênh RIÊNG với `transcribe_msg`
// của batch → [H4] không đụng use-transcribe.ts).
//
// Chống deadlock [C2]: writer thread SỞ HỮU ChildStdin, nhận line qua bounded channel; PCM dồn nhanh
// hơn sidecar đọc → try_send drop-on-full + emit backpressure (KHÔNG block command/UI, KHÔNG deadlock).
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter, State};

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

// Đủ đệm cho vài track ở nhịp chunk ~0.25–0.5s; đầy = backlog → drop PCM cũ nhất tại sender.
const CHANNEL_CAP: usize = 256;

// ── G6 probe (chẩn đoán chân audio) ─────────────────────────────────────────
#[derive(serde::Serialize, Clone)]
struct PcmEcho {
    seq: u32,
    t0_ms: f64,
    bytes: usize,
}

/// Nhận 1 chunk PCM (16kHz mono int16 LE) + timestamp gửi đi → emit echo ngay để đo round-trip.
#[tauri::command]
pub fn meeting_pcm_probe(app: AppHandle, seq: u32, t0_ms: f64, pcm: Vec<u8>) -> Result<(), String> {
    app.emit("meeting_pcm_echo", PcmEcho { seq, t0_ms, bytes: pcm.len() })
        .map_err(|e| e.to_string())
}

// ── Streaming transcribe (Phase 3) ──────────────────────────────────────────

/// Track id chỉ `[A-Za-z0-9_-]{1,64}` — chặn path traversal / ký tự lạ trước khi gửi xuống sidecar.
fn valid_track(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// base64 chuẩn (có padding) — mã hóa PCM cho hop Rust→sidecar (line-JSON). Tự cài để khỏi thêm crate.
fn base64_encode(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        out.push(T[(b0 >> 2) as usize] as char);
        out.push(T[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        out.push(if chunk.len() > 1 { T[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[(b2 & 0x3f) as usize] as char } else { '=' });
    }
    out
}

#[derive(Default)]
pub struct MeetingTranscriber {
    inner: Mutex<Option<StreamProc>>,
}

struct StreamProc {
    child: Child,
    tx: SyncSender<String>,
    dropped: Arc<AtomicU64>,
}

impl Drop for StreamProc {
    fn drop(&mut self) {
        let _ = self.tx.try_send("{\"cmd\":\"shutdown\"}".to_string());
        let _ = self.child.kill();
        let _ = self.child.wait(); // reap, tránh zombie trên Unix/macOS
    }
}

impl MeetingTranscriber {
    /// Spawn sidecar `--stream` nếu chưa chạy (hoặc đã chết). Dựng writer + reader thread.
    fn ensure_spawned(&self, app: &AppHandle) -> Result<(), String> {
        let mut g = self.inner.lock().map_err(|e| e.to_string())?;
        if let Some(p) = g.as_mut() {
            if matches!(p.child.try_wait(), Ok(None)) {
                return Ok(()); // còn sống
            }
        }
        let (program, mut args) = crate::transcribe::resolve(app)?;
        args.push("--stream".to_string());
        let mut cmd = Command::new(&program);
        cmd.args(&args).stdin(Stdio::piped()).stdout(Stdio::piped());
        #[cfg(debug_assertions)]
        cmd.stderr(Stdio::inherit());
        #[cfg(not(debug_assertions))]
        cmd.stderr(Stdio::null());
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Không chạy được sidecar streaming: {e}"))?;
        let mut stdin = child.stdin.take().ok_or("Không mở được stdin sidecar streaming")?;
        let stdout = child.stdout.take().ok_or("Không mở được stdout sidecar streaming")?;

        // Writer thread SỞ HỮU ChildStdin — line từ bounded channel → ghi. Tách khỏi thread lệnh Tauri.
        let (tx, rx) = sync_channel::<String>(CHANNEL_CAP);
        thread::spawn(move || {
            for line in rx.iter() {
                if stdin.write_all(line.as_bytes()).is_err() || stdin.write_all(b"\n").is_err() {
                    break;
                }
                let _ = stdin.flush();
            }
        });

        // Reader thread: stdout JSON → emit meeting_caption_msg. EOF (sidecar chết) → sidecar_crashed [H3].
        let app2 = app.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                    let _ = app2.emit("meeting_caption_msg", v);
                }
            }
            let _ = app2.emit("meeting_caption_msg", serde_json::json!({ "type": "sidecar_crashed" }));
        });

        *g = Some(StreamProc { child, tx, dropped: Arc::new(AtomicU64::new(0)) });
        Ok(())
    }

    /// Clone (tx, dropped) rồi NHẢ mutex — gửi channel KHÔNG giữ lock (tránh block command khác).
    fn handles(&self) -> Result<(SyncSender<String>, Arc<AtomicU64>), String> {
        let g = self.inner.lock().map_err(|e| e.to_string())?;
        let p = g.as_ref().ok_or("Sidecar streaming chưa khởi động")?;
        Ok((p.tx.clone(), p.dropped.clone()))
    }

    /// Lệnh điều khiển (start/end/shutdown): hiếm + nhỏ → block-send để KHÔNG mất lệnh.
    fn send_control(&self, line: String) -> Result<(), String> {
        let (tx, _) = self.handles()?;
        tx.send(line).map_err(|e| e.to_string())
    }

    /// PCM: try_send drop-on-full (bound backlog, KHÔNG deadlock) + emit backpressure gộp mỗi 25 lần.
    fn send_pcm(&self, app: &AppHandle, line: String) -> Result<(), String> {
        let (tx, dropped) = self.handles()?;
        match tx.try_send(line) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => {
                let n = dropped.fetch_add(1, Ordering::Relaxed) + 1;
                if n % 25 == 1 {
                    let _ = app.emit(
                        "meeting_caption_msg",
                        serde_json::json!({ "type": "backpressure", "scope": "ipc", "dropped": n }),
                    );
                }
                Ok(())
            }
            Err(TrySendError::Disconnected(_)) => Err("writer sidecar streaming đã dừng".into()),
        }
    }

    /// Tắt streaming: kill sidecar (lần sau tự spawn lại). Gọi khi rời phòng / thoát app.
    pub fn shutdown(&self) {
        if let Ok(mut g) = self.inner.lock() {
            g.take(); // Drop for StreamProc → shutdown + kill
        }
    }
}

/// Bắt đầu transcribe 1 track (tạo session trong sidecar). `size` mặc định "small" (live).
#[tauri::command]
pub fn meeting_transcribe_start(
    app: AppHandle,
    state: State<MeetingTranscriber>,
    track: String,
    size: Option<String>,
) -> Result<(), String> {
    if !valid_track(&track) {
        return Err("track id không hợp lệ".into());
    }
    let size = size.unwrap_or_else(|| "small".to_string());
    state.ensure_spawned(&app)?;
    let dir = crate::transcribe::model_dir(&app, &size)
        .to_string_lossy()
        .to_string();
    state.send_control(
        serde_json::json!({ "cmd": "stream_start", "track": track, "model_dir": dir, "size": size })
            .to_string(),
    )
}

/// Đẩy 1 chunk PCM (16kHz mono int16 LE, raw bytes) của track xuống sidecar.
#[tauri::command]
pub fn meeting_transcribe_pcm(
    app: AppHandle,
    state: State<MeetingTranscriber>,
    track: String,
    pcm: Vec<u8>,
) -> Result<(), String> {
    if !valid_track(&track) {
        return Err("track id không hợp lệ".into());
    }
    let b64 = base64_encode(&pcm);
    state.send_pcm(
        &app,
        serde_json::json!({ "cmd": "stream_pcm", "track": track, "pcm": b64 }).to_string(),
    )
}

/// Kết thúc 1 track (người rời phòng / tắt mic) — sidecar chốt nốt utterance đang mở.
#[tauri::command]
pub fn meeting_transcribe_end(
    state: State<MeetingTranscriber>,
    track: String,
) -> Result<(), String> {
    if !valid_track(&track) {
        return Err("track id không hợp lệ".into());
    }
    state.send_control(serde_json::json!({ "cmd": "stream_end", "track": track }).to_string())
}

/// Dừng toàn bộ streaming (rời phòng): kill sidecar streaming.
#[tauri::command]
pub fn meeting_transcribe_stop(state: State<MeetingTranscriber>) -> Result<(), String> {
    state.shutdown();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn base64_roundtrips_binary_pcm() {
        // int16 LE mẫu — độ dài không chia hết 3 để kiểm padding.
        let pcm: Vec<u8> = (0u8..=250).collect();
        let enc = base64_encode(&pcm);
        assert_eq!(enc.len(), pcm.len().div_ceil(3) * 4);
        assert!(enc.chars().all(|c| c.is_ascii_alphanumeric() || "+/=".contains(c)));
    }

    #[test]
    fn track_id_validation() {
        assert!(valid_track("participant-1_abc"));
        assert!(!valid_track(""));
        assert!(!valid_track("bad id!"));
        assert!(!valid_track("../etc/passwd"));
        assert!(!valid_track(&"x".repeat(65)));
    }
}
