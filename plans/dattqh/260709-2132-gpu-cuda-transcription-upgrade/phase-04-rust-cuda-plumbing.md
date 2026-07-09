# Phase 04 — Rust CUDA Plumbing (detect + download orchestration + settings)

## Context Links
- Overview: [plan.md](plan.md)
- Depends on: [phase-01](phase-01-cuda-runtime-downloader.md) (protocol `ensure_cuda`/`cuda_ready`), [phase-02](phase-02-gpu-device-selection-sidecar.md) (field `use_gpu`/`cuda_dir` trong cmd `transcribe`), [phase-03](phase-03-model-whisper-large-v3-turbo.md) (đặt trước để `transcribe.rs` không bị 2 phase sửa lệch base)

## Overview
- Priority: P1
- Status: pending
- 3 việc: (1) detect có GPU NVIDIA không (`cuda_supported`, runtime check qua `nvidia-smi`, KHÔNG dùng `cfg!` compile-time như `mlx_supported` vì NVIDIA là hardware-dependent, không xác định lúc build); (2) orchestrate tải CUDA runtime qua sidecar (mirror pattern `ensure_model` đã có) + poll progress; (3) field `use_gpu: bool` trong `AppSettings`, forward vào lệnh `transcribe`.

## Key Insights
- `mlx_supported()` (commands.rs:135-137) dùng `cfg!(all(target_os="macos", target_arch="aarch64"))` — compile-time, ĐÚNG cho Metal (luôn có trên mọi Apple Silicon). NVIDIA GPU thì KHÔNG — máy Windows có thể có/không có GPU NVIDIA, phải check runtime. Không có sẵn cơ chế detect GPU nào trong codebase (đã grep `Dxgi|EnumAdapters` → không có) → dùng `nvidia-smi` (đã cài kèm driver, verified máy user có, thường nằm trong `%SystemRoot%\System32` nên có trên PATH mặc định của mọi máy có driver NVIDIA).
- `transcribe.rs` đã 330 dòng (vượt ngưỡng 200 từ trước task này) — KHÔNG dồn thêm logic CUDA vào đây ngoài 1 method mới `ensure_cuda_runtime` + sửa `transcribe()` để forward field. Phần detect/path-helper tách sang file mới `cuda.rs` để không phình thêm file đã quá tải.
- Poller progress của `ensure_model` (transcribe.rs dòng 267-284) dùng `AtomicBool download_active` CHUNG cho mọi lần tải model. Tải CUDA runtime là tài nguyên KHÁC (không phải model) → dùng cờ riêng `cuda_download_active: Arc<AtomicBool>` trên struct `Transcriber`, tránh 2 tiến trình tải (model + cuda) đụng cờ nhau nếu xảy ra gần nhau.
- Reader thread hiện tại (dòng 238-249) chỉ tắt `dl` (download_active) khi thấy type `"model_ready"|"error"`. Cần thêm match arm cho `"cuda_ready"` → tắt cờ `cuda_download_active` riêng (KHÔNG dùng chung với `dl` — 2 cờ độc lập).
- `total_bytes()` (dòng 14-20) là hàm riêng cho MODEL, không tái dùng được thẳng cho CUDA (khác đơn vị/danh mục) — viết hằng số riêng `CUDA_TOTAL_BYTES: f64 = 1_290_300_000.0` (553.2MB + 737.1MB, theo Phase 01) trong `cuda.rs`.
- Đường dẫn: `cuda_dir(app) = app.path().app_data_dir().join("cuda")` — tương tự `model_dir()` (transcribe.rs dòng 77-87) nhưng đơn giản hơn (không cần case theo size).
- `cuda_runtime_present(app)`: check tồn tại file `cuda_dir(app).join("_cuda_ready.marker")` — tên marker PHẢI khớp `MARKER_NAME` trong `cuda_runtime.py` (Phase 01) — ghi rõ trong comment cả 2 phía để tránh lệch khi 1 bên đổi tên mà quên bên kia (2 codebase Rust/Python không share constant được).
- `AppSettings.use_gpu: bool` — mặc định `false` (`#[serde(default)]`, bool default tự nhiên là false) — KHÔNG tự bật true dù máy có GPU, vì bật GPU lần đầu kéo theo tải ~1.3GB, phải là hành động người dùng chủ động (giống hệt logic "không tự tải model lớn khi chưa được yêu cầu" đã áp dụng cho model).

