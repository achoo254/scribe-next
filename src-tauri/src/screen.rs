// Ghi màn hình — liệt kê monitor + overlay "chọn vùng" (Bandicam-style).
// Overlay là 1 WebviewWindow trong suốt phủ đúng monitor; toạ độ kéo (CSS px) × devicePixelRatio
// → px vật lý theo monitor = đúng offset/size cho ddagrab. Mở/đóng overlay đều ở Rust (khỏi cấp
// quyền tạo cửa sổ cho JS); kết quả gửi về cửa sổ chính qua event region_selected/region_cancelled.
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl, WebviewWindowBuilder};

const OVERLAY_LABEL: &str = "region-selector";
const BORDER_LABEL: &str = "record-border";
/// Viền vẽ NGOÀI vùng ghi: cửa sổ nở ra mỗi cạnh ngần này (px vật lý) để ddagrab không chụp phải.
/// Phải ≥ độ dày viền ở RecordBorderOverlay.vue (BORDER_PHYS=3) → chừa 1px an toàn.
const BORDER_INFLATE: i32 = 4;
/// Thanh công cụ nhỏ (tay nắm kéo ⠿ + nút ✕) phía trên khung. CHỈ hiện lúc idle nên không lọt vào video.
const TOOLBAR_LABEL: &str = "record-border-toolbar";
const TOOLBAR_W: i32 = 96; // px vật lý
const TOOLBAR_H: i32 = 32;

/// Trạng thái phiên kéo khung (giữ tay nắm để di chuyển vùng ghi). Lưu mốc đầu + vị trí hiện tại
/// (px vật lý, toạ độ tương đối monitor) để kẹp trong màn hình và báo vùng cuối về cửa sổ chính.
#[derive(Default)]
pub struct RegionDrag(pub Mutex<Option<DragStart>>);

pub struct DragStart {
    monitor_pos: PhysicalPosition<i32>,
    monitor_size: PhysicalSize<u32>,
    start_x: i32,
    start_y: i32,
    cur_x: i32,
    cur_y: i32,
    region_w: u32,
    region_h: u32,
}

/// Toạ độ + kích thước cửa sổ viền (nở BORDER_INFLATE mỗi cạnh) từ vùng (px vật lý, rel monitor).
fn border_rect(mpos: PhysicalPosition<i32>, x: i32, y: i32, w: u32, h: u32) -> (i32, i32, u32, u32) {
    let abs_x = mpos.x + x - BORDER_INFLATE;
    let abs_y = mpos.y + y - BORDER_INFLATE;
    let win_w = (w as i32 + BORDER_INFLATE * 2).max(1) as u32;
    let win_h = (h as i32 + BORDER_INFLATE * 2).max(1) as u32;
    (abs_x, abs_y, win_w, win_h)
}

/// Vị trí thanh công cụ: mép phải thẳng mép phải vùng, nằm ngay TRÊN mép trên khung (kẹp trong màn hình).
fn toolbar_pos(mpos: PhysicalPosition<i32>, x: i32, y: i32, w: u32) -> (i32, i32) {
    let tx = (mpos.x + x + w as i32 - TOOLBAR_W).max(mpos.x);
    let ty = (mpos.y + y - BORDER_INFLATE - TOOLBAR_H).max(mpos.y);
    (tx, ty)
}

