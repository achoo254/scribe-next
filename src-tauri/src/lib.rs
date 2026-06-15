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
        .setup(|app| {
            // Debug: log tình trạng ffmpeg sidecar lúc khởi động (chẩn đoán nhanh).
            #[cfg(debug_assertions)]
            {
                let handle = app.handle().clone();
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
            screen::show_record_border_close,
            screen::hide_record_border_close,
            screen::clear_record_region,
        ])
        .build(tauri::generate_context!())
        .expect("error while running OneScribe")
        .run(|handle, event| {
            // Kill Python sidecar khi thoát app (tránh process treo + ~1.5GB RAM).
            if let tauri::RunEvent::Exit = event {
                handle.state::<transcribe::Transcriber>().shutdown();
            }
        });
}
