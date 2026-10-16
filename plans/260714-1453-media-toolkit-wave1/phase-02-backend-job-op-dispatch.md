---
phase: 2
title: "Backend job op dispatch"
status: pending
priority: P1
dependencies: [1]
---

# Phase 2: Backend job op dispatch

## Overview
Mở rộng job queue để chạy convert/extract/trim ngoài split, KHÔNG thêm endpoint, KHÔNG phá contract split. **Tái dùng field `type`** (dispatch đã theo `job.job_type`), thêm params có kiểu, output đơn file trả `fileId`.

<!-- Updated: Validation Session 1 — op→type; criterion/value→Option; params typed; audio-only (bỏ mp4). -->

## Requirements
- Functional: `POST /api/jobs` nhận `type` (mặc định "split") + params op-specific; worker chạy đúng loại; job done trả tham chiếu output (fileId) để FE tải.
- Non-functional: split cũ hành vi y nguyên; ownership/quota/size-cap/retention áp như split; output kind=`output` (hiện ở "Tệp trên hệ thống", đếm quota).

## Architecture
- **Dispatch (đã có):** `jobs::run_job` match `job.job_type`: `"split"` → SplitWorker; thêm arm `"convert" | "extract"` → ConvertWorker; `"trim"` → TrimWorker; `_` → TranscribeWorker (giữ). (`worker/mod.rs` doc: "dispatch tĩnh theo job.type".)
- `models/job.rs`:
  - `criterion`/`value` → **`Option<SplitCriterion>` / `Option<f64>`** (op mới không dùng; split vẫn set). Kiểm serde: split cũ có sẵn → OK.
  - Thêm params có kiểu: `#[serde(skip_serializing_if=Option::is_none)] format: Option<String>`, `start_s: Option<f64>`, `end_s: Option<f64>`.
  - `create_media_job(db, user, input_file_id, job_type, format, start_s, end_s)`; `create_split` gọi lại nó với type=split (DRY).
- `jobs.rs` handler `create`: parse `type` (mặc định split). **Validate ở hàm pure** (test không cần DB):
  - split → yêu cầu criterion/value (giữ hành vi).
  - convert → `format` ∈ allowlist **{mp3, m4a, wav}** (Wave 1 audio-only; KHÔNG mp4).
  - extract → input phải có video (hasVideo); `format` audio {mp3,m4a,wav}, mặc định mp3.
  - trim → `start_s < end_s`, ≥0, ≤ duration input (probe); output mp3.
- `worker/convert_worker.rs` (dùng cho convert & extract), `worker/trim_worker.rs`:
  - Lấy input (ownership), out_dir `{data}/{user}/outputs/{jobId}/`, gọi `core_split::convert_file`(extract → `drop_video=true`)/`trim_file`, đăng ký 1 output file doc (kind=output, jobId), trả `OutputSeg` đơn.
  - Recording webm input: convert/trim **re-encode** nên khác split `-c copy`. Kiểm `-ss` trên webm re-encode có chính xác không; nếu lệch → remux-fix-cues trước (tái dùng `remux_copy_to_mkv`, như SplitWorker).
- Output đơn file: `run_job` lưu `segments` (1 phần tử) → FE đọc `result.segments[0].fileId` để tải. KHÔNG zip cho op mới.

## Related Code Files
- Modify: `server/scribe-be/src/models/job.rs` (op/params, create_media_job)
- Modify: `server/scribe-be/src/jobs.rs` (create handler: parse+validate op)
- Modify: `server/scribe-be/src/worker/mod.rs` (dispatch theo op)
- Create: `server/scribe-be/src/worker/convert_worker.rs`, `worker/trim_worker.rs`
- Modify: `server/scribe-be/src/worker/split_worker.rs` (không đổi logic; chỉ nếu refactor chung)

## Implementation Steps (TDD — test trước phần thuần logic)
1. **Test trước** (unit, không cần ffmpeg/DB) — đặt validate ở hàm pure (vd `parse_job_request(body) -> Result<..., AppError>`):
   - `type` không hợp lệ → 400; convert `format` ∉ {mp3,m4a,wav} (gồm mp4) → 400; trim start_s≥end_s hoặc <0 → 400; split thiếu criterion/value → 400 (giữ hành vi).
   - Serde: Job split cũ (có criterion/value) deserialize OK sau khi đổi sang Option; job type mới thiếu criterion/value deserialize OK.
2. Chạy → đỏ.
3. Impl model (criterion/value→Option + format/start_s/end_s) + validate pure + dispatch arm trong run_job + 2 worker mới (convert_worker dùng cho convert & extract, trim_worker).
4. Chạy `cargo test -p scribe-be` (unit) xanh; `cargo check` + `cargo clippy` sạch.
5. Integration thủ công (cần ffmpeg+Mongo, không automate): tạo job convert/extract/trim thật → done → tải output đúng. Ghi lại kết quả.

## Success Criteria
- [ ] Unit test validate `type` + serde criterion/value Option (split cũ + type mới) viết trước, đỏ→xanh.
- [ ] `POST /api/jobs` chạy được convert/extract/trim (audio); done trả fileId output.
- [ ] Split cũ KHÔNG đổi (test + smoke) — criterion/value Option không phá deserialize doc split.
- [ ] convert `format=mp4` bị từ chối 400 (Wave 1 audio-only).
- [ ] `cargo check`/`clippy` sạch; output đếm quota + hiện ở manage_list.

## Risk Assessment
- **Serde criterion/value → Option**: doc split cũ (queued/running lúc deploy) phải deserialize OK sau khi đổi Option — test serde bắt buộc trên cả doc split-cũ lẫn type-mới. (Không còn field `op`; dispatch qua `type` sẵn có nên không thêm trục phân loại.)
- **webm `-ss` accuracy**: convert/trim re-encode seek trên webm streaming có thể lệch → remux-fix-cues trước (tái dùng `remux_copy_to_mkv`). Kiểm ở integration; nếu OK thì bỏ remux cho nhẹ.
- **Concurrency=1 worker**: job convert/trim dài chiếm worker → job khác chờ. Audio-only (Wave 1) nên tương đối nhanh; MP4 video đã HOÃN chính vì rủi ro treo lâu. Note cho Wave sau khi mở video.