## Requirements
### Functional
- `cuda_supported() -> bool` (Tauri command, sync): chạy `nvidia-smi --query-gpu=name --format=csv,noheader` (Command mới, `CREATE_NO_WINDOW` trên Windows như đã dùng cho sidecar), exit code 0 + stdout non-empty → true; lỗi spawn/exit≠0/stdout rỗng → false (không throw ra frontend, trả false).
- `cuda_runtime_status(app) -> bool`: true nếu `cuda_runtime_present`.
- `ensure_cuda_runtime(app, transcriber)`: spawn sidecar nếu chưa (`ensure_spawned`), gửi `{"cmd":"ensure_cuda","cuda_dir":...}`, bật `cuda_download_active`, spawn poller thread emit `transcribe_msg` type `cuda_download` (pct) tới khi thấy marker HOẶC cờ tắt (do reader thread thấy `cuda_ready`/`error`).
- `AppSettings.use_gpu: bool` (`#[serde(default)]`) — thêm vào struct + `Default impl`.
- `Transcriber::transcribe()` (dòng 291-322): đọc `settings.use_gpu` (cùng lúc đọc `glossary` dòng 304-307, dùng chung 1 lần `settings::load`), forward `use_gpu` + `cuda_dir` (string, chỉ tính khi `use_gpu=true`, else `None`/bỏ field) vào JSON gửi sidecar.

### Non-functional
- KHÔNG có capability/permission mới cần khai trong `capabilities/default.json` (custom `#[tauri::command]` không cần allowlist riêng — đã verify: file hiện tại chỉ khai permission cho plugin + `shell:allow-execute` cho ffmpeg/ffprobe, không có entry cho command tuỳ biến).
- `cuda.rs` mới < 200 dòng.

## Architecture
```
Frontend invoke("cuda_supported") ──► commands::cuda_supported ──► cuda::gpu_detected() [nvidia-smi]
Frontend invoke("cuda_runtime_status") ──► commands::cuda_runtime_status(app) ──► cuda::cuda_runtime_present(app)
Frontend invoke("ensure_cuda_runtime") ──► commands::ensure_cuda_runtime(app, transcriber)
   └─► transcriber.ensure_cuda_runtime(app)
         ├─ ensure_spawned
         ├─ cuda_download_active.store(true)
         ├─ spawn poller: loop { sleep 1500ms; pct = dir_size(cuda_dir)/CUDA_TOTAL_BYTES*100 (cap 99); emit "cuda_download"; if marker exists → break }
         └─ send {"cmd":"ensure_cuda","cuda_dir": cuda.cuda_dir(app)}
                → sidecar main.py → cuda_runtime.ensure_cuda_runtime(cuda_dir) → send {"type":"cuda_ready"}
                → reader thread (dòng 238-249 hiện tại) thấy "cuda_ready" → cuda_download_active.store(false)

set_settings({..., use_gpu: true}) ──► AppSettings.use_gpu persist trong settings.json

transcribe_files(...) ──► Transcriber::transcribe() đọc settings.use_gpu + cuda.cuda_dir(app)
   └─► gửi sidecar {"cmd":"transcribe", ..., "use_gpu": bool, "cuda_dir": string|null}
```

## Related Code Files
- **Create:** `src-tauri/src/cuda.rs`
- **Modify:**
  - `src-tauri/src/settings.rs` (struct `AppSettings` dòng 9-36, `Default impl` dòng 68-86)
  - `src-tauri/src/transcribe.rs` (struct `Transcriber` dòng 27-31, reader thread dòng 238-249, `transcribe()` dòng 291-322, thêm method `ensure_cuda_runtime`)
  - `src-tauri/src/commands.rs` (thêm 3 command mới, import `crate::cuda`)
  - `src-tauri/src/lib.rs` (`mod cuda;` dòng ~11, `invoke_handler!` thêm 3 command dòng ~50-70)

