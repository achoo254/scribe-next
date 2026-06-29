// G6 probe (Phase 1 gate): nhận PCM chunk từ WebView2 qua Tauri command (Uint8Array → Vec<u8> raw)
// rồi echo lại để JS đo round-trip JS→Rust→JS + xác nhận byte tới đủ (không bị méo qua JSON-array).
// Spike đo CHÂN audio (transport). Hop Rust→sidecar streaming = Phase 3 (cơ học giống transcribe.rs::send).
use tauri::{AppHandle, Emitter};

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
