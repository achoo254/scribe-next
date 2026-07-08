// Persist cấu hình OneScribe qua tauri-plugin-store (settings.json).
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

const STORE_FILE: &str = "settings.json";
const STORE_KEY: &str = "settings";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppSettings {
    pub mic_id: Option<String>,
    pub out_dir: Option<String>,
    pub format: String,      // "mp3" (mặc định) | "m4a"
    pub split_value: f64,    // mặc định 10
    pub split_unit: String,  // "minutes" | "megabytes"
    // Nguồn ghi + thiết bị output (serde default → tương thích settings cũ thiếu field).
    #[serde(default = "default_source")]
    pub source: String, // "mic" | "system"
    #[serde(default)]
    pub output_id: Option<String>, // tên render device khi source="system"
    // Ghi màn hình (serde default → tương thích settings cũ).
    #[serde(default)]
    pub record_screen: bool,
    #[serde(default)]
    pub screen_monitor: u32, // index monitor (0 = chính)
    #[serde(default = "default_fps")]
    pub screen_fps: u32,
    #[serde(default = "default_region_mode")]
    pub region_mode: String, // "full" | "region"
    #[serde(default)]
    pub region: Option<RegionStore>, // vùng đã chọn lần trước (px vật lý theo monitor)
    // Glossary (initial_prompt ASR) — bias model đúng tên riêng/thuật ngữ. Dùng chung Mode A + C.
    // Mặc định = glossary DevOps (giữ nguyên trải nghiệm cũ); rỗng → transcribe không kèm prompt.
    #[serde(default = "default_glossary")]
    pub glossary: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RegionStore {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

fn default_source() -> String {
    "mic".into()
}

fn default_fps() -> u32 {
    30
}

fn default_region_mode() -> String {
    "full".into()
}

/// Glossary mặc định (domain IT/DevOps) — trước đây hardcode trong sidecar; nay là setting sửa được.
fn default_glossary() -> Option<String> {
    Some(
        "Bài nói kỹ thuật tiếng Việt về hệ thống và DevOps. Thuật ngữ thường gặp: \
VictoriaLogs, VictoriaMetrics, Grafana, OpenTelemetry, observability, log, parse, \
JSON, query, cheatsheet, cluster, HA, Docker, server, port, IP, API, systemctl, rsyslog."
            .into(),
    )
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            mic_id: None,
            out_dir: None,
            format: "mp3".into(),
            split_value: 10.0,
            split_unit: "minutes".into(),
            source: default_source(),
            output_id: None,
            record_screen: false,
            screen_monitor: 0,
            screen_fps: default_fps(),
            region_mode: default_region_mode(),
            region: None,
            glossary: default_glossary(),
        }
    }
}

/// Đọc settings từ store; trả mặc định nếu chưa có / lỗi parse.
pub fn load(app: &AppHandle) -> Result<AppSettings, String> {
    let store = app.store(STORE_FILE).map_err(|e| e.to_string())?;
    match store.get(STORE_KEY) {
        Some(v) => match serde_json::from_value(v) {
            Ok(settings) => Ok(settings),
            Err(e) => {
                // Schema lệch/JSON hỏng → dùng mặc định, nhưng cảnh báo để khỏi mất dữ liệu âm thầm.
                eprintln!("[settings] parse lỗi, dùng mặc định: {e}");
                Ok(AppSettings::default())
            }
        },
        None => Ok(AppSettings::default()),
    }
}

/// Ghi settings vào store + lưu xuống đĩa.
pub fn save(app: &AppHandle, settings: AppSettings) -> Result<(), String> {
    let store = app.store(STORE_FILE).map_err(|e| e.to_string())?;
    let value = serde_json::to_value(&settings).map_err(|e| e.to_string())?;
    store.set(STORE_KEY, value);
    store.save().map_err(|e| e.to_string())
}
