---
title: "Ghi tiếng output hệ thống (loopback) — Windows"
description: "Tùy chọn nguồn ghi: Mic (cpal) hoặc Tiếng hệ thống (WASAPI loopback). Output-only, Windows-first."
status: completed
priority: P2
created: "2026-06-15"
tags: [tauri, rust, audio, wasapi, loopback, mode-a]
---

# Ghi tiếng output hệ thống (loopback)

> **Đóng plan (2026-10-19):** đã giao trên app desktop (Windows); app desktop bỏ ngày 2026-10-16. Web ghi tiếng hệ thống
> qua chia sẻ màn hình của Chrome.

## Overview

Thêm tùy chọn **Nguồn ghi** trong tab Ghi âm: **Mic** (cpal, như cũ) hoặc **Tiếng hệ thống**
(âm thanh đang phát ra loa/tai nghe — WASAPI loopback).

**Scope chốt với người dùng (2026-06-15)**:
- **Output-only** (KHÔNG trộn mic+output).
- **Windows-first** (macOS để sau — sẽ cần ScreenCaptureKit, API khác hẳn).

## Key insight

- cpal 0.15 **không** hỗ trợ WASAPI loopback → dùng crate `wasapi` 0.23 (Windows-only dep).
- Loopback = mở **Render device** rồi `initialize_client(.., &Direction::Capture, &EventsShared{autoconvert:true})`
  → wasapi tự bật `AUDCLNT_STREAMFLAGS_LOOPBACK` (xác minh tại api.rs:1031 của crate).
- Request format **i16 PCM 48k/2 + autoconvert** → WASAPI tự convert (float→i16, resample) →
  không cần xử lý mẫu thủ công. Cùng kiểu `Vec<i16>` như mic → downstream ffmpeg KHÔNG đổi.

## Architecture

```
RecordView (toggle Nguồn ghi + select thiết bị)
   └─ start_recording { source: "mic"|"system", device, ... }
        └─ recorder.rs record_loop → source::build_capture(source, device, tx, on_error)
             ├─ "mic"    → capture::build_stream (cpal Stream, !Send, trên thread ghi)
             └─ "system" → loopback::start_loopback_capture (WASAPI, thread riêng + COM/MTA)
                   → Vec<i16> qua chung 1 channel → ffmpeg stdin (segment muxer) [KHÔNG đổi]
```

- `RunningCapture` (audio/source.rs): enum thống nhất Cpal(Stream) | Loopback{stop,handle}; play()/stop().
- Loopback chạy thread riêng (COM apartment-thread): mở device → báo CaptureConfig qua channel → vòng lặp
  `read_from_device_to_deque` + `wait_for_event(200ms)` (timeout ngắn để kiểm cờ stop) → push i16.

## Related code files

- Create: `src-tauri/src/audio/loopback.rs` (Windows-only) — list_output_devices + start_loopback_capture.
- Create: `src-tauri/src/audio/source.rs` — RunningCapture + build_capture (chọn mic|loopback).
- Modify: `src-tauri/src/audio/mod.rs` (khai báo module), `Cargo.toml` (dep wasapi target windows).
- Modify: `src-tauri/src/recorder.rs` (record_loop dùng source::build_capture; play/stop thống nhất).
- Modify: `src-tauri/src/commands.rs` (list_output_devices + start_recording nhận source/device).
- Modify: `src-tauri/src/lib.rs` (đăng ký list_output_devices), `src-tauri/src/settings.rs` (source + output_id, serde default).
- Modify: `src/composables/use-recorder.ts` (outputDevices + start{source,device}), `src/views/RecordView.vue` (toggle + select dùng chung).

## Todo

- [x] Dep wasapi 0.23 (target cfg windows) + module loopback.rs (đối chiếu API thật trong crate source)
- [x] source.rs trừu tượng RunningCapture + tích hợp recorder.rs
- [x] commands/lib/settings (list_output_devices, source/device, persist)
- [x] Frontend: toggle Nguồn ghi + select thiết bị theo nguồn + persist
- [x] `cargo check` EXIT=0 (fix Arc move vào closure) + `vue-tsc` EXIT=0
- [ ] **Test thật trên Windows**: chọn "Tiếng hệ thống", phát nhạc/video → ghi ra file có tiếng output;
      thử default + chọn render device cụ thể; auto-transcript chạy trên segment loopback
- [ ] MCP design system validate_brand_usage trên RecordView.vue

## Success criteria

- Toggle "Tiếng hệ thống" → liệt kê render device; ghi ra file chứa âm thanh đang phát.
- Mic vẫn hoạt động như cũ (không regress).
- Auto-transcript hoạt động với cả nguồn loopback (transcribe segment như thường).
- Dừng ghi → thread loopback dừng sạch (join), không treo process.

## Notes / limitations (v1)

- **Windows-only**: non-Windows → list_output_devices rỗng + chọn "system" trả lỗi rõ ràng. macOS để sau.
- **Khoảng lặng**: WASAPI loopback khi render idle (không app nào phát) có thể không trả data → đoạn ghi
  có thể bị "hụt" thời lượng / segment-theo-phút lệch wall-clock. Chấp nhận v1 (use case chính có âm thanh).
  Nếu cần liên tục → v2 chèn silence theo clock hoặc mở silent render stream giữ engine chạy.
- **Format**: cố định i16 48k stereo (autoconvert). ffmpeg vẫn downmix mono 48k như cũ.
- Phải chọn ĐÚNG render device đang phát (mặc định = default render). Chọn nhầm device im lặng → file im lặng.

## Unresolved questions

- Có cần xử lý khoảng-lặng (silence padding) để timeline liên tục không? (hiện bỏ qua — v1)
- macOS (ScreenCaptureKit) khi nào cần? (đã hoãn theo quyết định người dùng)
