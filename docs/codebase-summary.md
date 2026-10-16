# Codebase Summary — Scribe

Tổng quan kiến trúc + module chính. Cập nhật 2026-10-16 (bỏ app desktop; chỉ còn web và iPhone).

Scribe = app ghi/cắt/transcribe media trong hệ sinh thái hoandat. Repo có **2 ứng dụng**:

| App | Vị trí | Stack | Vai trò |
|-----|--------|-------|---------|
| **Web** | `web/` + `server/scribe-be/` + `crates/core-split/` | React 19 + Vite 7 + Tailwind 4 (token từ localman-next) · BE Rust/axum (musl static) · MongoDB · Firebase Auth | Ghi (browser) + cắt file + **chép lời bằng OpenAI** (key của server, đo phút theo user, quota tháng + trần chi phí; chỉ email trong `TRANSCRIBE_EMAILS` tới khi bật `TRANSCRIBE_OPEN`) |
| **iPhone** | `web/ios/` (Capacitor 8, đóng gói bản build của `web/`) | Swift plugin + cùng mã React | Ghi nền khi khoá màn hình theo đoạn 1 phút, lưu bền, tự tải lên; dùng chung backend web. Chi tiết: [ios-app.md](./ios-app.md) |


---

## Web app — Frontend (`web/`)

React 19 + Vite 7 + Tailwind 4 + TanStack Query + Radix + Firebase JS SDK. Token màu lấy từ localman-next (`styles/tokens.css`, kiểm tương phản bằng `npm run check:tokens`).

