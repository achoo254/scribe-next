// Tauri commands cho OneScribe.
// P1: ffmpeg_version (smoke) + settings. P2 (Mode A): list_devices + start/pause/resume/stop.
use crate::audio::capture::{self, DeviceInfo};
use crate::audio::encoder;
use crate::ffmpeg::ffmpeg_command;
use crate::recorder::Recorder;
use crate::settings::{self, AppSettings};
use crate::splitter::{self, MediaInfo};
use crate::transcribe::{self, Transcriber};
use tauri::{AppHandle, State};

/// Liệt kê mic đầu vào (Mode A).
#[tauri::command]
pub fn list_devices() -> Vec<DeviceInfo> {
    capture::list_input_devices()
}

/// Liệt kê output device (render) để ghi "tiếng hệ thống" (loopback). Non-Windows → rỗng.
#[tauri::command]
pub fn list_output_devices() -> Vec<DeviceInfo> {
    #[cfg(windows)]
    {
        crate::audio::loopback::list_output_devices()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

/// Bắt đầu ghi: tính segment_time từ đơn vị (phút | MB) rồi spawn pipeline.
/// `source` = "mic" (cpal) | "system" (loopback output, Windows). `device` = tên thiết bị theo nguồn.
#[tauri::command]
pub fn start_recording(
    app: AppHandle,
    recorder: State<'_, Recorder>,
    source: String,
    device: Option<String>,
    out_dir: String,
    format: String,
    split_value: f64,
    split_unit: String,
) -> Result<(), String> {
    let seg = encoder::segment_time_secs(&split_unit, split_value);
    if seg < 1.0 {
        return Err("Độ dài mỗi đoạn quá nhỏ (< 1 giây)".into());
    }
    recorder.start(&app, source, device, out_dir, format, seg)
}

#[tauri::command]
pub fn pause_recording(app: AppHandle, recorder: State<'_, Recorder>) {
    recorder.pause(&app);
}

#[tauri::command]
pub fn resume_recording(app: AppHandle, recorder: State<'_, Recorder>) {
    recorder.resume(&app);
}

/// Dừng ghi: join thread (chờ ffmpeg finalize) ở blocking pool → không chiếm runtime worker.
#[tauri::command]
pub async fn stop_recording(recorder: State<'_, Recorder>) -> Result<(), String> {
    if let Some(handle) = recorder.take_for_stop() {
        tauri::async_runtime::spawn_blocking(move || {
            let _ = handle.join();
        })
        .await
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Mode B: lấy metadata file (duration/bitrate/codec) để UI hiển thị trước khi cắt.
#[tauri::command]
pub async fn probe_file(app: AppHandle, path: String) -> Result<MediaInfo, String> {
    splitter::probe_file(&app, &path).await
}

/// Mode B: cắt batch file theo tiêu chí (minutes|megabytes|parts). Emit tiến độ qua event.
#[tauri::command]
pub async fn split_files(
    app: AppHandle,
    paths: Vec<String>,
    criterion: String,
    value: f64,
    out_dir: String,
) -> Result<(), String> {
    splitter::split_files(app, paths, criterion, value, out_dir).await
}

/// Mode C: model PhoWhisper (theo size: small|medium|large) đã tải về app-data chưa.
#[tauri::command]
pub fn model_status(app: AppHandle, size: String) -> bool {
    transcribe::model_present(&app, &size)
}

/// Mode C: đường dẫn thư mục model theo size (để hiển thị cho user biết model lưu ở đâu).
#[tauri::command]
pub fn model_path(app: AppHandle, size: String) -> String {
    transcribe::model_dir(&app, &size).to_string_lossy().to_string()
}

/// Mode C: tải model size lần đầu (progress qua event transcribe_msg type=download).
#[tauri::command]
pub fn ensure_model(app: AppHandle, transcriber: State<'_, Transcriber>, size: String) -> Result<(), String> {
    transcriber.ensure_model(&app, &size)
}

/// Mode C: transcribe batch bằng model size đã chọn (giữ ấm). Kết quả qua event.
#[tauri::command]
pub fn transcribe_files(
    app: AppHandle,
    transcriber: State<'_, Transcriber>,
    paths: Vec<String>,
    outputs: Vec<String>,
    size: String,
) -> Result<(), String> {
    transcriber.transcribe(&app, paths, outputs, &size)
}

/// Mode C: hủy (kill sidecar → lần sau spawn lại).
#[tauri::command]
pub fn cancel_transcribe(transcriber: State<'_, Transcriber>) {
    transcriber.shutdown();
}

/// Đọc settings (mic, folder, format, split) — trả mặc định nếu chưa lưu.
#[tauri::command]
pub fn get_settings(app: AppHandle) -> Result<AppSettings, String> {
    settings::load(&app)
}

/// Ghi settings xuống store (persist sau restart).
#[tauri::command]
pub fn set_settings(app: AppHandle, settings: AppSettings) -> Result<(), String> {
    crate::settings::save(&app, settings)
}

/// Smoke test: chạy ffmpeg sidecar `-version`, trả dòng version đầu tiên.
#[tauri::command]
pub async fn ffmpeg_version(app: AppHandle) -> Result<String, String> {
    let output = ffmpeg_command(&app)?
        .args(["-version"])
        .output()
        .await
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.lines().next().unwrap_or("").trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
