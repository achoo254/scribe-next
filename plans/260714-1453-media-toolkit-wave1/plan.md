---
title: "Scribe Web — Media toolkit Wave 1 (Convert/Extract/Trim)"
description: "Thêm Convert/Export định dạng, Tách audio từ video, Trim theo mốc — tái dùng core-split + job queue. Non-AI."
status: cancelled
priority: P2
branch: "main"
tags: [web, media, ffmpeg, core-split]
blockedBy: []
blocks: []
created: "2026-07-14T08:30:56.431Z"
createdBy: "ck:plan"
source: skill
---

# Scribe Web — Media toolkit Wave 1 (Convert/Extract/Trim)

> **Huỷ (2026-10-19):** cả 3 phase chưa làm (code không có job `convert`/`extract`/`trim`). Hướng sản phẩm từ 10/2026 chuyển
> sang chép lời, biên bản và kho bản ghi; plan này không nằm trong lộ trình hiện tại. Mở lại bằng plan mới nếu cần.

## Overview

Biến Scribe từ "ghi + cắt" thành mini media-toolkit non-AI. Lấp lỗ hổng lớn nhất: bản ghi ra `.webm` khó dùng ngoài app. Thêm 3 thao tác, TÁI DÙNG y nguyên job queue + worker + core-split + quota + retention:

- **Convert/Export**: recording/upload → MP3 / M4A / WAV (audio-only ở Wave 1; MP4 video hoãn).
- **Tách audio từ video**: screen recording (video) → MP3 (`-vn`).
- **Trim theo mốc**: cắt 1 đoạn in/out cụ thể (khác "Cắt file" chia đều).

Input brainstorm: `plans/reports/brainstorm-non-ai-features-260714-1453-media-toolkit-roadmap-report.md`.
Chế độ: **--tdd** (tests-first ở phase Rust có test harness; FE gate = typecheck/build/QA thủ công).

## Quyết định thiết kế (đã chốt qua validate 2026-07-14)

- **KHÔNG endpoint mới, KHÔNG field `op`**: **tái dùng field `type`** hiện có (dispatch đã theo `job.job_type` trong `run_job`). Thêm giá trị `type` = `convert` / `extract` / `trim` + arm dispatch. Mọi job đã có `type` → không cần serde-default. `criterion`/`value` đổi thành **`Option`** (op mới không dùng). Contract split (`/api/jobs` với criterion/value + type mặc định split) KHÔNG đổi.
- **Params op-specific = field Option có kiểu** (KHÔNG dùng Document chung): thêm `format: Option<String>`, `start_s: Option<f64>`, `end_s: Option<f64>` vào `Job`.
- **Wave 1 = AUDIO-ONLY**: convert/extract/trim chỉ xuất audio (mp3/m4a/wav). **MP4 video-convert HOÃN** (re-encode video tốn CPU trên .65 không GPU; tránh job treo lâu chiếm worker concurrency=1). Extract-audio nhận input video nhưng xuất audio.
- **Trim → MP3** (re-encode, mốc chính xác). Không cho chọn format trim ở Wave 1.
- **Output đơn file**: ghi output kind=`output`, `result.segments[0].fileId` → FE tải trực tiếp `/api/files/:id?download=1` (đã có). KHÔNG zip cho op mới. Split vẫn zip như cũ.
- **Codec audio**: mp3=`libmp3lame`, m4a=`aac`, wav=`pcm_s16le`.
- Quota/size-cap/retention/ownership áp như split (output đếm quota, hiện ở "Tệp trên hệ thống").

## Phases

| Phase | Name | Status |
|-------|------|--------|
| 1 | [Core media ops (core-split)](./phase-01-core-media-ops-core-split.md) | Pending |
| 2 | [Backend job op dispatch](./phase-02-backend-job-op-dispatch.md) | Pending |
| 3 | [Frontend toolkit UI](./phase-03-frontend-toolkit-ui.md) | Pending |

Thứ tự bắt buộc: 1 → 2 → 3 (2 phụ thuộc 1; 3 phụ thuộc 2).

## Acceptance criteria (toàn plan)

- [ ] Recording webm → export MP3 tải được, phát đúng, đúng thời lượng.
- [ ] Screen recording (video) → tách audio ra MP3 (không stream video trong output).
- [ ] Trim in/out (vd 00:10→00:40) → file ~30s đúng nội dung.
- [ ] Job hiển thị tiến độ + lỗi ffmpeg → failed có thông báo, như split.
- [ ] Split cũ (`/api/jobs` criterion/value) KHÔNG đổi hành vi (backward-compat).
- [ ] `cargo test -p core-split` xanh; BE `cargo check`/`clippy` sạch; FE `vue-tsc` + `vite build` EXIT 0.

## Dependencies

Không có phụ thuộc chéo plan khác (web split/record đã DONE; các plan desktop/cancelled không liên quan).

## Out of scope (Wave sau)

Ghép file, nén bitrate, chuẩn hoá âm lượng, **MP4 video-convert** (Wave 1-A sau); Tổ chức/Chia sẻ/Trải nghiệm ghi (Wave 2/3/4).

## Validation Log

### Session 1 — 2026-07-14

**Verification Results (Standard tier, 3 phase):**
- Claims checked: ~8. Verified: 6 · Failed: 2 · Unverified: 0.
- VERIFIED: core-split `split_file`/`remux_copy_to_mkv`/`probe_file`; test `tests/split_file_ffmpeg.rs`; `/api/files/:id?download=1`; FE poll/download helpers.
- FAILED→sửa: (a) dispatch theo `job.type` (không phải field mới) — `worker/mod.rs`+`jobs.rs:145` match `job.job_type`; (b) `criterion`/`value` là field bắt buộc (`SplitCriterion` enum) → phải `Option` cho op mới.

**Quyết định phỏng vấn:**
1. Job model → **tái dùng field `type`** (convert/extract/trim), criterion/value→Option. (thay cho `op` field trong bản plan gốc)
2. MP4 video-convert → **HOÃN**, Wave 1 audio-only.
3. Trim → **xuất MP3** (không cho chọn format).
4. Params → **field Option có kiểu** (format/start_s/end_s), không dùng Document.

**Whole-Plan Consistency Sweep:** đã rà — phase-02 (op→type, Option, audio-only), phase-01 (format audio {mp3,m4a,wav}, bỏ nhánh MP4 video, trim→mp3), phase-03 (bỏ MP4 khỏi lựa chọn format, trim→mp3) đã đồng bộ với decisions. Không còn tham chiếu `op` field. 0 mâu thuẫn tồn đọng.
