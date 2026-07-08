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

// Dung lượng model.bin ước tính theo size (float16 storage) → tính % tải. Cap 99% tới khi model_ready.
fn total_bytes(size: &str) -> f64 {
    match size {
        "small" => 490_000_000.0,
        "medium" => 1_540_000_000.0,
        _ => 3_088_000_000.0, // large
    }
}

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

/// Thư mục model theo size, dưới <app_data>/models/.
///
/// small/medium dùng leaf ngắn ("small"/"medium"). Lý do: snapshot_download stage file tải vào
/// <model_dir>/.cache/huggingface/download/<subfolder-repo>/<tên-hash>.incomplete. Subfolder repo
/// của 2 size này dài (PhoWhisper-<size>-ct2-fasterWhisper) + tên file hash của huggingface_hub
/// đẩy tổng đường dẫn chạm 260 = MAX_PATH Windows → open() lỗi [Errno 2] khi long-path chưa bật.
/// Leaf ngắn giữ path < 260 trên mọi máy, không phụ thuộc registry LongPathsEnabled.
///
/// large giữ "PhoWhisper-large-ct2": repo large KHÔNG có subfolder nên path vốn đã đủ ngắn,
/// đồng thời tái dùng bản ~3GB có thể đã tải sẵn, khỏi tải lại.
pub fn model_dir(app: &AppHandle, size: &str) -> PathBuf {
    let base = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    let leaf = match size {
        "large" => "PhoWhisper-large-ct2",
        other => other, // "small" | "medium" → leaf ngắn, tránh vượt MAX_PATH
    };
    base.join("models").join(leaf)
}

/// Có file trọng số model (>0 byte) ở đâu đó dưới dir không.
/// ct2 (small/medium/large): model.bin (small/medium nằm trong subfolder repo).
/// MLX (large-v3-mlx): weights.npz / weights.safetensors.
fn has_model_weights(dir: &Path) -> bool {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                if has_model_weights(&p) {
                    return true;
                }
            } else if p
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| matches!(n, "model.bin" | "weights.npz" | "weights.safetensors"))
                .unwrap_or(false)
                && std::fs::metadata(&p).map(|m| m.len() > 0).unwrap_or(false)
            {
                return true;
            }
        }
    }
    false
}

/// Model đã tải ĐỦ chưa: tìm file trọng số (>0 byte) — tránh model tải dở qua mặt.
pub fn model_present(app: &AppHandle, size: &str) -> bool {
    has_model_weights(&model_dir(app, size))
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

    pub fn ensure_model(&self, app: &AppHandle, size: &str) -> Result<(), String> {
        self.ensure_spawned(app)?;
        let dir = model_dir(app, size);
        let total = total_bytes(size);
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
                let pct = ((dir_size(&poll_dir) as f64 / total) * 100.0).min(99.0) as u32;
                let _ = app2.emit("transcribe_msg", serde_json::json!({ "type": "download", "pct": pct }));
                if has_model_weights(&poll_dir) {
                    break;
                }
            }
        });
        self.send(
            serde_json::json!({ "cmd": "ensure_model", "model_dir": dir.to_string_lossy(), "size": size })
                .to_string(),
        )
    }

    pub fn transcribe(
        &self,
        app: &AppHandle,
        paths: Vec<String>,
        outputs: Vec<String>,
        size: &str,
    ) -> Result<(), String> {
        if !model_present(app, size) {
            return Err("Chưa tải model — hãy tải model trước".into());
        }
        self.ensure_spawned(app)?;
        let dir = model_dir(app, size).to_string_lossy().to_string();
        for file in paths {
            let out_dir = Path::new(&file)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| ".".to_string());
            self.send(
                serde_json::json!({
                    "cmd": "transcribe", "file": file, "out_dir": out_dir,
                    "outputs": outputs, "model_dir": dir, "size": size,
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
