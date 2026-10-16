---
phase: 1
title: "Backend: transcript theo phiên + API"
status: done
priority: P1
effort: "M"
dependencies: []
---

# Phase 1: Backend — transcript theo phiên + API

<!-- Updated: Red Team 2026-10-16 - gắn edits với runId, đổi chữ ký delete_doc + sweep mồ côi, kiểm đầu vào, endpoint trạng thái nhẹ, gom phiên một nguồn, audit không chứa nội dung -->

## Goal

Backend trả transcript theo phiên ghi (gom các đoạn), lưu tiêu đề và câu đã sửa gắn với lượt chép, xoá transcript; dọn dữ liệu đúng và tự lành khi audio bị xoá.

## Files

- Create: `server/scribe-be/src/models/transcript_doc.rs` — collection `transcript_docs`, index, CRUD
- Create: `server/scribe-be/src/transcribe/docs.rs` — handler các route mới, gom phiên, áp phần sửa
- Modify: `server/scribe-be/src/models/file.rs` — `session_key(name, id)`; đổi `delete_doc(db, id)` (`:227-234`) thành `delete_doc(db, &FileDoc)`
- Modify 3 nơi gọi `delete_doc`: `server/scribe-be/src/files.rs:98-99`, `server/scribe-be/src/retention.rs:64-66`, `server/scribe-be/src/jobs.rs:272-276`
- Modify: `server/scribe-be/src/retention.rs` — thêm `sweep_orphan_transcript_docs` (cùng nhịp 1 giờ, như `sweep_stale_jobs` `:86-95`)
- Modify: `server/scribe-be/src/models/transcript.rs` — hàm `statuses_for_files` (projection `status, progress, runId`, như `statuses` `:159-174`); hàm `previews` (projection + `$slice` 3 câu đầu)
- Modify: `server/scribe-be/src/transcribe/mod.rs` — `export_files` (`:248-269`) gom phiên bằng `file::session_key` thay cho `strip_prefix`; `export` áp phần sửa ở cả chế độ `ids` và `session`
- Modify: `server/scribe-be/src/routes.rs` — đăng ký route mới trong một nhóm có `DefaultBodyLimit::max(1 MiB)` riêng (nhóm protected hiện dùng 8 MiB, `:32-35`)
- Modify: `server/scribe-be/src/main.rs` — tạo index `transcript_docs` **đồng bộ trước khi bind cổng**, lỗi thì dừng (các index khác vẫn spawn như cũ, `:55-70`)
- Modify: `server/scribe-be/src/audit.rs` — sự kiện `transcript.rename`, `transcript.edit`, `transcript.delete`, `transcript.rerun`

## Data model

```text
transcript_docs {
  _id: ObjectId, userId: String,
  key: String,                 // "s:<sessionId>" (iPhone) | "f:<fileId hex>" (bản ghi trình duyệt)
  title: Option<String>,       // None = UI tự đặt tên mặc định
  edits: { "<fileId hex>": { runId: String, texts: [String] } },
  createdAt, updatedAt
}
index: { userId: 1, key: 1 } unique
```

- `session_key(name, id)`: `name` khớp `^([0-9a-z-]{8,64})#(\d+)$` → `s:<nhóm 1>`; ngược lại → `f:<hex id>`. Test dùng cùng ví dụ với `web/src/features/files/recording-groups.test.ts`. Tiền tố `s:`/`f:` để tên do client đặt không trùng được key của file khác.
- **Phần sửa chỉ có hiệu lực khi `edits.<f>.runId == transcripts.runId` của đoạn đó và số câu bằng nhau.** Lệch thì bỏ qua (đoạn hiện câu máy, `edited=false`). Chép lại một đoạn tự làm phần sửa cũ hết hiệu lực vì `enqueue` đổi `runId` (`transcript.rs:59-80`); không cần dọn đồng bộ.

## API

Đọc/sửa/xoá transcript **của chính mình** chỉ cần đăng nhập (giống `GET /api/transcripts` và export hiện tại, `transcribe/mod.rs:180-210`); chỉ việc **xếp chép lời** cần `transcribe_allowed` như cũ. Mọi route kiểm ownership theo `userId`.

| Method | Route | Body / trả về |
|---|---|---|
| GET | `/api/transcript-docs` | `[{ key, title, createdAt, durationS, segments, done, failed, pending, preview }]` — phiên có ≥ 1 đoạn đã xếp chép lời, mới nhất trước. Dùng projection: không kéo `text`/toàn bộ `sentences` |
| GET | `/api/transcript-docs/{key}` | `{ key, title, createdAt, segments: [{ fileId, index, offsetS, durationS, status: none\|queued\|running\|done\|failed, progress, error, runId, edited, sentences }], missingIndexes }` |
| GET | `/api/transcript-docs/{key}/status` | `[{ fileId, status, progress, runId }]` — endpoint nhẹ để poll khi đang chép |
| PATCH | `/api/transcript-docs/{key}` | `{ title: string \| null }` — trim, ≤ 120 ký tự, rỗng/null = về mặc định |
| PUT | `/api/transcript-docs/{key}/segments/{fileId}` | `{ runId, texts: string[] }` — ghi có điều kiện: đoạn thuộc phiên, `status=done`, `runId` khớp `transcripts.runId` hiện tại, `texts.len()` = số câu máy (≤ 5000), mỗi câu ≤ 2000 ký tự; sai → 409 |
| DELETE | `/api/transcript-docs/{key}` | xoá doc `transcripts` của mọi đoạn trong phiên + doc `transcript_docs`; **không** đụng audio |

