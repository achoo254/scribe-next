---
phase: 3
title: "Mode B — Cắt file"
status: in-progress
priority: P1
effort: "2-3d"
dependencies: [2]
---

# Phase 3: Mode B — Cắt file

> **Progress 2026-07-11:** Code XONG + DEPLOYED LIVE. BE: `upload.rs` (chunked append-disk, sessionId UUID server-gen + bind user + validate UUID chống traversal, resumable stateless theo .part, size cap 2GB + quota tăng dần), `jobs.rs` (queue + claim atomic + worker loop + reset running→queued khi start + download zip), `worker/{mod,split_worker}.rs` (trait Worker + SplitWorker gọi core-split + TranscribeWorker stub), `models/{file,job}.rs` (ownership + user_usage_bytes). FE: SplitView + use-upload (resume) + use-splitter (poll) + route /split. `cargo check` + `vue-tsc` XANH; rebuild musl + redeploy .65 OK (BE restart active, health 200, /api/me 401). **CÒN LẠI:** code-review (đang chạy) + manual QA cắt file thật (user). Progress job coarse (queued→running→done) vì -c copy nhanh — progress mịn để dành Phase 4.

## Overview

Upload file audio/video → **cắt theo phút / MB / số phần** bằng `SplitWorker` (gọi crate `core-split`, `ffmpeg -c copy` lossless) qua job queue MongoDB → tải kết quả (zip nhiều đoạn). FE port `SplitView`. File upload có cues/keyframe chuẩn → `-c copy` cắt đúng như desktop (khác hẳn recording webm ở Phase 4).

## Requirements

- **Functional:** upload 1..n file → chọn tiêu chí (phút/MB/parts) + giá trị → tạo job → xem tiến độ (poll `GET /api/jobs/:id` mỗi 1-2s) → tải zip đoạn. Lỗi 1 file không chặn file khác.
- **Non-functional:** upload resumable (chunked, **append thẳng disk, KHÔNG buffer RAM**); job queue concurrency=1 + ffmpeg `nice 10`; ownership (user chỉ thấy job/file của mình); size cap.

## Architecture

**Upload (chunked POST — KISS, research):** FE `file.slice()` 5MB/chunk → POST `{sessionId, chunkIndex, totalChunks, chunk}`; BE **append thẳng vào file `.part` theo offset trên disk** (KHÔNG giữ `Vec` chunk trong RAM — box .65 yếu, tránh OOM); finalize → validate size/magic-byte → tạo `files{kind:"upload"}`. Progress bar bằng XHR upload events.