## Implementation Steps
1. **`cuda.rs` (mới):**
   ```rust
   // Detect GPU NVIDIA (runtime, không phải compile-time — khác mlx_supported) + path CUDA runtime.
   use std::path::PathBuf;
   use std::process::Command;
   use tauri::{AppHandle, Manager};

   #[cfg(windows)]
   use std::os::windows::process::CommandExt;
   #[cfg(windows)]
   const CREATE_NO_WINDOW: u32 = 0x0800_0000;

   pub const CUDA_TOTAL_BYTES: f64 = 1_290_300_000.0; // cublas 553.2MB + cudnn 737.1MB (Phase 01)
   // Tên marker PHẢI khớp MARKER_NAME trong sidecar/transcriber/cuda_runtime.py — đổi 1 bên nhớ đổi bên kia.
   const MARKER_NAME: &str = "_cuda_ready.marker";

   pub fn cuda_dir(app: &AppHandle) -> PathBuf {
       app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from(".")).join("cuda")
   }

   pub fn cuda_runtime_present(app: &AppHandle) -> bool {
       cuda_dir(app).join(MARKER_NAME).is_file()
   }

   /// Có GPU NVIDIA không — CHỈ Windows (nvidia-smi). Non-Windows: false (macOS dùng MLX riêng).
   pub fn gpu_detected() -> bool {
       #[cfg(not(windows))]
       { false }
       #[cfg(windows)]
       {
           let mut cmd = Command::new("nvidia-smi");
           cmd.args(["--query-gpu=name", "--format=csv,noheader"]).creation_flags(CREATE_NO_WINDOW);
           match cmd.output() {
               Ok(out) => out.status.success() && !out.stdout.is_empty(),
               Err(_) => false, // không cài driver NVIDIA / nvidia-smi không có trên PATH
           }
       }
   }
   ```
2. **`settings.rs`:** thêm field vào struct (sau dòng 35 `glossary`):
   ```rust
   #[serde(default)]
   pub use_gpu: bool,
   ```
   Thêm `use_gpu: false,` vào `Default impl` (sau dòng 83 `glossary: default_glossary(),`).
3. **`transcribe.rs` struct `Transcriber`** (dòng 27-31): thêm field
   ```rust
   cuda_download_active: Arc<AtomicBool>,
   ```
4. **Reader thread** (dòng 238-249): sửa match cho phép nhiều loại "ready" tắt đúng cờ của nó:
   ```rust
   match v.get("type").and_then(|x| x.as_str()).unwrap_or("") {
       "model_ready" | "error" => dl.store(false, Ordering::SeqCst),
       "cuda_ready" => cuda_dl.store(false, Ordering::SeqCst),
       _ => {}
   }
   ```
   (cần clone thêm `cuda_dl = self.cuda_download_active.clone()` trước khi `thread::spawn` trong `ensure_spawned`, tương tự `dl` dòng 237).
5. **Method mới `Transcriber::ensure_cuda_runtime`** (đặt sau `ensure_model`, trước `transcribe()`):
   ```rust
   pub fn ensure_cuda_runtime(&self, app: &AppHandle) -> Result<(), String> {
       self.ensure_spawned(app)?;
       let dir = crate::cuda::cuda_dir(app);
       std::fs::create_dir_all(&dir).ok();
       self.cuda_download_active.store(true, Ordering::SeqCst);
       let app2 = app.clone();
       let active = self.cuda_download_active.clone();
       let poll_dir = dir.clone();
       thread::spawn(move || {
           while active.load(Ordering::SeqCst) {
               thread::sleep(Duration::from_millis(1500));
               if !active.load(Ordering::SeqCst) { break; }
               let pct = ((dir_size(&poll_dir) as f64 / crate::cuda::CUDA_TOTAL_BYTES) * 100.0).min(99.0) as u32;
               let _ = app2.emit("transcribe_msg", serde_json::json!({ "type": "cuda_download", "pct": pct }));
               if crate::cuda::cuda_runtime_present(&app2) { break; }
           }
       });
       self.send(
           serde_json::json!({ "cmd": "ensure_cuda", "cuda_dir": dir.to_string_lossy() }).to_string(),
       )
   }
   ```
6. **`transcribe()`** (dòng 291-322): sau đọc `glossary` (dòng 304-307), đọc thêm:
   ```rust
   let settings = crate::settings::load(app).ok();
   let use_gpu = settings.as_ref().map(|s| s.use_gpu).unwrap_or(false);
   let cuda_dir_str = if use_gpu { Some(crate::cuda::cuda_dir(app).to_string_lossy().to_string()) } else { None };
   ```
   (gộp với đọc `prompt` sẵn có để chỉ gọi `settings::load` 1 lần — sửa dòng 304-307 dùng lại biến `settings` này thay vì gọi `load` riêng).
   Thêm `"use_gpu": use_gpu, "cuda_dir": cuda_dir_str` vào JSON gửi sidecar (dòng 314-317).
