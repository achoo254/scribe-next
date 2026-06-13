// Mode C — vòng đời Python sidecar faster-whisper (persistent, giữ ấm model).
// Dùng std::process (không tauri shell) để: write stdin + read stdout + đóng EOF + kill sạch.
// Dev: chạy .venv/Scripts/python.exe main.py. Release: binary PyInstaller transcriber-<target>.
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

// Tổng dung lượng model (model.bin 3086.9MB + vocab ~1MB, verify từ HF). Cap 99% tới khi model_ready.
const MODEL_TOTAL_BYTES: f64 = 3_088_000_000.0;

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Default)]
pub struct Transcriber {
    inner: Mutex<Option<Proc>>,
    download_active: Arc<AtomicBool>,
}

struct Proc {
    child: Child,
    stdin: ChildStdin,
}

impl Drop for Proc {
    fn drop(&mut self) {
        let _ = self.stdin.write_all(b"{\"cmd\":\"shutdown\"}\n");
        let _ = self.child.kill();
        let _ = self.child.wait(); // reap (tránh zombie trên Unix/macOS — P5)
    }
}

/// Thư mục model trong app-data: <app_data>/models/PhoWhisper-large-ct2.
pub fn model_dir(app: &AppHandle) -> PathBuf {
    let base = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    base.join("models").join("PhoWhisper-large-ct2")
}

fn model_present_at(d: &Path) -> bool {
    d.join("config.json").exists()
        && std::fs::metadata(d.join("model.bin"))
            .map(|m| m.len() > 0)
            .unwrap_or(false)
}

/// Model đã tải ĐỦ chưa: config.json + model.bin (kích thước > 0) — tránh model tải dở qua mặt.
pub fn model_present(app: &AppHandle) -> bool {
    model_present_at(&model_dir(app))
}

/// Tổng dung lượng thư mục (gồm cả file .incomplete đang tải) → tính % tải.
fn dir_size(dir: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                total += dir_size(&p);
            } else if let Ok(m) = e.metadata() {
                total += m.len();
            }
        }
    }
    total
}

fn find_script() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    for anc in exe.ancestors() {
        let cand = anc.join("sidecar").join("transcriber").join("main.py");
        if cand.exists() {
            return Some(cand);
        }
    }
    None
}

/// (program, args): ưu tiên binary PyInstaller (release), fallback venv python + main.py (dev).
fn resolve(app: &AppHandle) -> Result<(PathBuf, Vec<String>), String> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for name in ["transcriber-x86_64-pc-windows-msvc.exe", "transcriber.exe", "transcriber"] {
                let bin = dir.join(name);
                if bin.exists() {
                    return Ok((bin, vec![]));
                }
            }
        }
    }
    let _ = app;
    let script = find_script().ok_or("Không tìm thấy sidecar transcriber (main.py)")?;
    let venv_py = script
        .parent()
        .map(|d| d.join(".venv").join("Scripts").join("python.exe"))
        .filter(|p| p.exists());
    let program = venv_py.unwrap_or_else(|| PathBuf::from("python"));
    Ok((program, vec![script.to_string_lossy().to_string()]))
}

impl Transcriber {
    /// Spawn sidecar nếu chưa chạy (hoặc đã chết). Reader thread đọc stdout JSON → emit event.
    pub fn ensure_spawned(&self, app: &AppHandle) -> Result<(), String> {
        let mut g = self.inner.lock().map_err(|e| e.to_string())?;
        if let Some(p) = g.as_mut() {
            if matches!(p.child.try_wait(), Ok(None)) {
                return Ok(()); // còn sống
            }
        }
        let (program, args) = resolve(app)?;
        let mut cmd = Command::new(&program);
        cmd.args(&args).stdin(Stdio::piped()).stdout(Stdio::piped());
        // Debug: hiện stderr sidecar (chẩn đoán crash khởi động). Release: ẩn.
        #[cfg(debug_assertions)]
        cmd.stderr(Stdio::inherit());
        #[cfg(not(debug_assertions))]
        cmd.stderr(Stdio::null());
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        let mut child = cmd.spawn().map_err(|e| format!("Không chạy được sidecar: {e}"))?;
        let stdin = child.stdin.take().ok_or("Không mở được stdin sidecar")?;
        let stdout = child.stdout.take().ok_or("Không mở được stdout sidecar")?;

        let app2 = app.clone();
        let dl = self.download_active.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                    match v.get("type").and_then(|x| x.as_str()).unwrap_or("") {
                        "model_ready" | "error" => dl.store(false, Ordering::SeqCst),
                        _ => {}
                    }
                    let _ = app2.emit("transcribe_msg", v);
                }
            }
        });

        *g = Some(Proc { child, stdin });
        Ok(())
    }

    fn send(&self, line: String) -> Result<(), String> {
        let mut g = self.inner.lock().map_err(|e| e.to_string())?;
        let p = g.as_mut().ok_or("Sidecar chưa khởi động")?;
        p.stdin.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
        p.stdin.write_all(b"\n").map_err(|e| e.to_string())?;
        p.stdin.flush().map_err(|e| e.to_string())
    }

    pub fn ensure_model(&self, app: &AppHandle) -> Result<(), String> {
        self.ensure_spawned(app)?;
        let dir = model_dir(app);
        // Poller tiến độ: poll dung lượng thư mục / tổng ước lượng → emit download pct.
        self.download_active.store(true, Ordering::SeqCst);
        let app2 = app.clone();
        let active = self.download_active.clone();
        let poll_dir = dir.clone();
        thread::spawn(move || {
            while active.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(1500));
                if !active.load(Ordering::SeqCst) {
                    break;
                }
                let pct = ((dir_size(&poll_dir) as f64 / MODEL_TOTAL_BYTES) * 100.0).min(99.0) as u32;
                let _ = app2.emit("transcribe_msg", serde_json::json!({ "type": "download", "pct": pct }));
                if model_present_at(&poll_dir) {
                    break;
                }
            }
        });
        self.send(
            serde_json::json!({ "cmd": "ensure_model", "model_dir": dir.to_string_lossy() }).to_string(),
        )
    }

    pub fn transcribe(&self, app: &AppHandle, paths: Vec<String>, outputs: Vec<String>) -> Result<(), String> {
        if !model_present(app) {
            return Err("Chưa tải model — hãy tải model trước".into());
        }
        self.ensure_spawned(app)?;
        let dir = model_dir(app).to_string_lossy().to_string();
        for file in paths {
            let out_dir = Path::new(&file)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| ".".to_string());
            self.send(
                serde_json::json!({
                    "cmd": "transcribe", "file": file, "out_dir": out_dir,
                    "outputs": outputs, "model_dir": dir,
                })
                .to_string(),
            )?;
        }
        Ok(())
    }

    /// Hủy: kill sidecar (faster-whisper blocking không hủy giữa chừng được) — lần sau tự spawn lại.
    pub fn shutdown(&self) {
        if let Ok(mut g) = self.inner.lock() {
            g.take(); // Drop for Proc → shutdown + kill
        }
    }
}
