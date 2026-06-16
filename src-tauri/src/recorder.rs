// Mode A state machine + điều phối: cpal stream → ffmpeg stdin (segment muxer).
// Stream (!Send) + ffmpeg spawn + writer loop đều nằm TRONG record thread.
// Stop sạch: drop(stream) ngừng capture, drop(child) đóng stdin → ffmpeg nhận EOF → finalize.
use crate::audio::{encoder, meter, source};
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
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        &self,
        app: &AppHandle,
        source: String,
        device: Option<String>,
        out_dir: String,
        format: String,
        seg_secs: f64,
        video: Option<encoder::VideoConfig>,
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
            record_loop(app, source, device, dir, format, ext, prefix, seg_secs, video, r, p, d);
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

    /// Dừng đồng bộ + chờ thread ghi tự dọn (gọi khi thoát app) — tránh bỏ rơi ffmpeg/thread loopback
    /// đang chạy. take_for_stop đã set running=false; join chờ thread đóng stdin ffmpeg → finalize
    /// đoạn cuối hợp lệ rồi ffmpeg thoát (không để tiến trình con mồ côi giữ GPU/ổ đĩa).
    pub fn stop_blocking(&self) {
        if let Some(handle) = self.take_for_stop() {
            let _ = handle.join();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn record_loop(
    app: AppHandle,
    source: String,
    device: Option<String>,
    dir: PathBuf,
    format: String,
    ext: &'static str,
    prefix: String,
    seg_secs: f64,
    video: Option<encoder::VideoConfig>,
    running: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    done: Arc<AtomicBool>,
) {
    let is_screen = video.is_some();
    let finish = |app: &AppHandle, dir: &Path, prefix: &str| {
        let _ = app.emit("segments_final", list_segments(dir, prefix, ext));
        emit_state(app, "idle");
        done.store(true, Ordering::SeqCst);
    };
    let pattern = dir.join(format!("{prefix}_%03d.{ext}")).to_string_lossy().to_string();

    // 1) Build nguồn ghi (mic cpal | tiếng hệ thống loopback). Lỗi mở thiết bị → dừng loop.
    let (tx, rx) = mpsc::channel::<Vec<i16>>();
    let (app_err, run_err) = (app.clone(), running.clone());
    let (capture, cfg) = match source::build_capture(&source, device.as_deref(), tx, move |e| {
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

    // 2) Spawn ffmpeg sidecar. Ghi màn hình: thêm input video ddagrab + mux MP4; ghi âm: stdin → mp3/m4a.
    let args = match &video {
        Some(v) => encoder::ffmpeg_args_screen(cfg.sample_rate, cfg.channels, v, seg_secs, &pattern),
        None => encoder::ffmpeg_args(cfg.sample_rate, cfg.channels, &format, seg_secs, &pattern),
    };
    let (mut events, mut child) = match app.shell().sidecar("ffmpeg").and_then(|c| c.args(args).spawn()) {
        Ok(v) => v,
        Err(e) => {
            let _ = app.emit("rec_error", format!("Không spawn được ffmpeg: {e}"));
            finish(&app, &dir, &prefix);
            return;
        }
    };

    if let Err(e) = capture.play() {
        let _ = app.emit("rec_error", e);
        drop(child);
        finish(&app, &dir, &prefix);
        return;
    }
    emit_state(&app, "recording");

    // 3) Writer loop: drain event ffmpeg + feed stdin (trừ khi paused) + meter + quét segment.
    let mut seen: HashSet<String> = HashSet::new();
    let mut last_level = Instant::now();
    let mut last_scan = Instant::now();
    while running.load(Ordering::SeqCst) {
        drain_events(&app, &mut events, &running);
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(buf) => {
                // Tạm dừng → không feed ffmpeg (file không tăng) và meter về 0 (báo rõ đang dừng,
                // tránh hiểu nhầm "vẫn ghi" khi nguồn loopback vẫn phát tiếng hệ thống).
                // Ghi màn hình: video ddagrab chạy liên tục → không hỗ trợ pause (luôn active) để khỏi lệch A/V.
                let active = is_screen || !paused.load(Ordering::SeqCst);
                if active {
                    let mut bytes = Vec::with_capacity(buf.len() * 2);
                    for s in &buf {
                        bytes.extend_from_slice(&s.to_le_bytes());
                    }
                    if child.write(&bytes).is_err() {
                        break;
                    }
                }
                if last_level.elapsed() >= Duration::from_millis(33) {
                    let lvl = if active { meter::compute(&buf) } else { meter::compute(&[]) };
                    let _ = app.emit("level", lvl);
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

    // 4) Stop sạch: ngừng capture rồi ĐÓNG STDIN (drop child) → ffmpeg nhận audio EOF. Ghi màn hình
    // dùng `-shortest` nên audio EOF làm ffmpeg kết thúc + finalize MP4 hợp lệ. KHÔNG kill cứng:
    // kill để lại file ~rỗng (fragment chưa flush, "moov atom not found").
    capture.stop();
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
