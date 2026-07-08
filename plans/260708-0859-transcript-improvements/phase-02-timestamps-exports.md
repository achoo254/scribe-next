# Phase 02 — word_timestamps + SRT chuẩn phụ đề + export VTT/JSON

> Tier **M** · Phụ thuộc: 01 · Fix gap G4 (SRT dùng segment thô, dòng dài 10–20s).

## Bối cảnh

`main.py:100-105` (ct2) và `:119-122` (mlx): 1 segment Whisper = 1 cue SRT, không giới hạn ký tự/thời lượng → không dùng làm phụ đề thật được. Bật `word_timestamps=True` (faster-whisper hỗ trợ sẵn) → có mốc từng từ → resegment cue theo chuẩn phụ đề. Đồng thời timestamp/segment này là **dữ liệu Phase 03 cần**.

## Yêu cầu

- Cue SRT: ≤ ~42 ký tự/dòng, ≤ 2 dòng, ≤ ~6s/cue, không cắt giữa từ.
- Thêm output `vtt` (WEBVTT, mốc dùng `.`) và `json` (segments: `start,end,text,words[]`).
- `outputs` mở rộng: `["txt","srt","vtt","json"]` (UI chọn; mặc định giữ txt+srt).
- Áp cho cả ct2 và mlx (mlx_whisper cũng có `word_timestamps`).
- Không phá protocol hiện tại (event `done` giữ `text/txt_path/srt_path`; thêm `vtt_path/json_path` optional).

## Files sửa/tạo

- `sidecar/transcriber/main.py`:
  - ct2: `word_timestamps=True`; thu segments kèm `words`.
  - mlx: `word_timestamps=True`; chuẩn hoá segments về cùng shape.
  - Thêm helper `resegment_cues(segments, max_chars=42, max_dur=6.0)` → list cue `{start,end,text}` (gộp/tách theo word boundary).
  - `_write_outputs`: sinh `.srt` (từ cue đã resegment), `.vtt`, `.json` theo `outputs`.
  - `done` payload thêm `vtt_path`, `json_path` (nullable).
- `src/composables/use-transcribe.ts` + `use-record-transcribe.ts`: `Msg`/`TransResult` thêm `vtt_path`/`json_path` optional; `outputs` truyền theo lựa chọn UI.
- `src/views/TranscriptView.vue`: nhóm chọn định dạng xuất (checkbox txt/srt/vtt/json), mặc định txt+srt.

## Các bước

1. `main.py`: bật `word_timestamps=True` 2 nhánh; chuẩn hoá segments → `[{start,end,text,words:[{start,end,word}]}]`.
2. Viết `resegment_cues`: duyệt word, tích luỹ tới khi chạm max_chars/max_dur hoặc gặp ngắt câu (`. ? ! …`) → chốt cue; wrap ≤2 dòng.
3. `_write_outputs`: tách hàm nhỏ `to_srt(cues)`, `to_vtt(cues)`, `to_json(segments)`; ghi theo `outputs`.
4. Frontend: thêm state `exportFormats`, truyền vào `transcribe_files`; nới type.
5. Giữ `text` (toàn văn) như cũ cho panel.

## Test / validate

- SRT xuất ra: kiểm tay 1 file — không cue nào > 42 ký tự/dòng hay > 6s; mở bằng VLC/player → hiện đúng.
- VTT mở được (header WEBVTT, mốc `.`); JSON parse được, `words[]` có mốc.
- `vue-tsc --noEmit` + `cargo build` sạch (Rust không đổi logic, chỉ nếu chạm command outputs).
- So khớp: `text` = ghép cue; 3 định dạng cùng nội dung.

## Rủi ro / rollback

- `word_timestamps=True` chậm hơn ~10-20% + RAM nhỉnh — chấp nhận (chất lượng đổi lại). Nếu ngại: chỉ bật khi user chọn "phụ đề chuẩn".
- MLX word timestamps có thể thiếu ổn định trên vài bản model → fallback resegment theo segment-level nếu `words` rỗng.
- Rollback: bỏ resegment, `to_srt` về 1 segment = 1 cue như cũ.
