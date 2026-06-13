// Mode A state machine + điều phối: cpal stream → ffmpeg stdin (segment muxer).
// Stream (!Send) + ffmpeg spawn + writer loop đều nằm TRONG record thread.
// Stop sạch: drop(stream) ngừng capture, drop(child) đóng stdin → ffmpeg nhận EOF → finalize.
use crate::audio::{capture, encoder, meter};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;

#[derive(Clone, serde::Serialize)]
pub struct SegmentFile {
    pub name: String,
    pub path: String,
}

#[derive(Default)]
pub struct Recorder {
    inner: Mutex<Option<Session>>,
}

struct Session {
    running: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    done: Arc<AtomicBool>, // thread đã kết thúc (tự dừng do lỗi/EOF) → cho phép start lại
    handle: Option<JoinHandle<()>>,
}

fn emit_state(app: &AppHandle, state: &str) {
    let _ = app.emit("rec_state", state.to_string());
}

impl Recorder {
    pub fn start(
        &self,
        app: &AppHandle,
        mic: Option<String>,
        out_dir: String,
        format: String,
        seg_secs: f64,
    ) -> Result<(), String> {
        let mut guard = self.inner.lock().map_err(|e| e.to_string())?;
        // Phiên cũ còn chạy → từ chối. Phiên cũ đã tự dừng (done) → dọn rồi tiếp tục.
        if let Some(s) = guard.as_ref() {
            if !s.done.load(Ordering::SeqCst) {
                return Err("Đang ghi rồi".into());
            }
        }
        if let Some(mut stale) = guard.take() {
            if let Some(h) = stale.handle.take() {
                let _ = h.join();
            }
        }

        let dir = PathBuf::from(&out_dir);
        if !dir.is_dir() {
            return Err("Thư mục lưu không hợp lệ".into());
        }
        let ext = encoder::ext_for(&format);
        let prefix = format!("rec_{}", chrono::Local::now().format("%Y%m%d_%H%M%S"));

        let running = Arc::new(AtomicBool::new(true));
        let paused = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicBool::new(false));
        let app = app.clone();
        let (r, p, d) = (running.clone(), paused.clone(), done.clone());

        let handle = std::thread::spawn(move || {
            record_loop(app, mic, dir, format, ext, prefix, seg_secs, r, p, d);
        });

        *guard = Some(Session {
            running,
            paused,
            done,
            handle: Some(handle),
        });
        Ok(())
    }

    pub fn pause(&self, app: &AppHandle) {
        if let Ok(g) = self.inner.lock() {
            if let Some(s) = g.as_ref() {
                s.paused.store(true, Ordering::SeqCst);
                emit_state(app, "paused");
            }
        }
    }

    pub fn resume(&self, app: &AppHandle) {
        if let Ok(g) = self.inner.lock() {
            if let Some(s) = g.as_ref() {
                s.paused.store(false, Ordering::SeqCst);
                emit_state(app, "recording");
            }
        }
    }

    /// Lấy handle ra để join ở nơi khác (spawn_blocking) — không block runtime worker.
    pub fn take_for_stop(&self) -> Option<JoinHandle<()>> {
        let mut s = self.inner.lock().ok()?.take()?;
        s.running.store(false, Ordering::SeqCst);
        s.handle.take()
    }
}

