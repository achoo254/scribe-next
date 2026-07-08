# Phase 03 — Player đồng bộ + editor sửa & lưu + list batch per-file

> Tier **M** · Phụ thuộc: 02 (segments/timestamp) · Fix gap G2 (chỉ đọc), G3 (không player/seek), G5 (batch chỉ hiện file cuối).

## Bối cảnh

`TranscriptView.vue:130` textarea `readonly`; không có audio/video element; `use-transcribe.ts:113` `panelText` bị ghi đè → batch chỉ xem file cuối. Đây là phần trải nghiệm lõi của tool transcribe.

## Yêu cầu

- Player audio/video phát file gốc đang chọn (local path → `convertFileSrc`).
- Danh sách segment có mốc thời gian; click 1 dòng → player `currentTime = start`. (Tuỳ chọn) highlight dòng đang phát theo `timeupdate`.
- Text sửa được; nút **Lưu lại** ghi đè `.txt` (+ regenerate `.srt`/`.vtt` từ segment đã sửa nếu khả thi; tối thiểu: lưu `.txt`).
- Batch: đổi panel đơn thành **list per-file** (mỗi file expandable xem/sửa/lưu riêng) — bỏ `panelText` ghi đè.

## Hạ tầng cần bật (điểm dễ vướng nhất)

- `src-tauri/tauri.conf.json` → `app.security.assetProtocol`: `{ "enable": true, "scope": ["**"] }` (file user chọn ở đường dẫn bất kỳ; desktop offline chỉ-đọc → chấp nhận; nếu muốn chặt hơn: scope theo thư mục out_dir/last-picked).
- `src-tauri/capabilities/default.json` → thêm permission `"core:asset"` (hoặc `asset:default`) cho window `main`.
- Frontend dùng `import { convertFileSrc } from "@tauri-apps/api/core"` → `:src` cho `<audio>`/`<video>`.

## Files sửa/tạo

- `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json` — bật asset protocol (trên).
- `src-tauri/src/transcribe.rs` + `commands.rs` + `lib.rs` — thêm command `save_transcript(path: String, content: String)` (ghi UTF-8) — dùng command riêng thay vì fs plugin scope cho đơn giản/an toàn đường dẫn. (Nếu Phase 02 đã có JSON segments, có thể thêm `save_srt(segments)` để regen; tối thiểu làm `.txt` trước.)
- `src/components/TranscriptPlayer.vue` (mới) — player + segment list click-to-seek; props: `mediaSrc`, `segments`, emit `seek`/`save`.
- `src/composables/use-transcribe.ts` — bỏ `panelText` ghi đè; giữ `results[]` đầy đủ (mỗi item có `segments` từ JSON P02 + text editable state).
- `src/views/TranscriptView.vue` — render list `results` với `TranscriptPlayer` per-file (expandable).

## Các bước

1. Bật asset protocol + permission; smoke: `convertFileSrc(path)` phát được 1 mp3 local.
2. Command `save_transcript` (Rust) + đăng ký `lib.rs` invoke_handler.
3. `TranscriptPlayer.vue`: `<audio/video>` + list segment (dùng `<video>` nếu ext video, `<audio>` nếu audio — tái dùng AUDIO_EXT/VIDEO_EXT). Click dòng → `el.currentTime = seg.start; el.play()`.
4. Editor: textarea editable per-file; nút Lưu → `save_transcript`; trạng thái "đã lưu".
5. `use-transcribe.ts`: giữ segments trong `results`; bỏ `panelText` (hoặc chỉ giữ cho tương thích tạm). TranscriptView render list.

## Test / validate

- Chọn 1 file audio + 1 file video → transcribe → player phát được cả 2; click segment nhảy đúng mốc.
- Sửa text → Lưu → mở lại `.txt` trên đĩa thấy nội dung mới.
- Batch 3 file → xem/sửa được cả 3 trong app (không chỉ file cuối).
- `vue-tsc --noEmit` + `cargo build` sạch. Kiểm cả macOS lẫn Windows path (backslash) cho `convertFileSrc`.

## Rủi ro / rollback

- **Asset scope `**` = đọc mọi file** khi app chạy. Offline desktop, chỉ-đọc, file do user tự chọn → rủi ro thấp; ghi chú trong `docs`. Muốn chặt: scope động theo thư mục file đang mở.
- Video lớn phát trong webview có thể nặng — chấp nhận, hoặc chỉ audio-track cho video (dùng `<audio>` trỏ cùng src).
- Rollback: ẩn player/editor sau flag, quay lại panel readonly.
