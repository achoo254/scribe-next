---
phase: 3
title: "Mode B - Split existing files"
status: implemented
priority: P1
effort: "2d"
dependencies: [1]
implemented: "2026-06-13"
note: "Code + build + code-review (fix C1 collision, H1 stale-output, parts integer, probe→UI) + ViUI validate xong. Chờ test cắt file thủ công."
---

# Phase 3: Mode B - Split existing files

## Overview

Cắt file audio sẵn có thành nhiều đoạn theo **phút / MB / chia-K đều**, hỗ trợ **batch** nhiều file. Dùng ffmpeg `-c copy` (lossless, nhanh). Tất cả tiêu chí quy về một `segment_time` (giây) → một code path.

## Requirements

- Functional: chọn 1+ file; chọn tiêu chí [Phút | MB | Chia-K]; nhập giá trị; chọn output folder; nút Cắt; progress per-file + tổng; danh sách đoạn kết quả + mở thư mục.
- Non-functional: lossless (giữ codec gốc); batch chạy tuần tự; không trùng tên file output.

## Architecture

```
probe_file(path) --ffprobe/ffmpeg-> {duration, bitrate, codec, ext}
criterion → segment_time (giây):
   Phút  → value*60
   MB    → value*8_000_000 / bitrate_bps   (CBR sát; VBR ước lượng ±)
   Chia-K→ duration / value                 (chỉ Mode B)
ffmpeg -i <input> -f segment -segment_time <giây> -reset_timestamps 1 -c copy
   "<out_dir>/<stem>/<stem>_%03d.<ext>"     (subfolder theo tên gốc → tránh trùng)
batch: loop từng file tuần tự, emit progress (đọc ffmpeg -progress pipe:)
```

- **`splitter.rs`**: `probe_file()`, `compute_segment_time(criterion, value, meta)`, `split_one()`, `split_batch()`. Nếu > 200 dòng → tách `splitter/criterion.rs` (chỉ phần quy đổi).
- **Progress**: chạy ffmpeg với `-progress pipe:1`, parse `out_time_ms`/`total_size` → % so với duration; emit event per-file + tổng (file index / tổng số file).

## Related Code Files

- Create: `src-tauri/src/splitter.rs` (+ `splitter/criterion.rs` nếu cần)
- Modify: `src-tauri/src/commands.rs` (`probe_file`, `split_files`)
- Modify: `src-tauri/src/main.rs` (đăng ký), `src/views/SplitView.vue` (UI ViUI), `src/composables/use-splitter.ts` (wrap invoke + listen progress), `src/styles.css`

## Implementation Steps

1. `splitter.rs::probe_file()`: gọi `ffprobe` (kèm ffmpeg sidecar) hoặc `ffmpeg -i` parse → `{duration_s, bitrate_bps, codec, ext}`.
2. `compute_segment_time()`: switch theo criterion (minutes/megabytes/parts) → giây. Guard chia-K: K≥1, value>0.
3. `split_one(path, seg_time, out_dir)`: tạo subfolder `<stem>`, chạy ffmpeg segment `-c copy` với `-progress pipe:1`; emit progress.
4. `split_batch(paths[], criterion, value, out_dir)`: loop tuần tự; emit `{file_index, total, file_pct, overall_pct}`; gom kết quả.
5. `commands.rs`: expose `probe_file` (để UI hiển thị duration trước khi cắt) + `split_files`.
6. `SplitView.vue` (**UI bắt buộc ViUI — chỉ Vi*, token-only, WCAG AA**): multi-file picker = `ViButton` (mở dialog) + `ViList` hiển thị file đã chọn; radio tiêu chí [Phút|MB|Chia-K] = `ViRadioGroup`/`ViButtonToggle`; ô giá trị = `ViInput` (đổi label theo tiêu chí + ẩn chia-K nếu cần); output folder = `ViButton`; nút Cắt = `ViButton` (primary); progress per-file + tổng = thanh progress (verify `ViProgress`/`ViLoading` qua MCP; nếu thiếu wrapper linear → tạo Vi* wrapper hoặc div token-styled); danh sách kết quả = `ViList`/`ViDataTable` + `ViButton` mở thư mục; note VBR ước lượng = `ViAlert`.
7. Test: cắt 1 mp3 theo phút / MB / chia-4; batch 3 file → mỗi file ra subfolder riêng, đoạn lossless phát được, không trùng tên.
8. **Verify design system**: ViUI MCP `validate_brand_usage` + `review_design_tokens` trên `SplitView.vue` → 0 vi phạm.

## Todo List

- [x] probe_file (duration/bitrate/codec/ext) + nối UI hiển thị duration
- [x] compute_segment_time 3 tiêu chí + guard (parts integer ≥1, bitrate=0, secs<1)
- [x] split_one + ffmpeg -c copy segment + progress (out_time_us) + cleanup segment cũ
- [x] split_files batch tuần tự + progress tổng + dedupe subdir (chống ghi đè trùng tên)
- [x] commands probe_file + split_files
- [x] SplitView.vue UI (Vi* + token + WCAG): multi-file, radio, progress, kết quả
- [ ] **test cắt file thủ công**: phút/MB/chia-4 + batch (m4a copy chưa test)
- [x] ViUI MCP `validate_brand_usage` → 0 vi phạm (SplitView). *`review_design_tokens` không chạy được (MCP sandbox).*

## Success Criteria

- [ ] Cắt theo phút / MB / chia-K ra đúng số đoạn kỳ vọng, lossless, phát được
- [ ] Batch nhiều file: mỗi file ra bộ đoạn riêng (subfolder), không trùng tên
- [ ] Progress per-file + tổng hiển thị đúng
- [ ] VBR file: kết quả MB ước lượng chấp nhận được (note trong UI)

## Risk Assessment

- `-c copy` segment với VBR/timestamp lạ có thể lệch đoạn đầu. Mitigation: `-reset_timestamps 1`; nếu codec không cho copy-segment (hiếm với mp3/aac) → fallback note.
- ffprobe có thể không kèm trong build ffmpeg LGPL tối giản. Mitigation: dùng `ffmpeg -i` parse stderr nếu thiếu ffprobe, hoặc bundle cả ffprobe.

## Next Steps

Kết thúc build-block 1 (record + split) cùng P2 → có thể release sớm bản v1 không-transcribe ở P5.
