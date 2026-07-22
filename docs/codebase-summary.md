# Codebase Summary — OneScribe

Tổng quan kiến trúc + module chính. Cập nhật 2026-07-12 (sau Phase 4 Ghi + Phase 5 Retention + xoá thủ công + refactor UI web bám desktop).

OneScribe = sản phẩm **One Series (iNET)** ghi/cắt/transcribe media. Repo có **2 ứng dụng** dùng chung design system iNET ViUI (theme dark `inetOneDark` — primary xanh lá `#49c98d`, accent cam `#f7ad30`, nền tối):

| App | Vị trí | Stack | Vai trò |
|-----|--------|-------|---------|
| **Desktop** | `src/` + `src-tauri/` | Tauri v2 (Rust) + Vue 3 + Vuetify 3 + ViUI local | Ghi mic/màn hình/system + cắt file + **transcribe offline** (PhoWhisper) |
| **Web** | `web/` + `server/onescribe-be/` + `crates/core-split/` | Vue 3/Vite + `@hobui/viui` npm · BE Rust/axum (musl static) · MongoDB · Firebase Auth | Ghi (browser) + cắt file, **BỎ AI transcribe** (box .65 không GPU) |

---

## Desktop app (`src/`, `src-tauri/`)

- **UI** (`src/`): `App.vue` shell 3 tab (Ghi âm / Cắt file / Transcript) · `views/` (RecordView, SplitView, TranscriptView, RegionSelectorOverlay, RecordBorder*) · `components/` (BrandMark logo, LevelMeter, TranscriptPlayer, …) · `design-system/vi-ui/` wrapper Vi* local · `plugins/vuetify.ts` theme `inetOneDark` + Tabler iconset · `tokens/one-series.ts` token màu.
- **Native** (`src-tauri/src/`): `lib/commands/ffmpeg/settings/screen.rs` · `audio/encoder.rs` (ffmpeg args audio + ddagrab→MP4 ghi màn hình).
- **Sidecar**: ffmpeg/ffprobe (LGPL) + `transcriber` (faster-whisper PyInstaller).
- 3 chế độ: Ghi (mic cpal / system WASAPI loopback / màn hình ddagrab — Windows), Cắt (`src-tauri/src/splitter.rs`), Transcript (PhoWhisper, CPU int8 / GPU CUDA).

## Web app — Frontend (`web/`)

Vue 3 + Vite + `@hobui/viui` (Vi* npm) + Vue Router + Firebase JS SDK. Theme `inetOneDark` cấu hình trong `main.ts` (map token One Series vào Vuetify), font **Be Vietnam Pro** (Google Fonts), favicon = app icon desktop.

