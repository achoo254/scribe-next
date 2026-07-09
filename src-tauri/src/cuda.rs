// Detect GPU NVIDIA (runtime, KHÁC mlx_supported vốn compile-time) + path CUDA runtime on-demand.
//
// NVIDIA là hardware phụ thuộc máy — không xác định lúc build như Metal (luôn có trên Apple Silicon)
// → phải check runtime qua nvidia-smi (cài kèm driver, nằm trong System32 → trên PATH mọi máy có driver).
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
use std::process::Command;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

// Chỉ cublas (~553MB tải): ct2 Whisper KHÔNG dùng cuDNN — verified qua EnumProcessModules. Đổi
// CUDA_PACKAGES trong cuda_runtime.py thì sửa số này cho khớp thanh tiến độ.
pub const CUDA_TOTAL_BYTES: f64 = 553_200_000.0;
// Tên marker PHẢI khớp MARKER_NAME trong sidecar/transcriber/cuda_runtime.py — đổi 1 bên nhớ đổi bên kia.
const MARKER_NAME: &str = "_cuda_ready.marker";

/// Thư mục chứa DLL CUDA runtime tải về, dưới <app_data>/cuda/.
pub fn cuda_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("cuda")
}

/// CUDA runtime đã tải + giải nén xong chưa (marker do sidecar ghi sau khi verify + extract).
pub fn cuda_runtime_present(app: &AppHandle) -> bool {
    cuda_dir(app).join(MARKER_NAME).is_file()
}

/// Có GPU NVIDIA không — CHỈ Windows (nvidia-smi). Non-Windows: false (macOS dùng MLX riêng).
pub fn gpu_detected() -> bool {
    #[cfg(not(windows))]
    {
        false
    }
    #[cfg(windows)]
    {
        let mut cmd = Command::new("nvidia-smi");
        cmd.args(["--query-gpu=name", "--format=csv,noheader"])
            .creation_flags(CREATE_NO_WINDOW);
        match cmd.output() {
            Ok(out) => out.status.success() && !out.stdout.is_empty(),
            Err(_) => false, // không cài driver NVIDIA / nvidia-smi không có trên PATH
        }
    }
}
