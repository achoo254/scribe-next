---
status: completed
---

# Fix: Cắt-khi-ghi (Mode A web) ra nhiều đoạn + bản ghi tua được

> **Đóng plan (2026-10-19):** đã giao trên web Vue, bản React giữ nguyên hành vi: "Cắt bản ghi thành nhiều bản ghi" đạt
> trong bảng parity ngày 2026-10-15 (plan `../261015-1524-web-react-rewrite/`).

Branch: main · 2026-07-18

## Bối cảnh (chẩn đoán)

Web Scribe (thư mục `web/` + `server/scribe-be`), KHÔNG phải desktop Tauri.

**Bug #1 — bật "Cắt thành đoạn theo phút" nhưng Lịch sử ghi chỉ 1 file.**
- Dừng ghi → finalize tạo 1 bản ghi gốc (`kind:recording`). Bật cắt → job cắt tạo đoạn `kind:output`.
- `/api/files` (Lịch sử ghi) chỉ query `kind:recording` (`models/file.rs:168`) → đoạn `output` không bao giờ hiện. Đoạn chỉ tải qua zip.

**Bug #2 — bản ghi không tua được.**
- File ghi là webm MediaRecorder streaming, finalize KHÔNG remux → thiếu Cues + Duration (đã thực nghiệm xác nhận: `duration=N/A`, không Cues).
- BE `stream` có HTTP Range nhưng browser cần Cues để map timestamp→byte → không tua.

**Thực nghiệm (ffmpeg LGPL-equiv) đã xác nhận:**
- `remux -c copy -f webm` webm-streaming → webm có `duration=25.028` + Cues → tua được.
- `-f segment -segment_time -c copy` trên webm seekable → nhiều đoạn webm, mỗi đoạn có duration + Cues (phát/tua riêng được).

## Quyết định user (chốt)
- Q1: **Đoạn cắt vào Lịch sử ghi, thay bản ghi gốc** (xuất webm; xoá gốc sau cắt).
- Q2: **Remux webm khi finalize** → mọi bản ghi tua được + duration chuẩn.

## Phạm vi thay đổi

### BE — crate core-split
- `split.rs`: thêm `remux_copy_to_webm` (refactor helper chung với `remux_copy_to_mkv`).

### BE — server scribe-be
- `recordings.rs::finalize`: EBML (webm/mkv) → remux ra webm seekable (fallback rename nếu remux lỗi → không mất bản ghi). Ép ext `.webm`. Probe duration (remux có duration).
- `models/job.rs`: thêm field `recordingSplit` + param `create_split`.
- `jobs.rs`: `CreateReq.recordingSplit`; guard input phải `kind:recording` khi bật; truyền vào create_split.
- `worker/mod.rs`: `OutputSeg` thêm `duration_s: Option<f64>` + `has_video: bool`.
- `worker/split_worker.rs`: nhánh `recording_split` → segment thẳng webm (input đã seekable, KHÔNG remux mkv) + probe mỗi đoạn.
- `jobs.rs::run_job`: `recording_split` → tạo `kind:recording` mỗi đoạn (tên "<gốc> — Phần N") + xoá gốc (disk+doc) sau khi tạo xong.

### FE — web
- `use-splitter.ts`: `createJob(..., recordingSplit=false)` → body thêm `recordingSplit`.
- `RecordView.vue`: cắt-khi-ghi gọi `createJob(fileId, "minutes", N, true)`; không refresh history ngay (gốc sẽ bị xoá); khi job `done` → refresh history + usage; đổi UI kết quả (bỏ nút "Tải zip", báo "xem trong Lịch sử ghi").

## Acceptance
1. Bật cắt, ghi, dừng → Lịch sử ghi hiện N đoạn (không còn bản ghi gốc dài), mỗi đoạn phát + tua được.
2. Không bật cắt → bản ghi đơn tua được (seekbar kéo tới mọi vị trí).
3. Mode B (Cắt file thường) không đổi (vẫn output→zip).
4. `cargo build` server + `cargo test -p core-split` xanh; FE `vue-tsc` xanh.

## Rủi ro / rollback
- Remux finalize chậm hơn + disk tạm 2× → fallback rename khi lỗi (không mất bản ghi).
- File webm CŨ (trước fix) vẫn không seekable — chỉ bản ghi mới. Chấp nhận.
- Rollback: revert commit; job cũ `recordingSplit` mặc định false nên tương thích ngược.