- **Shell** (`views/HomeView.vue`): header (BrandMark + "OneScribe" + email + Đăng xuất) + **QuotaBar** + `ViTabs` 3 tab (Ghi âm / Cắt file / Transcript "Sắp có"), `keep-alive` giữ state khi đổi tab. Bám sát layout desktop.
- **views/**: `LoginView` (Google sign-in, nền dark), `RecordView` (Mode A — ghi), `SplitView` (Mode B — cắt file, 1 card).
- **components/**: `BrandMark` (logo SVG sóng âm→dòng transcript), `AppTopBar` (header), `QuotaBar` (dung lượng/quota), `RecordingHistory` (lịch sử ghi + nút Nghe lại/Xoá), `MediaPlayer` (`<video>/<audio>` phát qua `/api/files/:id`).
- **composables/**: `use-auth` (Firebase → session cookie), `use-upload` (chunked upload resumable), `use-splitter` (job cắt + poll), `use-recorder` (getUserMedia/getDisplayMedia + Web Audio mix + MediaRecorder timeslice 1s + upload nền + best-effort finalize), `use-recordings` (list + xoá recording), `use-usage` (quota, shared store).
- Dev proxy `/api` → `127.0.0.1:8787`; prod nginx cùng origin.

## Web app — Backend (`server/onescribe-be/`)

Rust/axum → **1 static musl binary**, bind `127.0.0.1:8787`, secret nhúng compile-time (`build.rs`/`option_env!`), session key HS256 sinh runtime, TLS out-bound = rustls (musl sạch).

**Modules** (`src/`):
- `main.rs` — bootstrap: connect Mongo, tạo index, reset job kẹt, khởi `jobs::start_worker` + `retention::start_sweeper`, serve.
- `config.rs` — hằng: `RETENTION_DAYS=14`, `QUOTA_BYTES=5GB`, `MAX_FILE_SIZE_BYTES=2GB`, Firebase project, allowlist.
- `auth.rs` — Firebase ID token → verify (JWKS securetoken + email_verified + allowlist) → session cookie HttpOnly+Secure+SameSite=Lax; audit-log; middleware `require_auth`.
- `jwks.rs`, `session.rs` — verify token + issue/verify session (jsonwebtoken).
- `upload.rs` — chunked upload (init/chunk/status/finalize) append-disk, quota tăng dần, magic-byte (`infer`).
- `recordings.rs` — **Mode A streaming**: init/chunk/finalize open-ended cho MediaRecorder (mirror upload, coll `recording_sessions` riêng) → `files{kind:recording}`. Finalize **remux `-c copy -f webm`** để dựng Cues+Duration (webm MediaRecorder streaming thiếu → không tua được); remux lỗi → fallback rename giữ container gốc.
- `files.rs` — `GET /api/files` (lịch sử ghi) · `GET /api/files/:id` (stream + **HTTP Range** cho player tua) · `DELETE /api/files/:id` (xoá thủ công, ownership) · `GET /api/me/usage` (quota).
- `jobs.rs` + `worker/split_worker.rs` — job queue cắt (claim atomic, concurrency=1). Mode B (upload webm): disk-free ≥2× → remux `-c copy -f matroska` → segment → dọn `fixed.mkv` → đoạn `kind:output` (tải zip). **Mode A (`recordingSplit`)**: input recording đã seekable → segment thẳng webm→webm + probe mỗi đoạn → đoạn `kind:recording` (vào Lịch sử ghi, phát/tua riêng) + **xoá bản ghi gốc**.
- `rate_limit.rs` — limiter in-memory theo real IP (`X-Real-IP` nginx-set): auth 20/60s, protected 300/60s (miễn `/chunk`).
- `retention.rs` — sweep 1h: xoá file quá hạn 14 ngày (**disk TRƯỚC → doc**, KHÔNG TTL native), dọn orphan `.part`/session/job cũ, panic-isolation.
- `models/{file,job,user}.rs`, `state.rs`, `error.rs`, `db.rs`, `routes.rs`.

**core-split** (`crates/core-split/`, chia sẻ desktop): `split_file` (ffmpeg segment `-c copy`), `remux_copy_to_mkv` (fix cues webm), `probe_file`/`size_cut_times`, `compute_segment_time`.

### API endpoints (web)

| Method | Route | Ghi chú |
|--------|-------|---------|
| GET | `/api/health` | liveness (không auth) |
| POST | `/api/auth/session` · `/api/auth/logout` | Firebase → cookie (rate-limit 20/60s) |
| GET | `/api/me` · `/api/me/usage` | user + quota |
| POST | `/api/upload/{init,chunk,status,finalize}` | cắt file (upload) |
| POST | `/api/recordings/{init,chunk,finalize}` | ghi (streaming) |
| GET/DELETE | `/api/files/:id` | phát (Range) / xoá |
| GET | `/api/files` | lịch sử ghi |
| POST/GET | `/api/jobs` · `/api/jobs/:id` · `/api/jobs/:id/download` | job cắt + tải zip |

### MongoDB (`onescribe`)

`users` (firebaseUid unique) · `files` (kind `upload|output|recording`, `size`, `durationS`, `hasVideo`, `jobId`, `expiresAt` index thường) · `jobs` (queue cắt) · `upload_sessions` / `recording_sessions` (append-disk).

### Storage (`{data_dir}/{userId}/`)

`tmp/*.part` (đang upload/ghi) · `uploads/` · `recordings/` · `outputs/{jobId}/` + `{jobId}.zip`.

---

## Deploy (web)

Build musl qua Docker (`scripts/build-musl.sh`, WSL) → binary `/opt/onescribe-be/onescribe-be` (systemd) + FE dist `/var/www/onescribe/dist`; nginx `:80` (CF DNS proxied lo TLS) tại `scribe.inetdev.io.vn`. Chi tiết: [deployment-guide.md](./deployment-guide.md). Thiết kế cắt/ghi: [split-record-design.md](./split-record-design.md).

## Tính năng mới (session 2026-07-12)

- **Refactor UI web bám 100% desktop**: theme dark One Series + BrandMark + Be Vietnam Pro + shell 3 tab (thay giao diện trắng/xanh dương cũ).
- **Mode A — Ghi (web)**: mic + màn hình/system audio (Chrome/Edge) → MediaRecorder streaming upload → finalize remux webm seekable (tua được) → lịch sử + player (stream/Range); tùy chọn cắt theo phút → đoạn thành các bản ghi riêng trong Lịch sử ghi (thay bản ghi gốc), phát/tua từng đoạn.
- **Retention & hardening**: auto-purge 14 ngày (sweep), rate-limit real-IP, `/api/me/usage` + QuotaBar, quota 5GB + size cap 2GB.
- **Xoá recording thủ công**: `DELETE /api/files/:id` + nút Xoá (xác nhận inline).

## Unresolved / follow-up

- Web chưa có Transcript (tab "Sắp có") — chờ hạ tầng GPU (trait `Worker`/`TranscribeWorker` stub sẵn).
- Chưa có unit test cho `use-recorder` / `RateLimiter` (logic pure, mock được).
- QA thủ công ghi mic/màn hình thật (getUserMedia/getDisplayMedia không automate được).