**[Red-team #8] sessionId do BE sinh, bind user:** endpoint khởi tạo upload (`POST /api/upload/init`, đã auth) → BE sinh **UUID `sessionId` server-side**, lưu Mongo kèm `userId` + `expectedSize`. Mọi POST chunk phải tra session → **verify `session.userId == req.userId`** trước khi append; validate format UUID trước khi dựng path (chống IDOR + path-traversal). KHÔNG tin `sessionId` client tự bịa.

**[Red-team #7] Resumable stateless (chốt ngay, không đẩy cook):** tiến độ upload = **độ dài byte thực của file `.part` trên disk** (nguồn sự thật duy nhất) → tự phục hồi sau khi BE restart, KHÔNG cần bảng chunk-state trong RAM (tránh đúng bug research cảnh báo). FE resume = hỏi BE `.part` hiện dài bao nhiêu → gửi tiếp từ offset đó. (Recording open-ended của Phase 4 dùng endpoint streaming riêng — xem Phase 4.)

**[Red-team #14] Size cap có số cụ thể:** `MAX_FILE_SIZE_BYTES = 2GB` (chốt ở validate 2026-07-11; ~40% quota 5GB). Reject NGAY ở `init` nếu `expectedSize > 2GB` và khi `totalChunks × chunkSize` vượt — KHÔNG đợi finalize (nginx `client_max_body_size 500M` chỉ chặn 1 request, không chặn tổng file lắp ráp). <!-- Updated: Validation Session 1 - size cap 2GB -->

**[Red-team #3] Quota check tăng dần:** cộng dồn byte đã append của session vào usage tạm của user; abort sớm (413) khi `usage + session.written > QUOTA_BYTES` — không đợi finalize (tránh ăn hết disk shared trước khi bị chặn). Giới hạn số session mở đồng thời/user; sweep dọn `.part` mồ côi.

**Job queue (Mongo `jobs`):** tạo `jobs{type:"split", inputFileId, params:{criterion,value}, status:"queued"}`. Worker loop: `find_one_and_update({status:queued}→{running})` (atomic, concurrency=1) → chạy `SplitWorker` → cập nhật progress → `done` với `result:{segments:[...]}` hoặc `error`. FE nhận tiến độ qua **poll `GET /api/jobs/:id` mỗi 1-2s** (đơn giản, không cần proxy đặc biệt — job `-c copy` xong trong giây; giữ SSE để dành cho re-encode nặng Phase 4).

**[Red-team, sót khi dedup] Job `running` hồi phục sau crash/restart:** BE bị OOM-kill/panic + `systemd Restart=on-failure` → job đang chạy kẹt `running` vĩnh viễn (poll không bao giờ thấy `done`). Fix: **khi BE start, reset mọi `jobs{status:running}` → `queued`** (hoặc `failed` + cho retry). Thêm `heartbeatAt` cập nhật định kỳ; sweep coi job `running` quá X phút không heartbeat là stale → requeue.

**[Fold C4] trait Worker (honor quyết định #8):** định nghĩa `trait Worker { async fn process(&self, job) -> Result<JobResult> }`; `SplitWorker` impl nó (gọi `core-split`). `TranscribeWorker` = stub impl trả "chưa hỗ trợ". Cắm engine sau = thêm 1 `impl Worker`, không đụng queue/API (~10 dòng, làm ngay để lợi ích pluggable có thật, không retrofit sau).

**SplitWorker:** gọi `core-split::split(input, criterion, value, out_dir, ffmpeg_path, ffprobe_path, on_progress)` — bê logic `splitter.rs` (phút→`-segment_time`; MB→keyframe byte `-segment_times`; parts→duration/k; HEVC hvc1; `-c copy`). Output `/data/{userId}/outputs/{jobId}/`. ffmpeg spawn `nice 10` + **`-threads N`** (cùng CPUQuota systemd Phase 1). `ffmpeg_path`/`ffprobe_path` đọc từ config (Phase 1 #5), KHÔNG hard-code.

**[Red-team #15] ffmpeg chạy input không tin cậy (upload công khai):** magic-byte chỉ soát header, KHÔNG chặn CVE demuxer. Mitigation pragmatic (KHÔNG cần seccomp/container nặng cho tool nội bộ): **allowlist BẬT mặc định** (Phase 5) + **timeout job** (kill ffmpeg treo) + **pin & track version ffmpeg** (theo dõi CVE). Xem đây là residual risk có kiểm soát, không phải sandbox đầy đủ.

**Download:** `GET /api/jobs/:id/download` → zip streaming các đoạn (ownership check). Hoặc list từng đoạn `GET /api/files/:id`.

**FE (`web/`):** port `SplitView.vue` — chọn file (upload thay vì path local), probe hiển thị thời lượng (BE `/api/probe` hoặc trả sau upload), chọn tiêu chí, nút Cắt, progress, danh sách đoạn + tải zip.

## Related Code Files

- Create: `server/onescribe-be/src/upload.rs` (chunked append-to-disk + init/sessionId server-gen), `server/onescribe-be/src/jobs.rs` (queue + worker loop + trait Worker + poll endpoint `GET /api/jobs/:id`; reset running→queued khi start), `server/onescribe-be/src/worker/split_worker.rs`, `server/onescribe-be/src/models/{file,job}.rs`, `web/src/views/SplitView.vue`, `web/src/composables/use-upload.ts`, `web/src/composables/use-splitter.ts`
- Modify: `crates/core-split/src/lib.rs` (đảm bảo API dùng được ngoài Tauri), `server/onescribe-be/src/routes.rs`
- Delete: none

## Implementation Steps

1. `upload.rs`: endpoint chunked POST → mở file `.part` `append` theo `chunkIndex*chunkSize` (seek+write, không RAM buffer) → finalize (rename + magic-byte check qua `infer`/đọc header + size cap) → tạo `files` doc + probe duration (ffprobe qua core-split hoặc helper).
2. `jobs.rs`: tạo job (POST `/api/jobs`), worker task nền (`tokio::spawn`) poll+claim atomic, **reset `running`→`queued` khi BE start** + `heartbeatAt`, `trait Worker`, poll endpoint `GET /api/jobs/:id` (SSE để dành job re-encode nặng Phase 4), ownership.
3. `split_worker.rs`: gọi `core-split`; map progress → cập nhật `jobs.progress`; ghi `result`/`error`; ffmpeg `nice`.
4. Download zip streaming (`async-zip` hoặc tokio) + ownership.
5. FE SplitView + use-upload (init sessionId + progress + resume theo `.part` size) + use-splitter (tạo job, poll tiến độ, tải zip).
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
- **In-memory session (chunk state) mất khi restart BE — ĐÃ CHỐT (#7):** KHÔNG giữ chunk-state trong RAM; nguồn sự thật = độ dài byte `.part` trên disk → resume stateless sau restart. `sessionId` + `userId` + `expectedSize` lưu Mongo (nhẹ), không lưu nội dung chunk.
