// Helper cho ffmpeg/ffprobe sidecar (externalBin).
// - Mode A (ghi) + smoke test: spawn qua Tauri shell sidecar (`ffmpeg_command`).
// - Mode B (cắt): uỷ thác crate `core-split` (spawn tokio) → cần PATH tường minh của sidecar
//   (`sidecar_path`) vì `core-split` không phụ thuộc Tauri.
use tauri::utils::platform;
use tauri::AppHandle;
use tauri_plugin_shell::process::Command;
use tauri_plugin_shell::ShellExt;

/// Tạo Command cho ffmpeg sidecar (đã khai báo externalBin "binaries/ffmpeg").
pub fn ffmpeg_command(app: &AppHandle) -> Result<Command, String> {
    app.shell().sidecar("ffmpeg").map_err(|e| e.to_string())
}

/// Đường dẫn tuyệt đối tới sidecar (vd "ffmpeg"/"ffprobe") để truyền cho `core-split` (spawn tokio).
///
/// Tái tạo ĐÚNG `tauri_plugin_shell::process::relative_command_path`: cạnh binary hiện tại, lùi khỏi
/// `deps/` khi chạy dev, thêm `.exe` trên Windows. Giữ khớp để đường dẫn y hệt cái `app.shell().sidecar()`
/// tự resolve — tránh lệch giữa Mode A (Tauri spawn) và Mode B (tokio spawn) cùng dùng 1 binary.
pub fn sidecar_path(name: &str) -> Result<String, String> {
    let exe = platform::current_exe().map_err(|e| e.to_string())?;
    let exe_dir = exe.parent().ok_or("binary hiện tại không có thư mục cha")?;
    // dev (cargo): binary nằm trong `target/<profile>/deps/` → sidecar ở thư mục cha (`target/<profile>/`).
    let base_dir = if exe_dir.ends_with("deps") {
        exe_dir.parent().unwrap_or(exe_dir)
    } else {
        exe_dir
    };
    #[allow(unused_mut)] // `mut` chỉ dùng ở nhánh #[cfg(windows)] bên dưới (thêm .exe).
    let mut path = base_dir.join(name);
    #[cfg(windows)]
    {
        let already_exe = path.extension().is_some_and(|ext| ext == "exe");
        if !already_exe {
            path.as_mut_os_string().push(".exe");
        }
    }
    Ok(path.to_string_lossy().into_owned())
}
