// Ghi màn hình — liệt kê monitor + overlay "chọn vùng" (Bandicam-style).
// Overlay là 1 WebviewWindow trong suốt phủ đúng monitor; toạ độ kéo (CSS px) × devicePixelRatio
// → px vật lý theo monitor = đúng offset/size cho ddagrab. Mở/đóng overlay đều ở Rust (khỏi cấp
// quyền tạo cửa sổ cho JS); kết quả gửi về cửa sổ chính qua event region_selected/region_cancelled.
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

const OVERLAY_LABEL: &str = "region-selector";
const BORDER_LABEL: &str = "record-border";
/// Viền vẽ NGOÀI vùng ghi: cửa sổ nở ra mỗi cạnh ngần này (px vật lý) để ddagrab không chụp phải.
/// Phải ≥ độ dày viền ở RecordBorderOverlay.vue (BORDER_PHYS=3) → chừa 1px an toàn.
const BORDER_INFLATE: i32 = 4;
const CLOSE_LABEL: &str = "record-border-close";
/// Cạnh nút ✕ (px vật lý) ở góc trên-phải khung. Nút CHỈ hiện lúc idle nên không lọt vào video.
const CLOSE_SIZE: i32 = 40;

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
///
/// PHẢI là `async`: trên Windows, tạo WebviewWindow bên trong command ĐỒNG BỘ sẽ deadlock —
/// command sync chạy trên main thread, còn `.build()` cần main event loop xử lý (WebView2 +
/// plugin lock lúc invoke) → kẹt vòng tròn (cửa sổ đen + Not Responding). Command async chạy
/// trên thread của async runtime, để main loop rảnh tạo window bình thường.
#[tauri::command]
pub async fn open_region_selector(app: AppHandle, monitor: u32) -> Result<(), String> {
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
    // Thu nhỏ cửa sổ chính để không che desktop khi chọn vùng (khôi phục ở finish/cancel).
    let _ = main.minimize();
    Ok(())
}

/// Khôi phục cửa sổ chính sau khi đóng overlay (đã thu nhỏ lúc mở chọn vùng).
fn restore_main(app: &AppHandle) {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.unminimize();
        let _ = main.set_focus();
    }
}

/// Overlay gọi khi kéo xong: gửi vùng (px vật lý theo monitor) về cửa sổ chính + đóng overlay.
#[tauri::command]
pub fn finish_region_selection(app: AppHandle, x: i32, y: i32, w: u32, h: u32) {
    let _ = app.emit_to("main", "region_selected", serde_json::json!({ "x": x, "y": y, "w": w, "h": h }));
    if let Some(win) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = win.close();
    }
    restore_main(&app);
}

/// Overlay gọi khi huỷ (Esc / click không kéo): báo cửa sổ chính + đóng overlay.
#[tauri::command]
pub fn cancel_region_selection(app: AppHandle) {
    let _ = app.emit_to("main", "region_cancelled", ());
    if let Some(win) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = win.close();
    }
    restore_main(&app);
}

/// Hiện viền đỏ quanh vùng đang ghi (kiểu Bandicam). Cửa sổ trong suốt + click-through, nở ra
/// BORDER_INFLATE px mỗi cạnh để viền nằm NGOÀI vùng ghi (ddagrab không chụp viền vào video).
/// async vì tạo WebviewWindow — xem ghi chú deadlock ở open_region_selector.
#[tauri::command]
pub async fn show_record_border(
    app: AppHandle,
    monitor: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
) -> Result<(), String> {
    let main = app.get_webview_window("main").ok_or("Không tìm thấy cửa sổ chính")?;
    let monitors = main.available_monitors().map_err(|e| e.to_string())?;
    let m = monitors.get(monitor as usize).ok_or("Màn hình không hợp lệ")?;
    let mpos = *m.position();
    // Toạ độ tuyệt đối = vị trí monitor + offset vùng (px vật lý); nở ra ngoài BORDER_INFLATE mỗi cạnh.
    let abs_x = mpos.x + x - BORDER_INFLATE;
    let abs_y = mpos.y + y - BORDER_INFLATE;
    let win_w = (w as i32 + BORDER_INFLATE * 2).max(1) as u32;
    let win_h = (h as i32 + BORDER_INFLATE * 2).max(1) as u32;

    if let Some(old) = app.get_webview_window(BORDER_LABEL) {
        let _ = old.close();
    }
    let win = WebviewWindowBuilder::new(&app, BORDER_LABEL, WebviewUrl::App("index.html".into()))
        .title("Vùng ghi")
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .focused(false)
        .build()
        .map_err(|e| format!("Không tạo được viền vùng ghi: {e}"))?;
    win.set_position(PhysicalPosition::new(abs_x, abs_y)).map_err(|e| e.to_string())?;
    win.set_size(PhysicalSize::new(win_w, win_h)).map_err(|e| e.to_string())?;
    let _ = win.set_ignore_cursor_events(true); // click-through: không chặn thao tác phía dưới viền
    let _ = win.show();
    Ok(())
}

