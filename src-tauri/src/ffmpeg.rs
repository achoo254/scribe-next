// Helper resolve ffmpeg/ffprobe sidecar (externalBin). Mọi mode A/B dùng chung.
use tauri::AppHandle;
use tauri_plugin_shell::process::Command;
use tauri_plugin_shell::ShellExt;

/// Tạo Command cho ffmpeg sidecar (đã khai báo externalBin "binaries/ffmpeg").
pub fn ffmpeg_command(app: &AppHandle) -> Result<Command, String> {
    app.shell().sidecar("ffmpeg").map_err(|e| e.to_string())
}

/// Tạo Command cho ffprobe sidecar (probe metadata — dùng ở Mode B/Phase 3).
pub fn ffprobe_command(app: &AppHandle) -> Result<Command, String> {
    app.shell().sidecar("ffprobe").map_err(|e| e.to_string())
}