- **Shell** (`layout/`): top bar + tab Ghi âm / Cắt file / Transcript (tab Transcript chỉ hiện khi `me.canTranscribe`; `transcript-nav.ts` cho trang khác mở một transcript).
- **pages/**: `login-page`, `record/` (nguồn ghi, theo dõi mức âm, thẻ ghi native trên iPhone, Lịch sử ghi gom theo ngày và theo phiên iPhone, panel "Chép lời", hộp xác nhận bản ghi dài), `split/` (cắt file), `transcript/` (danh sách transcript theo phiên + màn hình đọc: chạm câu để phát, tự sang đoạn kế, sửa câu theo đoạn, đổi tên, chép lại từng đoạn, xuất, xoá).
- **features/** (logic, không UI): `files` (danh sách/xoá/gom nhóm), `upload` (chunk), `split` (job cắt), `recorder` (trình duyệt + bản native qua plugin Swift), `transcript` (API chép lời, ghép phiên), `transcript-docs` (API `/api/transcript-docs`, query có poll `/status` nhẹ, `reader-model` thuần).
- **lib/**: `api.ts` (`apiFetch`, `isNativeShell`, origin backend khi chạy trong app), `download.ts` (tải/phát qua plugin native trên iPhone), `native.ts` (khai báo plugin Swift).
- Dev proxy `/api` → `127.0.0.1:8787`; prod nginx cùng origin.

## Web app — Backend (`server/scribe-be/`)

Rust/axum → **1 static musl binary**, bind `127.0.0.1:8787`, secret nhúng compile-time (`build.rs`/`option_env!`), session key HS256 sinh runtime, TLS out-bound = rustls (musl sạch).

**Modules** (`src/`):
- `main.rs` — bootstrap: connect Mongo, tạo index, reset job kẹt, khởi `jobs::start_worker` + `retention::start_sweeper`, serve.
- `config.rs` — hằng: `RETENTION_DAYS=14`, `QUOTA_BYTES=5GB`, `MAX_FILE_SIZE_BYTES=2GB`, Firebase project, allowlist.
- `auth.rs` — Firebase ID token → verify (JWKS securetoken + email_verified + allowlist) → session cookie HttpOnly+Secure+SameSite=Lax; audit-log; middleware `require_auth`.
- `jwks.rs`, `session.rs` — verify token + issue/verify session (jsonwebtoken).
- `upload.rs` — chunked upload (init/chunk/status/finalize) append-disk, quota tăng dần, magic-byte (`infer`).
- `transcribe/` — chép lời bằng OpenAI `gpt-4o-mini-transcribe`: `POST/GET /api/transcripts`, `GET /api/transcripts/export` (txt/srt, ghép cả phiên iPhone); hàng đợi riêng trong collection `transcripts`; cắt mảnh ≤45 giây tại khoảng lặng (ffmpeg `silencedetect`), nén Opus, gọi song song 4 mảnh. Key OpenAI của server đọc từ tệp lúc khởi động (`Transcriber`, có cờ ngắt khi key hỏng). Tiền: `POST /api/transcripts` giữ chỗ phút trong `models/usage.rs` (user → quỹ free → trần tổng, `$inc` có điều kiện, không cần transaction) rồi xếp hàng; worker cộng `sentS` trước mỗi lần gửi audio; lượt kết thúc (xong, lỗi, xoá) đóng sổ đúng một lần theo `runId` và quyết toán `sentS − chargedS`, ghi `transcribe_runs` để đối soát. Restart: lượt dở chạy lại, cắt ngang lần hai thì dừng. `admin.rs` — CLI `scribe-be admin set-plan|usage`. `transcribe/docs.rs` — transcript theo phiên ghi (`/api/transcript-docs`, xem bảng API): gom đoạn bằng `file::session_key` (`s:<sessionId>` cho phiên iPhone, `f:<fileId>` cho bản ghi một file), ghép kết quả máy với tiêu đề + câu sửa lúc đọc. Câu sửa gắn `runId` của lượt chép: chép lại thì phần sửa của đoạn đó tự hết hiệu lực; PUT ghi có điều kiện, chen lượt chép lại → 409.
- `access_log.rs`, `audit.rs` — log mọi request + collection `audit_events` (TTL 180 ngày) cho hành vi người dùng.
- `recordings.rs` — **Mode A streaming**: init/chunk/finalize open-ended cho MediaRecorder (mirror upload, coll `recording_sessions` riêng) → `files{kind:recording}`. Finalize **remux `-c copy -f webm`** để dựng Cues+Duration (webm MediaRecorder streaming thiếu → không tua được); remux lỗi → fallback rename giữ container gốc.
- `files.rs` — `GET /api/files` (lịch sử ghi) · `GET /api/files/:id` (stream + **HTTP Range** cho player tua) · `DELETE /api/files/:id` (xoá thủ công, ownership) · `GET /api/me/usage` (quota).
- `jobs.rs` + `worker/split_worker.rs` — job queue cắt (claim atomic, concurrency=1). Mode B (upload webm): disk-free ≥2× → remux `-c copy -f matroska` → segment → dọn `fixed.mkv` → đoạn `kind:output` (tải zip). **Mode A (`recordingSplit`)**: input recording đã seekable → segment thẳng webm→webm + probe mỗi đoạn → đoạn `kind:recording` (vào Lịch sử ghi, phát/tua riêng) + **xoá bản ghi gốc**.
- `rate_limit.rs` — limiter in-memory theo real IP (`X-Real-IP` nginx-set): auth 20/60s, protected 300/60s (miễn `/chunk`).
- `retention.rs` — sweep 1h: xoá file quá hạn 14 ngày (**disk TRƯỚC → doc**, KHÔNG TTL native), dọn orphan `.part`/session/job cũ và `transcript_docs` không còn audio, panic-isolation. `file::delete_doc(&FileDoc)` là nơi duy nhất xoá doc file: xoá kèm `transcripts` và câu sửa của đoạn.
- `models/{file,job,user,transcript,transcript_doc}.rs`, `state.rs`, `error.rs`, `db.rs`, `routes.rs`.

**core-split** (`crates/core-split/`): `split_file` (ffmpeg segment `-c copy`), `remux_copy_to_mkv` (fix cues webm), `probe_file`/`size_cut_times`, `compute_segment_time`.

### API endpoints (web)

| Method | Route | Ghi chú |
|--------|-------|---------|
| GET | `/api/health` | liveness (không auth) |
| POST | `/api/auth/session` · `/api/auth/logout` | Firebase → cookie (rate-limit 20/60s) |
| GET | `/api/me` · `/api/me/usage` | user + `transcription` (gói, phút đã dùng/hạn mức, ngày đặt lại) / quota dung lượng |
| POST | `/api/upload/{init,chunk,status,finalize}` | cắt file (upload) |
| POST | `/api/recordings/{init,chunk,finalize}` | ghi (streaming) |
| GET/DELETE | `/api/files/:id` | phát (Range) / xoá |
| GET | `/api/files` | lịch sử ghi |
| POST/GET | `/api/jobs` · `/api/jobs/:id` · `/api/jobs/:id/download` | job cắt + tải zip |
| POST/GET | `/api/transcripts` · `/api/transcripts/export` | xếp chép lời (giữ chỗ phút; 403 `QUOTA_EXCEEDED`/`CAP_REACHED` kèm số, 503 khi tạm ngưng) / trạng thái theo file / xuất txt·srt có áp câu sửa |
| GET | `/api/transcript-docs` | danh sách transcript theo phiên (projection, có preview) |
| GET/PATCH/DELETE | `/api/transcript-docs/{key}` | chi tiết (đoạn, `offsetS`, `missingIndexes`) / đổi tên / xoá transcript, giữ audio |
| GET | `/api/transcript-docs/{key}/status` | trạng thái từng đoạn, không kèm câu (poll khi đang chép) |
| PUT | `/api/transcript-docs/{key}/segments/{fileId}` | lưu câu sửa `{runId, texts}`; 409 khi đoạn vừa được chép lại |

Nhóm `/api/transcript-docs` có trần body 1 MiB; chỉ cần đăng nhập và đúng chủ dữ liệu.

### MongoDB (database `$DB_NAME`)

`users` (firebaseUid unique) · `files` (kind `upload|output|recording`, `size`, `durationS`, `hasVideo`, `jobId`, `expiresAt` index thường) · `jobs` (queue cắt) · `upload_sessions` / `recording_sessions` (append-disk) · `transcripts` (kết quả máy + hàng đợi chép lời, `_id` = id file) · `transcript_docs` (`{userId, key}` unique, tạo đồng bộ lúc khởi động: `title`, `edits.<fileId> = {runId, texts}`) · `transcribe_usage` (sổ giây theo tháng giờ VN: `u:<userId>:<YYYY-MM>`, `free:<YYYY-MM>`, `all:<YYYY-MM>`) · `transcribe_runs` (mỗi lượt đã quyết toán, chỉ thêm) · `transcribe_marks` (mốc 80%/100% đã báo) · `users.plan` / `planUntil` (gói `free|plus`) · `audit_events` (TTL 180 ngày; sự kiện transcript không chứa tiêu đề hay câu).

### Storage (`{data_dir}/{userId}/`)

`tmp/*.part` (đang upload/ghi) · `uploads/` · `recordings/` · `outputs/{jobId}/` + `{jobId}.zip`.

---

## Deploy (web)

Build musl qua Docker (`scripts/build-musl.sh`) hoặc `cargo zigbuild` trên macOS → binary `$DEPLOY_APP_DIR/$DEPLOY_SERVICE` (systemd) + FE dist `$DEPLOY_WEB_ROOT` (giá trị trong `.env.production`); nginx `:80` (CF DNS proxied lo TLS) tại domain prod (`DEPLOY_DOMAIN`). Chi tiết: [deploy-web-production.md](./deploy-web-production.md); app iPhone: [ios-app.md](./ios-app.md).

## Tính năng mới (session 2026-07-12)

- **Refactor UI web theo giao diện app desktop cũ**: theme dark của app + BrandMark + Be Vietnam Pro + shell 3 tab (thay giao diện trắng/xanh dương cũ).
- **Mode A — Ghi (web)**: mic + màn hình/system audio (Chrome/Edge) → MediaRecorder streaming upload → finalize remux webm seekable (tua được) → lịch sử + player (stream/Range); tùy chọn cắt theo phút → đoạn thành các bản ghi riêng trong Lịch sử ghi (thay bản ghi gốc), phát/tua từng đoạn.
- **Retention & hardening**: auto-purge 14 ngày (sweep), rate-limit real-IP, `/api/me/usage` + QuotaBar, quota 5GB + size cap 2GB.
- **Xoá recording thủ công**: `DELETE /api/files/:id` + nút Xoá (xác nhận inline).

## Unresolved / follow-up

- Transcript giai đoạn 2–3 (tìm kiếm, thư mục/nhãn, chia sẻ, docx) chưa làm.
- Chưa có unit test cho `RateLimiter` (logic pure, mock được).
- App iPhone: QA trên máy thật (đăng nhập, ghi khoá màn hình, chép lời) còn chờ.
- QA thủ công ghi mic/màn hình thật (getUserMedia/getDisplayMedia không automate được).
