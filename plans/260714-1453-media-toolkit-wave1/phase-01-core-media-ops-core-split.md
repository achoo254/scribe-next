---
phase: 1
title: "Core media ops (core-split)"
status: pending
priority: P1
dependencies: []
---

# Phase 1: Core media ops (core-split)

## Overview
Thêm 3 hàm media vào crate `core-split` (nguồn chân lý chung desktop+server): convert, extract-audio, trim. TDD: viết test ffmpeg thật TRƯỚC (khớp pattern `tests/split_file_ffmpeg.rs`).

## Requirements
- Functional: `convert_file`, `extract_audio`, `trim_file` gọi ffmpeg, ghi output đúng codec/thời lượng.
- Non-functional: pure Rust deps (giữ musl static sạch); không đổi chữ ký `split_file` (BE/desktop không vỡ); deflate/ffmpeg qua path truyền vào (không hardcode).

## Architecture

<!-- Updated: Validation Session 1 — Wave 1 AUDIO-ONLY (bỏ nhánh MP4 video); trim → mp3. -->

- `crates/core-split/src/` thêm module (vd `convert.rs`, `trim.rs`) + export ở `lib.rs`.
- **Wave 1 AUDIO-ONLY**: format ∈ **{mp3, m4a, wav}**. audio codec: mp3→`libmp3lame`; m4a→`aac`; wav→`pcm_s16le`. KHÔNG implement nhánh MP4 video re-encode (libx264) ở Wave 1 (YAGNI — hoãn).
- Chữ ký đề xuất:
  - `pub async fn convert_file(ffmpeg: &str, input: &str, out_path: &str, format: &str, drop_video: bool) -> Result<(), String>`
    - `drop_video=true` thêm `-vn` (dùng cho extract-audio từ input có video).
    - `extract_audio` = wrapper `convert_file(..., drop_video=true)` (DRY — KHÔNG copy-paste).
  - `pub async fn trim_file(ffmpeg: &str, input: &str, out_path: &str, start_s: f64, end_s: f64) -> Result<(), String>`
    - `-ss {start} -to {end}` + re-encode ra **mp3** (mốc chính xác). Validate `0 ≤ start < end`.
- ffmpeg flags: `-y -i <in> [-vn] -c:a <codec> <out>` (audio-only nên luôn không giữ video track ở output convert; extract dùng `-vn` cho input video).

## Related Code Files
- Create: `crates/core-split/src/convert.rs`, `crates/core-split/src/trim.rs`
- Create: `crates/core-split/tests/convert_trim_ffmpeg.rs` (test ffmpeg thật)
- Modify: `crates/core-split/src/lib.rs` (pub use các hàm mới)

## Implementation Steps (TDD — test trước)
1. **Test trước**: `tests/convert_trim_ffmpeg.rs` — dựng input nhỏ (tái dùng helper sinh media như test split hiện có, hoặc ffmpeg lavfi `sine`/`testsrc`). Cases:
   - convert wav→mp3: probe output `acodec` = mp3, duration ≈ input (±0.5s).
   - extract_audio từ input có video (testsrc+sine)→mp3: probe output KHÔNG có vcodec.
   - trim start=1.0 end=3.0 →mp3: duration ≈ 2.0s (±0.3).
   - lỗi: start≥end → `Err`.
2. Chạy test → đỏ (hàm chưa có).
3. Impl `convert_file` (+ `extract_audio` wrapper) trong `convert.rs`; `trim_file` trong `trim.rs`; export ở `lib.rs`.
4. Chạy `cargo test -p core-split` → xanh (12 cũ + mới).
5. `cargo clippy -p core-split` sạch.

## Success Criteria
- [ ] Test convert/extract/trim (ffmpeg thật) viết trước, ban đầu đỏ, sau xanh.
- [ ] `cargo test -p core-split` xanh toàn bộ; `cargo clippy` sạch.
- [ ] `split_file` chữ ký + hành vi KHÔNG đổi (desktop + BE compile).
- [ ] `extract_audio` không copy-paste `convert_file` (dùng lại qua param).

## Risk Assessment
- ffmpeg thiếu encoder (libmp3lame) trên môi trường CI/box → test/skip có điều kiện hoặc chọn encoder có sẵn; xác nhận sidecar ffmpeg .65 có libmp3lame/aac (nếu thiếu, fallback `-c:a aac` cho m4a và cảnh báo mp3). Mitigation: probe encoder khả dụng lúc test; document yêu cầu build ffmpeg.
- Duration probe webm = 0 (thiếu metadata) đã biết → test dùng input có duration rõ (wav/mkv), không webm thuần.
