# Ghi màn hình (Mode A mở rộng) — OneScribe

Thêm ghi màn hình (toàn màn hình + chọn vùng kiểu Bandicam) vào tính năng ghi âm hiện có,
xuất MP4 (video + audio mic/hệ thống), vẫn auto-transcribe audio track như Mode A.

## Quyết định chốt (từ spike + Q&A)
- **Windows-first**, license vô tư (nội bộ).
- **KHÔNG đổi binary ffmpeg** — bản LGPL hiện tại đã có `ddagrab` (DXGI) + `libopenh264` + `gdigrab`.
  (Spike đã chạy thật: fullscreen 1080p@30 và region 640×480 → MP4 hợp lệ.)
- Kiến trúc **hybrid**: Rust giữ nguyên capture audio (mic/loopback) → pipe PCM vào ffmpeg stdin;
  ffmpeg tự bắt video qua `ddagrab` (input 0) + mux → segment MP4. Encoder: `libopenh264` (software, mọi máy).
- **Region** dùng trực tiếp `ddagrab offset_x/offset_y/video_size` (không cần crop filter).
- **Chọn vùng**: overlay window trong suốt phủ monitor → kéo chuột → toạ độ × devicePixelRatio (DPI) → px vật lý.
- Stop ghi màn hình = `child.kill()` (ddagrab vô hạn, không EOF qua stdin); frag-MP4 nên file vẫn phát được.
- **Pause TẮT** khi ghi màn hình (2 nguồn live khó đồng bộ pause) — chỉ Ghi/Dừng. (giới hạn v1)

## Ngoài phạm vi v1
macOS/Linux · webcam overlay · pause khi ghi màn hình · map DXGI output đa-GPU (dùng index theo thứ tự monitor) · picker encoder.

## Phases
- [x] **P0 Spike** — xác nhận ddagrab + openh264 + region + 2-input mux (DONE, go).
- [x] **P1 Backend Rust** — `encoder.rs` (VideoConfig + ffmpeg_args_screen), `recorder.rs` (thread video + kill-stop), `commands.rs` (params + guard region), `screen.rs` (monitors + overlay), `lib.rs`, `settings.rs`, capability overlay. (cargo check PASS)
- [x] **P2 Overlay chọn vùng** — `main.ts` route theo window label, `RegionSelectorOverlay.vue` (drag + DPI).
- [x] **P3 Frontend RecordView** — toggle "Ghi kèm màn hình", monitor/FPS, nút Chọn vùng, ẩn format/MB khi screen, clear region khi đổi monitor.
- [x] **P4 Compile + Review** — `cargo check` PASS, `vue-tsc` PASS, code-reviewer DONE_WITH_CONCERNS (đã fix m1/m2/m6). Không có test suite.

## Giới hạn cần verify (chưa che bởi spike single-monitor)
- **M1 — đa màn hình**: `list_monitors` index (winit enumerate) dùng làm ddagrab `output_idx` (DXGI) — 2 thứ tự thường trùng nhưng KHÔNG đảm bảo. Máy 1 màn hình: đúng. Máy ≥2 màn hình: cần test; nếu lệch → map DXGI output đúng.
- **A/V sync** dài hạn: spike 3s OK; cần test bản ghi vài phút trong app thật, tinh chỉnh nếu trôi (`-use_wallclock_as_timestamps`/`-itsoffset`).

## Acceptance
1. Toggle "Ghi kèm màn hình" → chọn monitor + FPS + (toàn màn hình | chọn vùng).
2. Ghi ra MP4 segment có cả video + audio (mic/hệ thống), A/V đồng bộ.
3. Region đúng px dưới DPI scaling 100/125/150%.
4. Auto-transcript chạy trên audio track của MP4 như hiện tại.
5. Không regression ghi âm-only (mp3/m4a) Mode A/B.

## Touchpoints
`src-tauri/src/audio/encoder.rs`, `recorder.rs`, `commands.rs`, `screen.rs`(mới), `lib.rs`, `settings.rs`,
`capabilities/region-selector.json`(mới); `src/main.ts`, `src/views/RegionSelectorOverlay.vue`(mới),
`src/composables/use-recorder.ts`, `src/views/RecordView.vue`.