7. **`commands.rs`:** thêm 3 command:
   ```rust
   #[tauri::command]
   pub fn cuda_supported() -> bool {
       crate::cuda::gpu_detected()
   }

   #[tauri::command]
   pub fn cuda_runtime_status(app: AppHandle) -> bool {
       crate::cuda::cuda_runtime_present(&app)
   }

   #[tauri::command]
   pub fn ensure_cuda_runtime(app: AppHandle, transcriber: State<'_, Transcriber>) -> Result<(), String> {
       transcriber.ensure_cuda_runtime(&app)
   }
   ```
8. **`lib.rs`:** thêm `mod cuda;` (cạnh `mod commands;` dòng 6); thêm 3 command vào `invoke_handler!` (cạnh `commands::mlx_supported,` dòng 63).
9. `cargo check` trong `src-tauri/` → xanh.

## Todo List
- [ ] Tạo `cuda.rs` (gpu_detected, cuda_dir, cuda_runtime_present, CUDA_TOTAL_BYTES, MARKER_NAME)
- [ ] `settings.rs`: field `use_gpu` + Default
- [ ] `transcribe.rs`: field `cuda_download_active`, reader thread match arm, method `ensure_cuda_runtime`, `transcribe()` forward use_gpu/cuda_dir
- [ ] `commands.rs`: 3 command mới
- [ ] `lib.rs`: mod + invoke_handler
- [ ] `cargo check` xanh
- [ ] Test `cuda_supported()` trên máy dev (có GPU) → true; test trên máy giả lập không GPU (rename nvidia-smi tạm hoặc test logic Err branch) → false
- [ ] Test toàn luồng: bật toggle (Phase 06 UI) → `ensure_cuda_runtime` → poll pct tăng → marker xuất hiện → pct 100/done

## Success Criteria
- `cuda_supported()` trả đúng true trên máy có RTX 5070 Ti, false nếu giả lập không có `nvidia-smi`.
- Gọi `ensure_cuda_runtime` → nhận được event `transcribe_msg` type `cuda_download` với pct tăng dần → cuối cùng `cuda_runtime_status()` trả true.
- `transcribe_files` với `use_gpu=true` (đã tải runtime) → sidecar log info "cuda (float16)" (verify chéo với Phase 02).
- `use_gpu=false` (mặc định, chưa đổi setting) → hành vi CPU y hệt trước khi có phase này — regression zero.

## Risk Assessment
| Risk | Likelihood × Impact | Mitigation |
|---|---|---|
| `nvidia-smi` không có trên PATH dù có driver (cài đặt tuỳ biến hiếm) | Thấp × TB | trả `false` an toàn (ẩn option GPU) — không crash; user vẫn dùng CPU bình thường |
| 2 cờ AtomicBool (`download_active` model vs `cuda_download_active`) dùng nhầm nếu code sau này copy-paste sai | TB × TB | đặt tên rõ ràng, comment chỉ rõ "KHÔNG dùng chung" ngay tại field declaration |
| Quên đồng bộ `MARKER_NAME` giữa `cuda.rs` (Rust) và `cuda_runtime.py` (Python) khi sửa sau này | TB × TB | comment chéo ở cả 2 file trỏ tới nhau (đã ghi trong Key Insights + code step 1) |
| `settings::load` gọi 2 lần trong `transcribe()` (1 cho glossary, 1 cho use_gpu) nếu implement cẩu thả | Thấp × Thấp | bước 6 gộp thành 1 lần load, tái dùng biến |

## Security Considerations
- `nvidia-smi` chạy với arg cố định (không nhận input người dùng) → không có injection risk.
- Không thêm bề mặt file-write mới ngoài `cuda_dir` (đã sandbox trong app_data, giống pattern model).

## Next Steps
- Phase 06 UI cần đúng tên 3 command: `cuda_supported`, `cuda_runtime_status`, `ensure_cuda_runtime` + field settings `use_gpu`.
- Phase 07 verify end-to-end trên máy RTX 5070 Ti thật.