/// Dời cửa sổ viền + thanh công cụ tới vùng mới mà KHÔNG tạo lại cửa sổ (mượt khi kéo).
fn reposition_border_windows(app: &AppHandle, mpos: PhysicalPosition<i32>, x: i32, y: i32, w: u32, h: u32) {
    if let Some(b) = app.get_webview_window(BORDER_LABEL) {
        let (bx, by, bw, bh) = border_rect(mpos, x, y, w, h);
        let _ = b.set_position(PhysicalPosition::new(bx, by));
        let _ = b.set_size(PhysicalSize::new(bw, bh));
    }
    if let Some(t) = app.get_webview_window(TOOLBAR_LABEL) {
        let (tx, ty) = toolbar_pos(mpos, x, y, w);
        let _ = t.set_position(PhysicalPosition::new(tx, ty));
    }
}

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
    let (abs_x, abs_y, win_w, win_h) = border_rect(mpos, x, y, w, h);

    // Đã có cửa sổ → chỉ dời/đổi cỡ tại chỗ (mượt, tránh nháy khi đổi vùng hoặc đồng bộ sau khi kéo).
    if let Some(win) = app.get_webview_window(BORDER_LABEL) {
        let _ = win.set_position(PhysicalPosition::new(abs_x, abs_y));
        let _ = win.set_size(PhysicalSize::new(win_w, win_h));
        let _ = win.show();
        return Ok(());
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

/// Hiện thanh công cụ (cửa sổ nhỏ, click được): tay nắm KÉO để di chuyển vùng + nút ✕ đóng khung.
/// Nằm ngay TRÊN mép trên khung, ngoài vùng ghi. CHỈ gọi khi chưa ghi (idle) → không lọt vào video.
/// Tách cửa sổ riêng để khung viền vẫn click-through. async vì tạo WebviewWindow — xem ghi chú
/// deadlock ở open_region_selector.
#[tauri::command]
pub async fn show_record_border_toolbar(
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
    let (tx, ty) = toolbar_pos(mpos, x, y, w);

    // Đã có cửa sổ → chỉ dời tại chỗ (mượt khi đổi vùng / kéo).
    if let Some(win) = app.get_webview_window(TOOLBAR_LABEL) {
        let _ = win.set_position(PhysicalPosition::new(tx, ty));
        let _ = win.show();
        return Ok(());
    }
    let win = WebviewWindowBuilder::new(&app, TOOLBAR_LABEL, WebviewUrl::App("index.html".into()))
        .title("Công cụ vùng ghi")
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .focused(false)
        .build()
        .map_err(|e| format!("Không tạo được thanh công cụ vùng ghi: {e}"))?;
    win.set_position(PhysicalPosition::new(tx, ty)).map_err(|e| e.to_string())?;
    win.set_size(PhysicalSize::new(TOOLBAR_W as u32, TOOLBAR_H as u32)).map_err(|e| e.to_string())?;
    let _ = win.show();
    Ok(())
}

/// Ẩn thanh công cụ (khi bắt đầu ghi hoặc khung tắt).
#[tauri::command]
pub fn hide_record_border_toolbar(app: AppHandle) {
    if let Some(win) = app.get_webview_window(TOOLBAR_LABEL) {
        let _ = win.close();
    }
}

/// Nút ✕ gọi: báo cửa sổ chính xoá vùng + đóng cả khung viền lẫn thanh công cụ.
#[tauri::command]
pub fn clear_record_region(app: AppHandle) {
    let _ = app.emit_to("main", "region_cleared", ());
    if let Some(win) = app.get_webview_window(BORDER_LABEL) {
        let _ = win.close();
    }
    if let Some(win) = app.get_webview_window(TOOLBAR_LABEL) {
        let _ = win.close();
    }
}

/// Bắt đầu kéo khung: chốt mốc vùng + thông số monitor đọc từ cửa sổ viền hiện có.
#[tauri::command]
pub fn begin_region_drag(app: AppHandle, drag: State<RegionDrag>) -> Result<(), String> {
    let border = app.get_webview_window(BORDER_LABEL).ok_or("Chưa có khung vùng ghi")?;
    let bpos = border.outer_position().map_err(|e| e.to_string())?;
    let bsize = border.outer_size().map_err(|e| e.to_string())?;
    let mon = border
        .current_monitor()
        .map_err(|e| e.to_string())?
        .ok_or("Không xác định được màn hình")?;
    let mpos = *mon.position();
    let msize = *mon.size();
    // Vùng = cửa sổ viền trừ phần nở BORDER_INFLATE mỗi cạnh.
    let region_w = bsize.width.saturating_sub((BORDER_INFLATE * 2) as u32);
    let region_h = bsize.height.saturating_sub((BORDER_INFLATE * 2) as u32);
    let x = bpos.x - mpos.x + BORDER_INFLATE;
    let y = bpos.y - mpos.y + BORDER_INFLATE;
    *drag.0.lock().unwrap() = Some(DragStart {
        monitor_pos: mpos,
        monitor_size: msize,
        start_x: x,
        start_y: y,
        cur_x: x,
        cur_y: y,
        region_w,
        region_h,
    });
    Ok(())
}

/// Cập nhật khi kéo: dx/dy là TỔNG độ dời (px vật lý) so với lúc bắt đầu; kẹp vùng trong monitor.
#[tauri::command]
pub fn update_region_drag(app: AppHandle, drag: State<RegionDrag>, dx: i32, dy: i32) {
    let mut guard = drag.0.lock().unwrap();
    if let Some(s) = guard.as_mut() {
        let max_x = (s.monitor_size.width as i32 - s.region_w as i32).max(0);
        let max_y = (s.monitor_size.height as i32 - s.region_h as i32).max(0);
        s.cur_x = (s.start_x + dx).clamp(0, max_x);
        s.cur_y = (s.start_y + dy).clamp(0, max_y);
        reposition_border_windows(&app, s.monitor_pos, s.cur_x, s.cur_y, s.region_w, s.region_h);
    }
}

/// Kết thúc kéo: báo vùng mới (rel monitor) về cửa sổ chính để lưu + dùng cho lần ghi sau.
#[tauri::command]
pub fn end_region_drag(app: AppHandle, drag: State<RegionDrag>) {
    if let Some(s) = drag.0.lock().unwrap().take() {
        let _ = app.emit_to(
            "main",
            "region_moved",
            serde_json::json!({ "x": s.cur_x, "y": s.cur_y, "w": s.region_w, "h": s.region_h }),
        );
    }
}
