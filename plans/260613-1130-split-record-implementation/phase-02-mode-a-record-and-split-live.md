---
phase: 2
title: "Mode A - Record and split live"
status: implemented
priority: P1
effort: "3-4d"
dependencies: [1]
implemented: "2026-06-13"
note: "Code + build + code-review + ViUI validate xong. Chờ test ghi âm thủ công (cần mic + nghe lại)."
---

# Phase 2: Mode A - Record and split live

## Overview

Ghi audio micro và split LIVE: cpal capture PCM → đẩy qua stdin vào ffmpeg sidecar dùng segment muxer để vừa nén (MP3/M4A) vừa cắt file mỗi N phút/MB. Kèm level meter, pause/resume, danh sách file gần đây.

## Requirements

<!-- Updated: Validation Session 1 - format ghi live mặc định = MP3 (crash-safe); M4A-AAC là option -->
- Functional: liệt kê mic; chọn mic + thư mục lưu; nhập độ dài segment (đơn vị **phút** hoặc **MB**); chọn format **MP3 (mặc định)** / **M4A-AAC** (option); Record/Pause/Resume/Stop; level meter realtime; danh sách segment vừa ghi + nút mở thư mục.
- Lý do MP3 default: stream-friendly → crash giữa phiên thì segment dở vẫn nghe được (M4A cần `+frag_keyframe+empty_moov` mới đạt điều này).
- Non-functional: mỗi segment phát được ngay; pause không tạo gap/lỗi; CPU/UI không block (capture ở audio thread riêng).

## Architecture

```
[Mic] --cpal stream (callback)--> i16/f32 frames
        ├── meter.rs: RMS/peak mỗi buffer → throttle ~30fps → emit "level" event → UI
        ├── nếu Paused: bỏ qua, không ghi
        └── ringbuf → writer thread → ffmpeg stdin (s16le)
ffmpeg sidecar (encode-only, đọc stdin):
  -f s16le -ar 48000 -ac 1 -i -
  -c:a aac (-movflags +frag_keyframe+empty_moov) | libmp3lame -b:a 128k
  -f segment -segment_time <giây> -reset_timestamps 1
  "<out_dir>/rec_YYYYMMDD_HHMMSS_%03d.<ext>"
```

- **State machine** (`recorder.rs`): `Idle → Recording ⇄ Paused → (Stop) Idle`. Stop: đóng stdin → đợi ffmpeg flush → finalize → refresh list.
- **Đơn vị split → segment_time (giây)**: phút→`N*60`; MB→`MB*8_000_000 / bitrate_bps` (bitrate = giá trị encode CBR, sát vì ta kiểm soát).
- **Pause semantics**: ngừng feed stdin → pts đóng băng → segment đo theo audio thực, không theo wall-clock.
- **Crash-safety**: M4A dùng `+frag_keyframe+empty_moov` để file dở vẫn phát; MP3 vốn miễn nhiễm.

## Related Code Files

- Create: `src-tauri/src/audio/capture.rs` (cpal device enum + stream + ringbuf)
- Create: `src-tauri/src/audio/meter.rs` (RMS/peak)
- Create: `src-tauri/src/audio/encoder.rs` (ffmpeg stdin pipe + segment args + writer thread)
- Create: `src-tauri/src/recorder.rs` (state machine, điều phối capture+encoder)
- Modify: `src-tauri/src/commands.rs` (`list_devices`, `start_recording`, `pause`, `resume`, `stop_recording`, `pick_folder`)
- Modify: `src-tauri/src/main.rs` (đăng ký commands + state), `src/views/RecordView.vue` (UI dùng Vi*), `src/composables/use-recorder.ts` (wrap invoke + listen events), `src/styles.css`
- Create (nếu thiếu): `src/components/LevelMeter.vue` (level meter custom, token-styled — không có Vi* sẵn); wrapper Vi* bổ sung nếu P1 chưa dựng (vd `ViProgressLinear`)

## Implementation Steps

