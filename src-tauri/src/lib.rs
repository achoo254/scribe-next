// OneScribe — Tauri v2 entry. Plugins + command registration.
// Commands được thêm dần theo phase (P1: ffmpeg_version, settings).
use tauri::Manager;

mod audio;
mod commands;
mod ffmpeg;
mod recorder;
mod screen;
mod settings;
mod splitter;
mod transcribe;
#[cfg(windows)]
mod webview_permissions;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_fs::init())
        .manage(recorder::Recorder::default())
        .manage(transcribe::Transcriber::default())
        .manage(screen::RegionDrag::default())
        .on_window_event(|window, event| {
            // Đóng cửa sổ chính = thoát hẳn app: exit(0) đóng kèm MỌI overlay (viền/thanh công cụ/
            // chọn vùng). Overlay skip_taskbar nếu còn mở sẽ giữ app sống ngầm (process treo, không
            // icon taskbar). exit(0) cũng kích hoạt RunEvent::Exit bên dưới để dừng ghi + kill sidecar.
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { .. } = event {
                    window.app_handle().exit(0);
                }
            }
        })
        .setup(|_app| {
            // Cấp quyền camera/mic cho WebView2 (Phase 2 — họp video): getUserMedia cần hook này (H2).
            #[cfg(windows)]
            if let Some(win) = _app.get_webview_window("main") {
                if let Err(e) = webview_permissions::grant_media_permissions(&win) {
                    eprintln!("[onescribe] hook quyền media WebView2 lỗi: {e}");
                }
            }
            // Debug: log tình trạng ffmpeg sidecar lúc khởi động (chẩn đoán nhanh).
            // `_app` chỉ dùng trong khối debug này → bản release (debug_assertions off) không cảnh báo unused.
            #[cfg(debug_assertions)]
            {
                let handle = _app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    match commands::ffmpeg_version(handle).await {
                        Ok(v) => eprintln!("[onescribe] ffmpeg OK: {v}"),
                        Err(e) => eprintln!("[onescribe] ffmpeg FAIL: {e}"),
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::ffmpeg_version,
            commands::get_settings,
            commands::set_settings,
            commands::list_devices,
            commands::list_output_devices,
            commands::start_recording,
            commands::pause_recording,
            commands::resume_recording,
            commands::stop_recording,
            commands::probe_file,
            commands::split_files,
            commands::model_status,
            commands::model_path,
            commands::ensure_model,
            commands::transcribe_files,
            commands::cancel_transcribe,
            screen::list_monitors,
            screen::open_region_selector,
            screen::finish_region_selection,
            screen::cancel_region_selection,
            screen::show_record_border,
            screen::hide_record_border,
            screen::show_record_border_toolbar,
            screen::hide_record_border_toolbar,
            screen::clear_record_region,
            screen::begin_region_drag,
            screen::update_region_drag,
            screen::end_region_drag,
        ])
        .build(tauri::generate_context!())
        .expect("error while running OneScribe")
        .run(|handle, event| {
            // Thoát app: dừng ghi (đóng stdin → ffmpeg finalize đoạn cuối rồi thoát) trước, sau đó
            // kill Python sidecar — không để tiến trình con/thread treo lại (tránh rò bộ nhớ +
            // ffmpeg giữ GPU/ổ đĩa khi đang quay màn hình).
            if let tauri::RunEvent::Exit = event {
                handle.state::<recorder::Recorder>().stop_blocking();
                handle.state::<transcribe::Transcriber>().shutdown();
            }
        });
}