/// Ẩn viền vùng ghi (khi dừng ghi hoặc về trạng thái idle).
#[tauri::command]
pub fn hide_record_border(app: AppHandle) {
    if let Some(win) = app.get_webview_window(BORDER_LABEL) {
        let _ = win.close();
    }
}

/// Hiện nút ✕ (cửa sổ nhỏ, click được) ở góc trên-phải vùng ghi để đóng khung. CHỈ gọi khi chưa
/// ghi (idle) → nút không lọt vào video. Tách cửa sổ riêng để khung viền vẫn click-through.
/// async vì tạo WebviewWindow — xem ghi chú deadlock ở open_region_selector.
#[tauri::command]
pub async fn show_record_border_close(
    app: AppHandle,
    monitor: u32,
    x: i32,
    y: i32,
    w: u32,
    _h: u32,
) -> Result<(), String> {
    let main = app.get_webview_window("main").ok_or("Không tìm thấy cửa sổ chính")?;
    let monitors = main.available_monitors().map_err(|e| e.to_string())?;
    let m = monitors.get(monitor as usize).ok_or("Màn hình không hợp lệ")?;
    let mpos = *m.position();
    // Nút nằm NGOÀI vùng: mép phải thẳng mép phải vùng, đặt ngay TRÊN mép trên khung. Vùng sát
    // mép trên màn hình (không đủ chỗ phía trên) → kẹp xuống cạnh trên màn hình cho khỏi mất.
    let bx = mpos.x + x + w as i32 - CLOSE_SIZE;
    let by = (mpos.y + y - BORDER_INFLATE - CLOSE_SIZE).max(mpos.y);

    if let Some(old) = app.get_webview_window(CLOSE_LABEL) {
        let _ = old.close();
    }
    let win = WebviewWindowBuilder::new(&app, CLOSE_LABEL, WebviewUrl::App("index.html".into()))
        .title("Đóng khung")
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .focused(false)
        .build()
        .map_err(|e| format!("Không tạo được nút đóng khung: {e}"))?;
    win.set_position(PhysicalPosition::new(bx, by)).map_err(|e| e.to_string())?;
    win.set_size(PhysicalSize::new(CLOSE_SIZE as u32, CLOSE_SIZE as u32)).map_err(|e| e.to_string())?;
    let _ = win.show();
    Ok(())
}

/// Ẩn nút ✕ (khi bắt đầu ghi hoặc khung tắt).
#[tauri::command]
pub fn hide_record_border_close(app: AppHandle) {
    if let Some(win) = app.get_webview_window(CLOSE_LABEL) {
        let _ = win.close();
    }
}

/// Nút ✕ gọi: báo cửa sổ chính xoá vùng + đóng cả khung viền lẫn nút.
#[tauri::command]
pub fn clear_record_region(app: AppHandle) {
    let _ = app.emit_to("main", "region_cleared", ());
    if let Some(win) = app.get_webview_window(BORDER_LABEL) {
        let _ = win.close();
    }
    if let Some(win) = app.get_webview_window(CLOSE_LABEL) {
        let _ = win.close();
    }
}
