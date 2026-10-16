# Codebase Summary — Scribe

Tổng quan kiến trúc + module chính. Cập nhật 2026-10-16 (bỏ app desktop; đăng nhập chuyển sang hoandat-auth; prod ở `scribe.hoandat.com`; giao diện web theo khung Localman; gói "Sau cuộc họp": biên bản, dấu mốc, gửi sang chat).

Scribe = app ghi/cắt/transcribe media trong hệ sinh thái hoandat. Repo có **2 ứng dụng**:

| App | Vị trí | Stack | Vai trò |
|-----|--------|-------|---------|
| **Web** | `web/` + `server/scribe-be/` + `crates/core-split/` | React 19 + Vite 7 + Tailwind 4 (token từ localman-next) · BE Rust/axum (musl static) · MongoDB · đăng nhập hoandat-auth (OIDC) | Ghi (browser) + cắt file + **chép lời bằng OpenAI** (key của server, đo phút theo user, quota tháng + trần chi phí; chỉ email trong `TRANSCRIBE_EMAILS` tới khi bật `TRANSCRIBE_OPEN`) |
| **iPhone** | `web/ios/` (Capacitor 8, đóng gói bản build của `web/`) | Swift plugin + cùng mã React | Ghi nền khi khoá màn hình theo đoạn 1 phút, lưu bền, tự tải lên; dùng chung backend web. Chi tiết: [ios-app.md](./ios-app.md) |


---

## Web app — Frontend (`web/`)

React 19 + Vite 7 + Tailwind 4 + TanStack Query + Radix. Token màu lấy từ localman-next (`styles/tokens.css`, kiểm tương phản bằng `pnpm run check:tokens`).