- `{fileId}` parse bằng `ObjectId::parse_str` rồi mới dùng `to_hex()` trong field path; `{key}` phải khớp `^(s:[0-9a-z-]{8,64}|f:[0-9a-f]{24})$`. Sai → 400.
- PATCH/PUT chỉ upsert `transcript_docs` **sau khi** đã tìm thấy ≥ 1 file `recording` của user thuộc phiên; không có → 404.
- `offsetS` = tổng `durationS` các đoạn **đang có** đứng trước; `durationS` thiếu hoặc 0 → dùng `end` của câu cuối. Đoạn thiếu (index bị nhảy) trả trong `missingIndexes` để UI hiện "thiếu đoạn N".
- Audit `details` chỉ gồm `{ key, fileId?, sentences?: số câu, titleLen? }` — **không** ghi tiêu đề hay câu (audit giữ 180 ngày, `audit.rs:15-16`).

## Tasks

1. `file::session_key` + test (iPhone, trình duyệt, tên lạ, selftest `RecorderPlugin.swift:73`, chữ hoa).
2. `models/transcript_doc.rs`: `ensure_indexes` (đồng bộ), `get_many(user, keys)`, `set_title`, `set_edits` (upsert `$set edits.<hex> = {runId, texts}`), `unset_edits(user, key, fileHex)`, `delete(user, key)`, `delete_orphans(db)`.
3. `transcribe/docs.rs`: gom `file::list_recordings` theo `session_key`, ghép `statuses_for_files`/`previews` (danh sách) hoặc `list_owned` theo id (chi tiết), áp `edits` theo luật `runId`; 6 handler; audit.
4. PUT ghi có điều kiện: đọc doc `transcripts` lấy `runId`+`status`+số câu, ghi `set_edits`, rồi đọc lại `transcripts.runId`; nếu đã đổi thì `unset_edits` và trả 409 (chặn cửa sổ đua với chép lại).
5. `export`: gom phiên bằng `session_key` (một nguồn với route mới); với mỗi file tra `transcript_docs` theo `{userId, key: {$in}}` một lần, áp phần sửa theo luật `runId`.
6. `delete_doc(db, &FileDoc)`: xoá file doc, xoá `transcripts` của file, rồi `unset_edits` (lỗi chỉ `warn`, không trả lỗi sau khi đĩa đã xoá). Sweep `delete_orphans`: xoá `transcript_docs` không còn file `recording` nào khớp `key` — đây là cơ chế đảm bảo, bước trong `delete_doc` chỉ là tối ưu.
7. Không thêm `#[serde(default)]` cho `runId` (doc thiếu `runId` sẽ bị `update_run` coi như đã huỷ — xem Phase 2).

## Verification

- `cargo clippy --workspace --all-targets -- -D warnings` sạch.
- `cargo test --workspace`, test mới: `session_key`; gom phiên + `offsetS` + `missingIndexes` + `durationS=0`; luật áp sửa (khớp / lệch `runId` / lệch số câu); validate `key`, `fileId`, title; export ở hai chế độ dùng bản sửa.
- Test Mongo thật (`#[ignore]`, DB dev lab 176, `TRANSCRIPT_TEST_MONGO_URI` + `TRANSCRIPT_TEST_DB_NAME`): đổi tên → sửa → chép lại làm sửa hết hiệu lực → PUT với `runId` cũ bị 409 → xoá file cuối → sweep xoá doc phiên; hai upsert đồng thời chỉ tạo một doc.
- Dev local bằng `curl` có cookie: mọi route trả đúng 200/400/404/409; body > 1 MiB → 413.

### Kết quả (2026-10-16)

- `cargo clippy --workspace --all-targets -- -D warnings` sạch; `cargo test --workspace` đạt (scribe-be 22 test, test mới: `session_key`, validate khoá, gom phiên + `offsetS` + `missingIndexes` + `durationS=0`, luật áp sửa, tiêu đề/câu).
- Test Mongo thật trên dev lab 176: `transcript_docs_lifecycle_on_real_mongo` và `transcript_queue_lifecycle_on_real_mongo` đều đạt (hai upsert đồng thời ra một doc, chép lại làm sửa hết hiệu lực, xoá đoạn dọn câu sửa, sweep xoá doc phiên mồ côi).
- Dev local bằng HTTP có cookie: 34/34 kiểm đạt (200/400/404/409, body > 1 MiB → 413, export hai chế độ dùng bản sửa, audit không chứa tiêu đề/câu, xoá transcript giữ audio).
- Khác plan: danh sách/status trả `{ items: [...] }` như các route cũ; danh sách thêm `name` (tên file bản ghi trình duyệt) và `untranscribed`; chi tiết thêm `name`, `hasVideo`. Sự kiện `transcript.rerun` ghi trong `POST /api/transcripts` khi có file đã chép trước đó.

## Risks

- Phiên lớn (49 đoạn): chi tiết ~50–100 KB, chỉ tải khi mở và khi có đoạn mới `done`; poll dùng `/status` (vài KB).
- Đoạn của một phiên hết hạn ở các vòng sweep khác nhau (`file.rs:12-15`): transcript còn một phần trong tối đa 1 giờ; chấp nhận theo quyết định "xoá cùng audio".
