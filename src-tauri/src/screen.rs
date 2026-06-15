// Ghi màn hình — liệt kê monitor + overlay "chọn vùng" (Bandicam-style).
// Overlay là 1 WebviewWindow trong suốt phủ đúng monitor; toạ độ kéo (CSS px) × devicePixelRatio
// → px vật lý theo monitor = đúng offset/size cho ddagrab. Mở/đóng overlay đều ở Rust (khỏi cấp
// quyền tạo cửa sổ cho JS); kết quả gửi về cửa sổ chính qua event region_selected/region_cancelled.
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

const OVERLAY_LABEL: &str = "region-selector";

#[derive(serde::Serialize)]
pub struct MonitorInfo {
    pub index: u32,
    pub name: String,
    pub width: u32,  // px vật lý
    pub height: u32,
}

/// Liệt kê monitor (theo thứ tự enumerate — index dùng làm ddagrab output_idx).
#[tauri::command]
pub fn list_monitors(app: AppHandle) -> Vec<MonitorInfo> {
    let mut out = Vec::new();
    if let Some(main) = app.get_webview_window("main") {
        if let Ok(monitors) = main.available_monitors() {
            for (i, m) in monitors.iter().enumerate() {
                let size = m.size();
                out.push(MonitorInfo {
                    index: i as u32,
                    name: m.name().cloned().unwrap_or_else(|| format!("Màn hình {}", i + 1)),
                    width: size.width,
                    height: size.height,
                });
            }
        }
    }
    out
}

/// Mở overlay chọn vùng phủ trọn `monitor`. Người dùng kéo chuột → finish_region_selection.
#[tauri::command]
pub fn open_region_selector(app: AppHandle, monitor: u32) -> Result<(), String> {
    let main = app.get_webview_window("main").ok_or("Không tìm thấy cửa sổ chính")?;
    let monitors = main.available_monitors().map_err(|e| e.to_string())?;
    let m = monitors.get(monitor as usize).ok_or("Màn hình không hợp lệ")?;
    let pos = *m.position();
    let size = *m.size();
    #[cfg(debug_assertions)]
    eprintln!(
        "[onescribe] open_region_selector: monitor={monitor} pos=({},{}) size={}x{}",
        pos.x, pos.y, size.width, size.height
    );

    if let Some(w) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = w.close(); // dọn overlay cũ nếu còn
    }
    let win = WebviewWindowBuilder::new(&app, OVERLAY_LABEL, WebviewUrl::App("index.html".into()))
        .title("Chọn vùng ghi")
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .focused(true)
        .build()
        .map_err(|e| format!("Không tạo được overlay: {e}"))?;
    // Đặt vị trí + kích thước theo px vật lý → phủ đúng monitor (tránh sai lệch do DPI scaling).
    win.set_position(PhysicalPosition::new(pos.x, pos.y)).map_err(|e| e.to_string())?;
    win.set_size(PhysicalSize::new(size.width, size.height)).map_err(|e| e.to_string())?;
    let _ = win.show();
    let _ = win.set_focus();
    // Debug: mở devtools overlay để soi xem content có mount + có lỗi console không (chẩn đoán
    // ca "window tạo OK nhưng vô hình" — nghi render transparent trên WebView2).
    #[cfg(debug_assertions)]
    {
        win.open_devtools();
        eprintln!("[onescribe] open_region_selector: overlay window đã tạo OK (devtools đã mở)");
    }
    Ok(())
}

/// Overlay gọi khi kéo xong: gửi vùng (px vật lý theo monitor) về cửa sổ chính + đóng overlay.
#[tauri::command]
pub fn finish_region_selection(app: AppHandle, x: i32, y: i32, w: u32, h: u32) {
    let _ = app.emit_to("main", "region_selected", serde_json::json!({ "x": x, "y": y, "w": w, "h": h }));
    if let Some(win) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = win.close();
    }
}

/// Overlay gọi khi huỷ (Esc / click không kéo): báo cửa sổ chính + đóng overlay.
#[tauri::command]
pub fn cancel_region_selection(app: AppHandle) {
    let _ = app.emit_to("main", "region_cancelled", ());
    if let Some(win) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = win.close();
    }
}