1. `audio/capture.rs`: `list_input_devices()` qua cpal; `start_stream(device, cfg)` → callback đẩy frames vào `ringbuf`, resample/convert về i16 mono 48k nếu cần.
2. `audio/meter.rs`: tính RMS + peak mỗi buffer; trả giá trị 0..1.
3. `audio/encoder.rs`: spawn ffmpeg sidecar với args theo format + segment_time; writer thread đọc ringbuf → ghi stdin; hàm `finalize()` đóng stdin + join.
4. `recorder.rs`: struct giữ state + handles; `start/pause/resume/stop`; sinh prefix `rec_YYYYMMDD_HHMMSS` (thời điểm bắt đầu phiên).
5. `commands.rs`: expose các lệnh + emit events `level`, `segment_created`, `rec_state`.
6. `RecordView.vue` (**UI bắt buộc ViUI — chỉ Vi*, token-only, WCAG AA**): mic = `ViSelect`; chọn folder = `ViButton` (+ `ViIcon` Tabler folder); ô số = `ViInput` (number); toggle phút/MB = `ViButtonToggle`/`ViRadioGroup`; format MP3/M4A = `ViRadioGroup`/`ViSelect`; Record/Pause/Stop = `ViButton` (màu semantic: record=primary, stop=error); đồng hồ + số segment = text (typography token); **level meter = `src/components/LevelMeter.vue`** (canvas/div width — custom, style chỉ bằng token CSS vars); danh sách segment gần đây = `ViList` + `ViButton` mở thư mục. Logic invoke/listen tách `use-recorder.ts`. Tra spec Vi* qua MCP `get_component_api`/`get_component_usage`; token qua `lookup_token`/`search_tokens`.
7. Test thủ công: ghi 30s với segment 0.2 phút → ra nhiều file, mỗi file nghe được; pause giữa chừng → resume → không mất tiếng; rút mic → báo lỗi gọn (`ViAlert` semantic error).
8. **Verify design system**: ViUI MCP `validate_brand_usage` + `review_design_tokens` trên `RecordView.vue` + `LevelMeter.vue` + `styles.css` → 0 raw Vuetify trong view / 0 hardcoded value / 0 vi phạm.

## Todo List

- [x] capture.rs (cpal device enum + stream → i16 channel; convert f32/i16/u16)
- [x] meter.rs (RMS/peak) + emit throttle ~30fps (recorder)
- [x] encoder.rs (ffmpeg args stdin + segment muxer, M4A frag + MP3)
- [x] recorder.rs state machine + naming (rec_YYYYMMDD_HHMMSS) + drain events + done-flag
- [x] commands + events (level/segment_created/segments_final/rec_state/rec_error)
- [x] RecordView.vue (Vi* + token + WCAG) + LevelMeter.vue custom + use-recorder.ts
- [x] MB↔giây (×62.5 @128k) + phút↔giây (×60) conversion
- [ ] **test ghi âm thủ công** (cần mic): 30s segment 0.2 phút → nhiều file; pause/resume; rút mic; M4A crash-test
- [x] ViUI MCP `validate_brand_usage` → 0 vi phạm (RecordView + LevelMeter). *`review_design_tokens` không chạy được (MCP sandbox không thấy ổ local).*

## Success Criteria

- [ ] Record → các file ~N phút/MB liên tiếp, mỗi file phát được
- [ ] Pause/Resume không tạo file lỗi, không mất tiếng
- [ ] Level meter nhúc nhích đúng tiếng vào
- [ ] Stop → segment cuối hợp lệ + hiện trong danh sách
- [ ] M4A crash-test: kill app giữa segment → file dở vẫn mở được (frag mp4)

## Risk Assessment

- cpal format khác nhau giữa device (sample rate/format) → cần convert về s16le 48k mono trước khi feed ffmpeg. Mitigation: ép config + resample.
- ffmpeg stdin backpressure nếu encode chậm → ringbuf đầy. Mitigation: ringbuf đủ lớn + drop/log nếu tràn (hiếm với audio).
- Segment boundary lệch vài chục ms (segment muxer cắt ở frame) → chấp nhận cho audio.

## Next Steps

Hoàn tất build-block 1 cùng P3. UI record + danh sách file tái dùng cho transcribe (P4).
