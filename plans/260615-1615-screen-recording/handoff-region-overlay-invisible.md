# Handoff — Ghi màn hình Scribe (tiếp tục máy khác)

> Ngày: 2026-06-15. Hết giờ làm, commit để pull tiếp máy khác.
> Trạng thái: **Feature ghi màn hình ĐÃ implement + compile + review xong. CÒN 1 bug đang debug:
> overlay "Chọn vùng" tạo được nhưng VÔ HÌNH.**

## 1. Đã hoàn thành (verified)
Ghi màn hình MP4 trong Mode A (hybrid: Rust giữ capture audio → pipe vào ffmpeg; ffmpeg thêm
input video `ddagrab` (DXGI) → mux MP4 segment; encode `libopenh264`). Toggle "Ghi kèm màn hình".
- Spike ffmpeg thật: fullscreen + region + 2-input segment mux → MP4 hợp lệ (video+audio). ✅
- `cargo check` ✅ · `vue-tsc --noEmit` ✅ · `code-reviewer` DONE_WITH_CONCERNS (đã fix m1/m2/m6).
- **Ghi toàn màn hình NHIỀU KHẢ NĂNG đã chạy** (không phụ thuộc overlay) — CHƯA test trong app thật.
- Auto-transcript audio track MP4: dùng lại pipeline cũ (chưa test runtime).

## 2. BUG đang dở — overlay "Chọn vùng" vô hình
**Triệu chứng:** bật "Ghi kèm màn hình" → "Chọn vùng" → bấm "Chọn vùng…" → màn hình KHÔNG mờ, không thấy khung kéo.

**Bằng chứng đã thu (user xác nhận):** terminal `tauri dev` IN `[scribe] open_region_selector: ... overlay window đã tạo OK` và KHÔNG có alert lỗi.
→ Command Rust chạy OK, `WebviewWindowBuilder::build()` thành công, set_position/size OK.
→ **Root cause khoanh vùng: cửa sổ overlay TRONG SUỐT không render nội dung (nghi WebView2 transparent trên Windows) — KHÔNG phải lỗi command/IPC.**

Phần đã loại trừ: lỗi invoke (đã surface ra UI, không có alert); command không chạy (đã có log OK);
Rust chưa rebuild (user đã chạy lại).

## 3. Bước tiếp theo (làm trên máy khác) — theo thứ tự
Đã thêm sẵn **devtools auto-open cho overlay (chỉ debug)** trong `screen.rs` + log `[scribe] mounting
RegionSelectorOverlay` trong `main.ts`. Khi chạy `npm run tauri dev` và bấm "Chọn vùng…":

1. **Devtools overlay sẽ tự mở** → kiểm tra:
   - Console có in `[scribe] mounting RegionSelectorOverlay (label=region-selector)` không?
     - KHÔNG → `main.ts` mount nhầm `App` (label != "region-selector") → sửa logic route theo label.
     - CÓ → component mount OK → vào (2).
   - Elements tab: `<div class="rs-root">` có tồn tại + có `background: rgba(0,0,0,0.35)` không?
     - Có element nhưng nhìn xuyên thấu → **đúng là transparent render fail** → thử fix mục (4).

2. **Fix ưu tiên (transparent WebView2):** thử lần lượt:
   - (a) Set màu nền webview trong suốt tường minh: WebView2 cần `DefaultBackgroundColor=Transparent`.
     Tauri v2: thử builder `.background_color(tauri::webview::Color(0,0,0,1))` (gần như đen, alpha thấp)
     HOẶC dùng `tauri::window::Color` transparent. Nếu API không có → dùng `with_webview` set
     `ICoreWebView2Controller2.DefaultBackgroundColor`.
   - (b) Tạm bỏ `transparent(true)`, để nền `.rs-root` mờ đậm hơn (rgba 0.5) xác nhận window HIỂN THỊ
     trước (loại trừ vấn đề vị trí/size). Nếu opaque mà hiện → chắc chắn lỗi transparent.
   - (c) Khai báo window trong `tauri.conf.json` với `"transparent": true` thay vì tạo runtime, xem khác không.
   - (d) Đổi cách chọn vùng: KHÔNG dùng overlay trong suốt — dùng dialog nhập toạ độ, hoặc thư viện
     chọn vùng native. (phương án dự phòng nếu transparent quá khó trên WebView2)

3. Sau khi overlay hiện: test kéo chọn vùng → kiểm tra `region_selected` về đúng px vật lý (DPI 125/150%).

4. **Gỡ diagnostic** trước khi chốt: bỏ `win.open_devtools()` + 2 dòng `eprintln!`/`console.log` đã thêm.

## 4. Verify còn thiếu (cần app thật)
- A/V sync bản ghi vài phút (spike 3s OK). Nếu trôi → `-use_wallclock_as_timestamps`/`-itsoffset` trong `encoder.rs`.
- Đa màn hình: `list_monitors` index → `ddagrab output_idx`; 1 màn hình đúng, ≥2 cần test.
- Auto-transcript trên file MP4 (whisper đọc audio track).

## 5. Files đã đổi/ mới
**Backend:** `src-tauri/src/audio/encoder.rs` (VideoConfig + ffmpeg_args_screen), `recorder.rs` (luồn video + kill-stop),
`commands.rs` (params + RegionArg + guard region), `lib.rs` (đăng ký), `settings.rs` (field mới), `screen.rs` (MỚI: monitor + overlay),
`capabilities/region-selector.json` (MỚI).
**Frontend:** `src/main.ts` (route theo window label), `src/views/RegionSelectorOverlay.vue` (MỚI: kéo + DPI),
`src/composables/use-recorder.ts` (monitors/region/openRegionSelector + surface lỗi), `src/views/RecordView.vue` (UI toggle/monitor/FPS/region).
**Docs/Plan:** `README.md`, `plans/260615-1615-screen-recording/plan.md` + report này.

## 6. Resume nhanh
```
git pull
npm install            # nếu cần
npm run tauri dev      # Rust rebuild ~1-2 phút lần đầu
# Tab Ghi âm → bật "Ghi kèm màn hình" → "Chọn vùng" → "Chọn vùng…" → xem devtools overlay
```