- **Router** (`routes.tsx`, `createBrowserRouter`): `/` → `/record`, `/record`, `/split`, `/transcripts`, `/transcripts/:key` (khoá phiên qua `transcriptPath()` ở `layout/app-paths.ts`), `/login`, `/__ui` (chỉ dev). Hai trang `/transcripts*` chỉ có khi `me.canTranscribe`. Trạng thái ghi (`pages/record/record-state.tsx`) và upload/job Cắt file (`pages/split/split-state.tsx`) nằm trong `layout/signed-in-state.tsx`, dưới `RequireAuth`: đổi trang không mất phiên ghi, đăng xuất thì unmount. `PageError` là `errorElement` bên dưới shell, trang lỗi không làm mất trạng thái đó. Trang không lazy, để tab đang mở qua một lần deploy không phải tải chunk đã mất.
- **Shell** (`layout/`, chép từ Localman, ghi nguồn ở đầu tệp): top bar sticky (wordmark, nav theo URL, account menu); dưới 640 px nav thành một hàng gọn ngay dưới thanh. Account menu có link IdP `/account` và `/account#account-apps` (`lib/idp.ts`), cùng nút "Đăng xuất"; khi đang ghi hoặc đang tải lên thì hỏi xác nhận trước (`account-controls.tsx`, `session-activity.ts`). Account menu còn có mục liên kết / gỡ liên kết chat.hoandat.com (hộp hướng dẫn `components/chat-link-dialog.tsx`; người chưa liên kết không thấy khi máy chủ báo `enabled: false`). Footer có link IdP và theme toggle. Còn có skip link, `page-header.tsx` dùng chung, và `leave-guard.tsx` hỏi trước khi rời trang đọc còn câu chưa lưu.
- **pages/**: `login-page`, `record/` (nguồn ghi, theo dõi mức âm, thẻ ghi native trên iPhone, Lịch sử ghi gom theo ngày và theo phiên iPhone, panel "Chép lời", hộp xác nhận bản ghi dài), `split/` (cắt file), `transcript/` (trang "Lời đã chép": danh sách bản chép lời theo phiên + màn hình đọc: chạm câu để phát, tự sang đoạn kế, sửa câu theo đoạn, đổi tên, chép lại từng đoạn, xuất, xoá; khối "Biên bản" `minutes-card.tsx` với nút "Tóm tắt" và "Gửi sang chat", danh sách dấu mốc `bookmark-list.tsx`). Nút "Đánh dấu" khi đang ghi (trình duyệt và thẻ ghi iPhone) ở `record/bookmark-control.tsx`. URL và tên trong code vẫn là `transcript`.
- **i18n/** (chép từ Localman): mọi chuỗi giao diện nằm trong từ điển tiếng Việt `i18n/messages/vi/`; component dùng `useT()`, module không phải React dùng `activeTranslator()`. `dictionaries.test.ts` bắt khoá thừa/thiếu, `hard-coded-strings.test.ts` báo lỗi khi `src/` còn chuỗi tiếng Việt viết cứng ngoài từ điển.
- **features/** (logic, không UI): `files` (danh sách/xoá/gom nhóm), `upload` (chunk), `split` (job cắt), `recorder` (trình duyệt + bản native qua plugin Swift), `transcript` (API chép lời, ghép phiên), `transcript-docs` (API `/api/transcript-docs`, query có poll `/status` nhẹ, `reader-model` thuần, hàng đợi gửi dấu mốc `bookmark-queue.ts` thử lại nền, không chặn việc ghi), `chat-link` (API `/api/chat/link`; sau khi mở deep link thì đọc lại trạng thái tới khi người dùng xác nhận trong chat).
- **auth/**: `auth-api.ts`, `auth-context.tsx` — web chuyển hẳn sang `/api/auth/login` (BE lo OIDC), iPhone gọi plugin Swift `HoandatAuth`; đăng xuất web đi tiếp tới trang kết thúc phiên của IdP.
- **lib/**: `api.ts` (`apiFetch`, `isNativeShell`, origin backend khi chạy trong app), `download.ts` (tải/phát qua plugin native trên iPhone), `native.ts` (khai báo plugin Swift), `idp.ts` (origin hoandat-auth, đọc `VITE_IDP_ORIGIN`, mặc định `https://auth.hoandat.com`), `external-link.ts` (mở link ngoài; trong app iPhone, Capacitor chuyển sang trình duyệt hệ thống).
- **e2e/** (Playwright, `pnpm run test:e2e`): stub API (`support/api-stubs.ts`) và stub Capacitor (`support/capacitor-stub.ts`) nên không cần backend; chạy trên Chromium desktop và WebKit iPhone (nhánh iOS qua stub), so ảnh chụp sáng/tối, kiểm axe. `pnpm run build` kết thúc bằng `check:dist`: build thất bại nếu stub lọt vào `dist/`.
- Dev proxy `/api` → `127.0.0.1:8787`; prod nginx cùng origin.

## Web app — Backend (`server/scribe-be/`)

Rust/axum → **1 static musl binary**, bind `127.0.0.1:8787`, cấu hình nhúng compile-time (`build.rs`/`option_env!`), bí mật (session key, secret OIDC, key OpenAI, token bot) đọc từ tệp lúc khởi động, TLS out-bound = rustls (musl sạch).

**Modules** (`src/`):
- `main.rs` — bootstrap (hoặc nhánh CLI `admin`): connect Mongo, tạo index, reset job kẹt, khởi `jobs::start_worker` + `retention::start_sweeper` + task long-poll `chat_link::poll` (chỉ khi bật `CHAT_LINK_POLL`), serve.
- `config.rs` — hằng: `RETENTION_DAYS=14`, `QUOTA_BYTES=5GB`, `MAX_FILE_SIZE_BYTES=2GB`; biến build (bắt buộc / mặc định), allowlist, hạn mức chép lời.
- `auth.rs` — đăng nhập hoandat-auth. Web: BFF, client confidential `scribe-web`, Code + PKCE phía server (`state`/`nonce`/verifier ở `auth_states`), back-channel logout. iPhone: client public `scribe-mobile`, app gửi code + verifier + nonce tới `/api/auth/native/exchange`, nhận token thiết bị để `/api/auth/native/refresh`. Email chưa xác minh / ngoài allowlist / tài khoản bị khoá → từ chối; user cũ thời Firebase ghép theo email. Cookie phiên `__Host-scribe` (https) HttpOnly+SameSite=Lax; middleware `require_auth` (cache phiên 30 giây).
- `oidc.rs` — discovery, JWKS, xác minh ID token và logout token, đổi code, refresh; email/tên lấy từ `userinfo` (ID token chỉ có `sub`), `sub` hai nguồn phải khớp.
- `session.rs` — session key (tự sinh), token ngẫu nhiên, băm token, mã hoá refresh token IdP. `models/session.rs` — `sessions` (web 12 giờ trượt tới 7 ngày, mobile 12 giờ), `devices`, `auth_states`, `logout_jtis`.
- `upload.rs` — chunked upload (init/chunk/status/finalize) append-disk, quota tăng dần, magic-byte (`infer`).
- `transcribe/` — chép lời bằng OpenAI `gpt-4o-mini-transcribe`: `POST/GET /api/transcripts`, `GET /api/transcripts/export` (txt/srt, ghép cả phiên iPhone); hàng đợi riêng trong collection `transcripts`; cắt mảnh ≤45 giây tại khoảng lặng (ffmpeg `silencedetect`), nén Opus, gọi song song 4 mảnh; mảnh hoàn toàn im lặng (đỉnh < −30 dB theo `volumedetect` lúc nén) không gửi, không tính phút. Key OpenAI của server đọc từ tệp lúc khởi động (`Transcriber`, có cờ ngắt khi key hỏng). Tiền: `POST /api/transcripts` giữ chỗ phút trong `models/usage.rs` (user → quỹ free → trần tổng, `$inc` có điều kiện, không cần transaction) rồi xếp hàng; worker cộng `sentS` trước mỗi lần gửi audio; lượt kết thúc (xong, lỗi, xoá) đóng sổ đúng một lần theo `runId` và quyết toán `sentS − chargedS`, ghi `transcribe_runs` để đối soát. Restart: lượt dở chạy lại, cắt ngang lần hai thì dừng. Gói free còn giới hạn ngày mỗi tài khoản và thời gian chờ của tài khoản mới (không giới hạn theo IP). `admin.rs` — CLI `scribe-be admin set-plan|usage|block|unblock|notify-test` (`block` gỡ luôn liên kết chat; `usage --recompute` cộng cả chi phí tóm tắt từ `summary_runs`). `notify.rs` — báo chủ sản phẩm qua bot chat.hoandat.com. `transcribe/docs.rs` — transcript theo phiên ghi (`/api/transcript-docs`, xem bảng API): gom đoạn bằng `file::session_key` (`s:<sessionId>` cho phiên iPhone, `f:<fileId>` cho bản ghi một file), ghép kết quả máy với tiêu đề + câu sửa lúc đọc. Câu sửa gắn `runId` của lượt chép: chép lại thì phần sửa của đoạn đó tự hết hiệu lực; PUT ghi có điều kiện, chen lượt chép lại → 409. `transcribe/bookmarks.rs` — dấu mốc bấm lúc đang ghi: doc tạo lười bằng upsert vì lúc đó phiên có thể chưa có file; quyền sở hữu kiểm theo trạng thái phiên (giải thích ở đầu tệp). `POST /api/recordings/init` trả `fileId` giữ trước để web có khoá `f:<fileId>` ngay khi ghi.
- `summary/` — biên bản (tóm tắt, quyết định, việc cần làm, mỗi mục kèm giây) bằng OpenAI `SUMMARY_MODEL` (mặc định `gpt-4.1-mini`), chỉ khi người dùng bấm "Tóm tắt". Đầu vào là chữ đã áp câu sửa; lưu kèm băm đầu vào nên API báo `stale` khi chữ đổi. Tiền: lượt mỗi ngày theo gói + trần chi phí tháng mỗi tài khoản (sổ `summary_usage`), chi phí token quy ra giây chép lời tương đương và cộng vào trần tổng tháng của chép lời; phần có tiền chạy trong task riêng nên client ngắt kết nối không để sổ treo. Key hỏng chỉ tạm ngưng tóm tắt (cờ riêng), không ngắt chép lời. Lý do và quy tắc hoàn lượt ở đầu `summary/mod.rs`. `summary/share.rs` — gửi biên bản tới chat riêng đã liên kết (không bao giờ vào nhóm).
- `chat_link/` — liên kết tài khoản với chat riêng trên chat.hoandat.com, không thêm endpoint public: deep link `/start <token>` → bot hỏi xác nhận (email che bớt) → bấm "Xác nhận liên kết" mới gắn (`flow.rs`, lý do chống liên kết hộ ở đầu tệp); bot nhận `/unlink`. `poll.rs` — task long-poll `GET {NOTIFY_BOT_API}/updates`, offset lưu Mongo. **Chỉ một backend được poll:** dev và prod dùng chung token bot, mỗi bot chỉ có một poll (poll mới làm poll cũ nhận 409), nên `CHAT_LINK_POLL` mặc định tắt và chỉ bật trên prod; tắt thì `POST /api/chat/link` trả 503 `CHAT_LINK_DISABLED`. `bot.rs` — client Bot API.
- `access_log.rs`, `audit.rs` — log mọi request + collection `audit_events` (TTL 180 ngày) cho hành vi người dùng.
- `recordings.rs` — **Mode A streaming**: init/chunk/finalize open-ended cho MediaRecorder (mirror upload, coll `recording_sessions` riêng) → `files{kind:recording}`. Finalize **remux `-c copy -f webm`** để dựng Cues+Duration (webm MediaRecorder streaming thiếu → không tua được); remux lỗi → fallback rename giữ container gốc.
- `files.rs` — `GET /api/files` (lịch sử ghi) · `GET /api/files/:id` (stream + **HTTP Range** cho player tua) · `DELETE /api/files/:id` (xoá thủ công, ownership) · `GET /api/me/usage` (quota).
- `jobs.rs` + `worker/split_worker.rs` — job queue cắt (claim atomic, concurrency=1). Mode B (upload webm): disk-free ≥2× → remux `-c copy -f matroska` → segment → dọn `fixed.mkv` → đoạn `kind:output` (tải zip). **Mode A (`recordingSplit`)**: input recording đã seekable → segment thẳng webm→webm + probe mỗi đoạn → đoạn `kind:recording` (vào Lịch sử ghi, phát/tua riêng) + **xoá bản ghi gốc**.
- `rate_limit.rs` — limiter in-memory theo real IP (`X-Real-IP` nginx-set): auth 120/60s, `/api/auth/native/exchange` 10/60s, protected 300/60s (miễn `/chunk`); ngân sách chung 45 lần/phút gọi token endpoint của IdP.
- `retention.rs` — sweep 1h: xoá file quá hạn 14 ngày (**disk TRƯỚC → doc**, KHÔNG TTL native), dọn orphan `.part`/session/job cũ và `transcript_docs` không còn audio (doc chưa có biên bản được chừa 24 giờ để dấu mốc bấm trước khi có file không bị xoá), panic-isolation. `file::delete_doc(&FileDoc)` là nơi duy nhất xoá doc file: xoá kèm `transcripts` và câu sửa của đoạn.
- `models/{file,job,user,session,plan,usage,transcript,transcript_doc,summary,chat_link}.rs`, `state.rs`, `error.rs`, `db.rs`, `routes.rs`.

**core-split** (`crates/core-split/`): `split_file` (ffmpeg segment `-c copy`), `remux_copy_to_mkv` (fix cues webm), `probe_file`/`size_cut_times`, `compute_segment_time`.

### API endpoints (web)

| Method | Route | Ghi chú |
|--------|-------|---------|
| GET | `/api/health` | liveness (không auth) |
| GET | `/api/auth/login` · `/api/auth/callback` | web: chuyển tới IdP / nhận code, tạo phiên, đặt cookie |
| GET/POST | `/api/auth/native/config` · `/api/auth/native/exchange` · `/api/auth/native/refresh` | iPhone: tham số đăng nhập / đổi code lấy phiên + token thiết bị / phiên mới bằng token thiết bị |
| POST | `/api/auth/logout` · `/api/auth/backchannel-logout` | đăng xuất / IdP báo đăng xuất (logout token có chữ ký) |
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
| POST | `/api/transcript-docs/{key}/summary` | tạo / tạo lại biên bản, trả đồng bộ; 409 `NOT_TRANSCRIBED`, 403 `SUMMARY_DAILY_LIMIT`/`SUMMARY_MONTHLY_LIMIT`/`CAP_REACHED` kèm `retryAt`, 503 khi tạm ngưng. `GET …/{key}` trả kèm `summary` (có `stale`) và `bookmarks` |
| POST/DELETE | `/api/transcript-docs/{key}/bookmarks` · `…/bookmarks/{id}` | thêm dấu mốc `{at, label?}` (tối đa 200 mốc mỗi doc) / xoá |
| POST | `/api/transcript-docs/{key}/share` | gửi biên bản tới chat đã liên kết; 409 `CHAT_NOT_LINKED`, `CHAT_BLOCKED` (bot bị chặn → đã gỡ liên kết) |
| GET/POST/DELETE | `/api/chat/link` | trạng thái `{linked, linkedAt, enabled}` / tạo deep link một lần (hạn 10 phút; 503 `CHAT_LINK_DISABLED`) / gỡ liên kết |

Nhóm `/api/transcript-docs` và `/api/chat/link` có trần body 1 MiB; chỉ cần đăng nhập và đúng chủ dữ liệu.

### MongoDB (database `$DB_NAME`)

`users` (`idpSub` unique; user cũ còn `firebaseUid`; `blocked`, `plan`/`planUntil`; `chat = {chatId, linkedAt}` khi đã liên kết chat) · `sessions` / `devices` / `auth_states` / `logout_jtis` (phiên thu hồi được) · `files` (kind `upload|output|recording`, `size`, `durationS`, `hasVideo`, `jobId`, `expiresAt` index thường) · `jobs` (queue cắt) · `upload_sessions` / `recording_sessions` (append-disk) · `transcripts` (kết quả máy + hàng đợi chép lời, `_id` = id file) · `transcript_docs` (`{userId, key}` unique, tạo đồng bộ lúc khởi động: `title`, `edits.<fileId> = {runId, texts}`, `summary` (biên bản + băm đầu vào), `bookmarks`) · `transcribe_usage` (sổ giây theo tháng giờ VN: `u:<userId>:<YYYY-MM>`, `free:<YYYY-MM>`, `all:<YYYY-MM>`; sổ ngày gói free `d:<userId>:<YYYY-MM-DD>`) · `transcribe_runs` (mỗi lượt đã quyết toán, chỉ thêm) · `transcribe_marks` (mốc quỹ/trần đã báo, đếm lần chạm giới hạn ngày) · `summary_usage` (lượt ngày `<userId>:<YYYY-MM-DD>`, chi phí tháng `cost:<userId>:<YYYY-MM>`) · `summary_runs` (mỗi lượt gọi model: mở trước khi gọi với `reservedS`, đóng khi quyết toán với token và `costS`) · `chat_link_tokens` (băm token deep link, TTL theo `expiresAt`) · `kv` (doc `chat_link.offset`: offset long-poll đã xử lý) · `audit_events` (TTL 180 ngày; sự kiện transcript không chứa tiêu đề hay câu).

### Storage (`{data_dir}/{userId}/`)

`tmp/*.part` (đang upload/ghi) · `uploads/` · `recordings/` · `outputs/{jobId}/` + `{jobId}.zip`.

---

## Deploy (web)

Build musl bằng `cargo zigbuild` → binary `/opt/scribe-be/scribe-be` (systemd `scribe-be`) + FE dist `/var/www/scribe/dist`; nginx `:443` (Cloudflare proxied, SSL Full) tại `scribe.hoandat.com`. Chi tiết: [deploy-web-production.md](./deploy-web-production.md); app iPhone: [ios-app.md](./ios-app.md).

## Tính năng mới (session 2026-07-12)

- **Mode A — Ghi (web)**: mic + màn hình/system audio (Chrome/Edge) → MediaRecorder streaming upload → finalize remux webm seekable (tua được) → lịch sử + player (stream/Range); tùy chọn cắt theo phút → đoạn thành các bản ghi riêng trong Lịch sử ghi (thay bản ghi gốc), phát/tua từng đoạn.
- **Retention & hardening**: auto-purge 14 ngày (sweep), rate-limit real-IP, `/api/me/usage` + QuotaBar, quota 5GB + size cap 2GB.
- **Xoá recording thủ công**: `DELETE /api/files/:id` + nút Xoá (xác nhận inline).

## Unresolved / follow-up

- Transcript giai đoạn 2–3 (tìm kiếm, thư mục/nhãn, chia sẻ, docx) chưa làm.
- Chưa có unit test cho `RateLimiter` (logic pure, mock được).
- App iPhone: còn kiểm cuộc gọi đến, AirPods, chế độ máy bay (xem [ios-app.md](./ios-app.md)).
- QA thủ công ghi mic/màn hình thật (getUserMedia/getDisplayMedia không automate được).
