---
phase: 3
title: "Mode B — Cắt file"
status: pending
priority: P1
effort: "2-3d"
dependencies: [2]
---

# Phase 3: Mode B — Cắt file

## Overview

Upload file audio/video → **cắt theo phút / MB / số phần** bằng `SplitWorker` (gọi crate `core-split`, `ffmpeg -c copy` lossless) qua job queue MongoDB → tải kết quả (zip nhiều đoạn). FE port `SplitView`. File upload có cues/keyframe chuẩn → `-c copy` cắt đúng như desktop (khác hẳn recording webm ở Phase 4).

## Requirements

- **Functional:** upload 1..n file → chọn tiêu chí (phút/MB/parts) + giá trị → tạo job → xem tiến độ (SSE) → tải zip đoạn. Lỗi 1 file không chặn file khác.
- **Non-functional:** upload resumable (chunked, **append thẳng disk, KHÔNG buffer RAM**); job queue concurrency=1 + ffmpeg `nice 10`; ownership (user chỉ thấy job/file của mình); size cap.

## Architecture

**Upload (chunked POST — KISS, research):** FE `file.slice()` 5MB/chunk → POST `{sessionId, chunkIndex, totalChunks, chunk}`; BE **append thẳng vào file `.part` theo offset trên disk** (KHÔNG giữ `Vec` chunk trong RAM — box .65 yếu, tránh OOM); finalize → validate size/magic-byte → tạo `files{kind:"upload"}`. Progress bar bằng XHR upload events.

**Job queue (Mongo `jobs`):** tạo `jobs{type:"split", inputFileId, params:{criterion,value}, status:"queued"}`. Worker loop: `find_one_and_update({status:queued}→{running})` (atomic, concurrency=1) → chạy `SplitWorker` → cập nhật progress → `done` với `result:{segments:[...]}` hoặc `error`. FE nhận tiến độ qua `GET /api/jobs/:id/stream` (SSE).

**SplitWorker:** gọi `core-split::split(input, criterion, value, out_dir, ffmpeg_path, on_progress)` — bê logic `splitter.rs` (phút→`-segment_time`; MB→keyframe byte `-segment_times`; parts→duration/k; HEVC hvc1; `-c copy`). Output `/data/{userId}/outputs/{jobId}/`. ffmpeg spawn `nice 10`.

**Download:** `GET /api/jobs/:id/download` → zip streaming các đoạn (ownership check). Hoặc list từng đoạn `GET /api/files/:id`.

**FE (`web/`):** port `SplitView.vue` — chọn file (upload thay vì path local), probe hiển thị thời lượng (BE `/api/probe` hoặc trả sau upload), chọn tiêu chí, nút Cắt, progress, danh sách đoạn + tải zip.

## Related Code Files

- Create: `server/onescribe-be/src/upload.rs` (chunked append-to-disk), `server/onescribe-be/src/jobs.rs` (queue + worker loop + SSE), `server/onescribe-be/src/worker/split_worker.rs`, `server/onescribe-be/src/models/{file,job}.rs`, `web/src/views/SplitView.vue`, `web/src/composables/use-upload.ts`, `web/src/composables/use-splitter.ts`
- Modify: `crates/core-split/src/lib.rs` (đảm bảo API dùng được ngoài Tauri), `server/onescribe-be/src/routes.rs`
- Delete: none

## Implementation Steps

1. `upload.rs`: endpoint chunked POST → mở file `.part` `append` theo `chunkIndex*chunkSize` (seek+write, không RAM buffer) → finalize (rename + magic-byte check qua `infer`/đọc header + size cap) → tạo `files` doc + probe duration (ffprobe qua core-split hoặc helper).
2. `jobs.rs`: tạo job (POST `/api/jobs`), worker task nền (`tokio::spawn`) poll+claim atomic, SSE `/api/jobs/:id/stream`, ownership.
3. `split_worker.rs`: gọi `core-split`; map progress → cập nhật `jobs.progress`; ghi `result`/`error`; ffmpeg `nice`.
4. Download zip streaming (`async-zip` hoặc tokio) + ownership.
5. FE SplitView + use-upload (progress) + use-splitter (tạo job, theo SSE, tải zip).
6. E2E: upload file 60' → cắt phút/MB/parts → tải zip → mở đoạn OK; user khác không truy cập được job/file.

## Success Criteria

- [ ] Upload file lớn (vài trăm MB) không làm BE spike RAM (append disk); resume chunk lỗi.
- [ ] Cắt phút/MB/parts đúng như desktop (đối chiếu số đoạn + thời lượng); `-c copy` lossless.
- [ ] Job queue concurrency=1; 2 job → tuần tự; lỗi 1 file emit error, không chặn file khác.
- [ ] Ownership: `/api/jobs/:id`, `/api/files/:id`, download đều chặn cross-user.
- [ ] Tải zip đoạn hoạt động; size cap + magic-byte chặn file rác.

## Risk Assessment

- **Chunked upload buffer RAM (research example giữ HashMap chunks):** BẮT BUỘC append thẳng disk theo offset, không giữ trong RAM → tránh OOM box .65.
- **ffmpeg đè CPU:** `-c copy` RTF rất cao (cắt 60' xong trong giây) → nhẹ; `nice 10` + concurrency=1 vẫn giữ.
- **Magic-byte/decode RCE (upload công khai):** validate magic-byte + chạy ffmpeg với input không tin cậy (ffmpeg LGPL, không exec) + size/time cap; worker không thừa kế secret ngoài env compile-time.
- **Disk đầy giữa job:** kiểm dung lượng trước khi cắt + quota (Phase 5).
- **In-memory session (chunk state) mất khi restart BE:** lưu session state vào Mongo hoặc chấp nhận re-upload (file dở); chốt ở cook.