#[allow(clippy::too_many_arguments)]
fn record_loop(
    app: AppHandle,
    mic: Option<String>,
    dir: PathBuf,
    format: String,
    ext: &'static str,
    prefix: String,
    seg_secs: f64,
    running: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    done: Arc<AtomicBool>,
) {
    let finish = |app: &AppHandle, dir: &Path, prefix: &str| {
        let _ = app.emit("segments_final", list_segments(dir, prefix, ext));
        emit_state(app, "idle");
        done.store(true, Ordering::SeqCst);
    };
    let pattern = dir.join(format!("{prefix}_%03d.{ext}")).to_string_lossy().to_string();

    // 1) Build cpal stream (Stream !Send — sống trên thread này). Lỗi mic → dừng loop.
    let (tx, rx) = mpsc::channel::<Vec<i16>>();
    let (app_err, run_err) = (app.clone(), running.clone());
    let (stream, cfg) = match capture::build_stream(mic.as_deref(), tx, move |e| {
        run_err.store(false, Ordering::SeqCst);
        let _ = app_err.emit("rec_error", e);
    }) {
        Ok(v) => v,
        Err(e) => {
            let _ = app.emit("rec_error", e);
            finish(&app, &dir, &prefix);
            return;
        }
    };

    // 2) Spawn ffmpeg sidecar (đọc stdin), args theo rate/channels gốc.
    let args = encoder::ffmpeg_args(cfg.sample_rate, cfg.channels, &format, seg_secs, &pattern);
    let (mut events, mut child) = match app.shell().sidecar("ffmpeg").and_then(|c| c.args(args).spawn()) {
        Ok(v) => v,
        Err(e) => {
            let _ = app.emit("rec_error", format!("Không spawn được ffmpeg: {e}"));
            finish(&app, &dir, &prefix);
            return;
        }
    };

    use cpal::traits::StreamTrait;
    let _ = stream.play();
    emit_state(&app, "recording");

    // 3) Writer loop: drain event ffmpeg + feed stdin (trừ khi paused) + meter + quét segment.
    let mut seen: HashSet<String> = HashSet::new();
    let mut last_level = Instant::now();
    let mut last_scan = Instant::now();
    while running.load(Ordering::SeqCst) {
        drain_events(&app, &mut events, &running);
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(buf) => {
                if !paused.load(Ordering::SeqCst) {
                    let mut bytes = Vec::with_capacity(buf.len() * 2);
                    for s in &buf {
                        bytes.extend_from_slice(&s.to_le_bytes());
                    }
                    if child.write(&bytes).is_err() {
                        break;
                    }
                }
                if last_level.elapsed() >= Duration::from_millis(33) {
                    let _ = app.emit("level", meter::compute(&buf));
                    last_level = Instant::now();
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }
        if last_scan.elapsed() >= Duration::from_secs(1) {
            scan_new_segments(&app, &dir, &prefix, ext, &mut seen);
            last_scan = Instant::now();
        }
    }

    // 4) Stop sạch: ngừng capture, đóng stdin (EOF) → chờ ffmpeg finalize.
    drop(stream);
    drop(child);
    loop {
        match events.blocking_recv() {
            Some(CommandEvent::Terminated(_)) | None => break,
            Some(CommandEvent::Stderr(b)) => {
                let _ = app.emit("rec_error", String::from_utf8_lossy(&b).to_string());
            }
            Some(_) => {}
        }
    }
    finish(&app, &dir, &prefix);
}

/// Drain non-blocking event ffmpeg trong lúc ghi (tránh đầy channel → block stdin).
fn drain_events(
    app: &AppHandle,
    events: &mut tauri::async_runtime::Receiver<CommandEvent>,
    running: &Arc<AtomicBool>,
) {
    while let Ok(ev) = events.try_recv() {
        match ev {
            CommandEvent::Stderr(b) => {
                let _ = app.emit("rec_error", String::from_utf8_lossy(&b).to_string());
            }
            CommandEvent::Error(e) => {
                let _ = app.emit("rec_error", e);
            }
            CommandEvent::Terminated(_) => running.store(false, Ordering::SeqCst),
            _ => {}
        }
    }
}

fn matches_session(name: &str, prefix: &str, ext: &str) -> bool {
    name.starts_with(prefix) && name.ends_with(&format!(".{ext}"))
}

fn scan_new_segments(app: &AppHandle, dir: &Path, prefix: &str, ext: &str, seen: &mut HashSet<String>) {
    for f in list_segments(dir, prefix, ext) {
        if seen.insert(f.name.clone()) {
            let _ = app.emit("segment_created", f);
        }
    }
}

fn list_segments(dir: &Path, prefix: &str, ext: &str) -> Vec<SegmentFile> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if matches_session(&name, prefix, ext) {
                out.push(SegmentFile {
                    name,
                    path: e.path().to_string_lossy().to_string(),
                });
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}
