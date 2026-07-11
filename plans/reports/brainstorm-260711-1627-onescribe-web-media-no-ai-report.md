# Brainstorm Report — OneScribe Web (media ghi+cắt, BỎ AI)

**Date:** 2026-07-11 · **Skill:** /brainstorm · **Status:** APPROVED (user duyệt; BE chốt **Rust/axum** sau tư vấn /ask — repo AI-built 100%)
**Target deploy:** box .65 (103.72.98.65 — 4 vCPU Broadwell, ~7.5GB RAM, ~35GB disk trống, ~15 service, **MongoDB sẵn**, **nginx** reverse proxy; **Cloudflare chỉ là DNS proxied** trỏ endpoint về nginx trên .65 — KHÔNG tunnel/Access)

## 1. Problem statement

Web hóa OneScribe để giải nỗi đau **phân phối cross-platform** (desktop phải build/ký/cài riêng Win/macOS, iPad bất khả thi). **BỎ hoàn toàn phần AI transcribe (Mode C)** vì box server không GPU, cấu hình yếu — bài học từ pivot cũ (`260707-onescribe-web-pivot`) **đã CANCELLED** do CPU Broadwell không kham nổi ASR realtime. Web app lần này = **frontend nền tảng cho tương lai**: làm ghi+cắt trước, chừa chỗ cắm transcribe khi có hạ tầng GPU.

## 2. Hiện trạng codebase (scout)

- Desktop Tauri v2 (Rust) + Vue 3 + Vuetify + ViUI. 3 mode: Ghi (A) / Cắt (B) / Transcript (C).
- **Mode B** `src-tauri/src/splitter.rs`: cắt theo **phút / MB / số phần**, `ffmpeg -f segment -c copy` (stream-copy lossless, KHÔNG re-encode → nhẹ CPU, không GPU). MB cắt tại keyframe theo byte thực tế. HEVC ép tag `hvc1`.
- **Mode A** `src/composables/use-recorder.ts`: mic (cpal), tiếng hệ thống (WASAPI loopback, Win-only), ghi màn hình (ddagrab/libopenh264), cắt live theo phút.
- **Mode C** `src-tauri/src/transcribe.rs` + sidecar Python PhoWhisper/faster-whisper: **phần bỏ**.
- UI tái dùng: `src/views/RecordView.vue` + `SplitView.vue` + `design-system/vi-ui/` (bỏ `TranscriptView.vue`). Repo này CHỈ có desktop — backend web là code mới; `apps/web`/`meeting-server` của plan cũ ở repo khác, không tái dùng trực tiếp.
- **BE viết bằng Rust** → **DRY thật**: tách logic cắt trong `splitter.rs` thành **crate chung `core-split`** (Cargo workspace) dùng cho cả desktop (Tauri) lẫn web BE (axum). Chỉ cần gỡ phần Tauri (`AppHandle`/`Emitter`/tauri shell) khỏi thuật toán thuần (`compute_segment_time`, `size_cut_times`, cấu trúc lệnh ffmpeg segment), thay bằng `tokio::process` + callback tiến độ.

## 3. Quyết định đã chốt (nguồn chân lý — user duyệt 2026-07-11)

| # | Quyết định | Ghi chú |
|---|---|---|
| 1 | Định vị **frontend nền tảng cho tương lai** — ghi+cắt trước, chừa chỗ transcribe | Kiến trúc pluggable worker |
| 2 | Phạm vi: **Ghi mic + Ghi màn hình + tiếng hệ thống + Cắt file + Xuất/tải** | Bỏ Mode C |
| 3 | Cắt file: **server-side ffmpeg `-c copy`** | Stream-copy, không GPU |
| 4 | Deploy: **box .65** + **FE Vue3/Vite + BE Rust (axum) + MongoDB** (đã có trên .65) | Expose qua **nginx vhost**; **CF chỉ DNS proxied** (không tunnel) |
| 5 | Recording: **upload lưu server + lịch sử** | Sẵn cho pipeline transcribe sau |
| 6 | Auth: **Google Sign-In (OAuth2)**, **mọi tài khoản Google** | Multi-user theo email; allowlist optional |
| 7 | Ghi: **có cắt live theo phút** → hiện thực bằng **ghi liền → server tự cắt** | Không mất frame, tái dùng SplitWorker |
| 8 | AI tương lai: **khung job/queue + storage sẵn** (TranscribeWorker stub) | Cắm engine = thêm 1 class |
| 9 | Retention: **N ngày auto-purge + quota disk/user** | Bắt buộc (disk hạn chế) |
| 10 | **Tách 2 phần FE/BE riêng biệt** | FE = static bundle; BE = **1 static binary Rust** + systemd, localhost-only |
| 11 | **FE**: Vite build → static `dist`; **nginx serve** trực tiếp | nginx trỏ về thư mục deploy FE |
| 12 | **BE**: **`cargo build --release --target x86_64-unknown-linux-musl`** (build ở local) → **1 static binary self-contained**; env nhúng **compile-time** (`env!`/`build.rs`) | Server **KHÔNG có env**, **KHÔNG có `node_modules`/runtime**; **systemd** start/giám sát binary |
| 13 | **BE chỉ chạy `127.0.0.1`** (không public); nginx **listen :80** block **`/api` proxy_pass** tới BE | Chỉ nginx là bề mặt public; **CF proxied lo TLS edge → không cần cert/443 origin** |
| 14 | **BE = Rust/axum** (không Node) | Repo AI-built 100% → bỏ yếu tố "team quen ngôn ngữ"; ràng buộc self-contained hợp Rust; compiler = lớp an toàn cho code AI viết; DRY crate `core-split` với desktop |
| 15 | **musl static build dùng `rustls`** (tránh OpenSSL native) | TLS out-bound **Google JWKS/OAuth (bắt buộc HTTPS)** qua rustls; native-tls/OpenSSL (C lib) phá static musl → cấm. MongoDB local `127.0.0.1` plaintext không cần TLS |

## 4. Approaches đã cân

- **A (CHỐT): FE static tách khỏi BE monolith Rust/axum + MongoDB + ffmpeg, storage filesystem.** FE Vue3/Vite build static (nginx serve). BE 1 binary Rust (axum) sau nginx `/api` (localhost, systemd): Google OAuth2 → session/JWT, REST (`/api/auth /api/upload /api/jobs /api/files /api/me`), job queue trên MongoDB collection `jobs` (worker poll + `find_one_and_update` atomic claim, concurrency=1), **trait `Worker`** (`SplitWorker`=ffmpeg qua crate `core-split`, `TranscribeWorker`=stub), storage `/data/{userId}/`, cron purge+quota. Crates: `axum` + `tokio` (web/async), `mongodb` (official), `oauth2` + `jsonwebtoken` + `jsonwebtoken`/JWKS (verify Google id_token), `tower-http` (static/limits), `serde`. Pros: 1 static binary self-contained khớp ràng buộc no-env/no-node_modules **miễn phí**; compiler bắt lỗi (an toàn cho code AI viết); **DRY `core-split` với desktop**; tận dụng MongoDB sẵn; cắm AI sau = thêm 1 impl `Worker`. Cons: compile chậm hơn (AI iterate lâu hơn — chấp nhận, `incremental=true` + dự án nhỏ); OAuth2 phải lắp tay hơn Node.
- **B: BE Node + esbuild bundle.** Iterate nhanh hơn (no compile), ecosystem web nhỉnh; nhưng ràng buộc no-env/no-node_modules phải hack (`--define` nhúng secret, external-hoá optional native của mongodb, test bundle). Repo AI-built → mất lợi thế "người quen JS". Loại (ràng buộc self-contained hợp Rust hơn).
- **C: FE + FastAPI Python riêng.** Boundary cho AI sau (cùng Python) nhưng +1 HTTP surface, auth trùng, chưa có AI → thừa. Loại (YAGNI).
- **D: Client-side ffmpeg.wasm (zero server).** Nặng RAM browser, file lớn đuối, không hợp "upload lưu server + lịch sử". Loại (mâu thuẫn Decision #5).

## 5. Giải pháp cuối (Approach A) — kiến trúc

```
Browser (Vue3+Vite+ViUI)     CF DNS proxied    Box .65
─ Ghi: getUserMedia/          ──TLS edge──▶     nginx listen :80 (bề mặt public DUY NHẤT; CF lo TLS)
    getDisplayMedia                              ├─ location /      → serve FE static (dist)
   → MediaRecorder → blob                        └─ location /api   → proxy_pass 127.0.0.1:PORT
─ Cắt: chọn file + tiêu chí                                              │
─ Lịch sử + player nghe/xem lại                    ┌────────────────────┘
─ Progress qua SSE/poll                            ▼
                                          BE Rust/axum (systemd, 127.0.0.1 only — KHÔNG public)
                                          ─ Auth: Google OAuth2 → session/JWT
                                          ─ REST: /api/auth /upload /jobs /files /me
                                          ─ Job queue (MongoDB coll `jobs`, conc=1, nice 10)
                                             ├─ SplitWorker      = ffmpeg -f segment -c copy (core-split)
                                             └─ TranscribeWorker = STUB (cắm sau)
                                          ─ MongoDB: DB riêng `onescribe`
                                          ─ Storage FS: /data/{userId}/{recordings,outputs}
                                          ─ Cron: purge > N ngày + enforce quota

Build/deploy:
  FE → Vite build → dist/ (static) → copy tới thư mục nginx serve
  BE → cargo build --release --target x86_64-unknown-linux-musl (BUILD Ở LOCAL)
       → 1 static binary → scp lên .65 → systemd (systemctl enable --now)
       env: nhúng COMPILE-TIME (env!/build.rs) → server KHÔNG có env
       server BE chỉ có: binary + systemd unit + ffmpeg — KHÔNG node_modules, KHÔNG runtime
```

**Data model (MongoDB — DB riêng `onescribe`):** `users{_id, googleSub, email, createdAt}` · `files{_id, userId, kind[recording|upload], path, size, durationS, createdAt, expiresAt}` (TTL index trên `expiresAt`) · `jobs{_id, userId, type[split|transcribe], inputFileId, params, status[queued|running|done|error], progress, result, createdAt}`. Claim job atomic bằng `find_one_and_update({status:queued},{status:running})` (mongodb crate).

**Worker (Rust trait):** `async fn process(&self, job) -> Result<JobResult>` — `SplitWorker` gọi crate **`core-split`** (thuật toán từ `splitter.rs`: phút/MB/số phần, keyframe MB-cut, HEVC hvc1) qua `tokio::process` spawn ffmpeg. `TranscribeWorker` = stub trả "chưa hỗ trợ" (khi có GPU thay bằng spawn sidecar Python). Thêm engine sau = thêm 1 `impl Worker`, không đụng queue/API.

## 6. Ma trận parity desktop → web

| Tính năng | Web | Ghi chú |
|---|---|---|
| Cắt file (Mode B) | ✅ 100% | ffmpeg server-side, giữ nguyên logic |
| Ghi mic | ✅ 100% | getUserMedia + MediaRecorder |
| Ghi màn hình | ✅ ~90% | getDisplayMedia → webm (browser encode) |
| Tiếng hệ thống | ⚠️ Chrome/Edge only | share-tab/system audio; browser khác disclosure |
| Cắt live theo phút | ✅ (khác cách) | ghi liền → server cắt (không mất frame) |
| Transcript (Mode C) | 🔌 stub | khung sẵn, engine cắm sau |

## 7. Điểm kỹ thuật cần lưu ý

1. **Cắt live** → ghi liền 1 file rồi server ffmpeg cắt theo phút (đã chốt cách A). "Cắt" xong ngay sau khi ghi, tái dùng SplitWorker.
2. **File ghi màn hình = `.webm` (VP8/VP9)**, không phải mp4. ffmpeg `-c copy` cắt webm cần TEST (segment muxer kén hơn mp4; fallback matroska). Mic `.webm/opus` → convert `.mp3` tốn CPU nhẹ (chấp nhận).
3. **Google OAuth**: cần Google Cloud project + OAuth client + consent screen. HTTPS: **CF proxied cấp TLS ở edge** cho client (getUserMedia + OAuth redirect thỏa secure-context); **origin nginx chỉ listen :80** (CF↔origin HTTP, KHÔNG cần cert). Redirect URI OAuth = `https://domain/...` (CF edge). BE (Rust) verify `id_token` bằng `jsonwebtoken` + **Google JWKS** (fetch qua rustls) — thủ công hơn `google-auth-library` của Node nhưng straightforward. "Mọi tài khoản Google" → **nên thêm allowlist/domain option** để hãm lạm dụng.
4. **Băng thông upload = nút cổ chai**: video màn hình vài trăm MB qua nginx/CF → cần chunked/resumable upload + size cap + progress UI.
5. **MongoDB dùng chung ~15 service**: bắt buộc **DB riêng `onescribe` + user/role riêng** (không đụng data service khác); index `expiresAt` (TTL) + `userId`.
6. **cargo build BE — 1 static binary, server KHÔNG có `node_modules`/runtime**: `cargo build --release --target x86_64-unknown-linux-musl` → 1 binary self-contained (musl static link) → server chỉ có binary + systemd unit + ffmpeg binary. Rust tự native → **không có vấn đề "native binding không bundle được"** (khác hẳn Node/esbuild). **musl static: dùng `rustls`** cho TLS out-bound (Google JWKS/OAuth token bắt buộc HTTPS) để **tránh link OpenSSL/native-tls (C lib)** phá static — `reqwest` feature `rustls-tls` (MongoDB local plaintext không cần TLS; nếu Mongo TLS thì `mongodb` cũng `rustls-tls`). **Env nhúng compile-time** qua `env!("MONGO_URI")` (đặt biến lúc `cargo build` ở local) hoặc `build.rs`/`option_env!` (OAuth secret, JWT key, v.v.) → binary tự đủ, server không cần env. ffmpeg là external binary (spawn qua `tokio::process`), không link vào. **Hệ quả vận hành:** đổi bất kỳ env nào (rotate secret, đổi Mongo URI/domain OAuth) = **rebuild + redeploy BE**. Binary chứa secret (`strings` moi được) → **đặt ngoài web root** (chỉ FE `dist` nằm trong thư mục nginx serve). Build ở local (cần cross-target musl: `rustup target add x86_64-unknown-linux-musl`; nếu build trên Windows → dùng WSL/Docker cho musl Linux).

## 8. Rủi ro & mitigation

| # | Rủi ro | Mitigation |
|---|---|---|
| 1 | **Disk 35GB + video nặng + mọi tài khoản Google** → đầy disk/abuse | Retention N ngày + quota/user + size cap + rate-limit + allowlist optional (BẮT BUỘC) |
| 2 | CPU chia sẻ ~15 service | ffmpeg `nice 10` + queue concurrency=1 + CPUQuota systemd |
| 3 | webm `-c copy` segment lỗi | TEST sớm ở Phase 3; fallback matroska/remux |
| 4 | Upload file lớn fail giữa chừng | resumable upload (tus/chunk) + retry |
| 5 | Lạm dụng OAuth mở | allowlist email/domain + quota + audit log |
| 6 | MongoDB chung với service khác | DB riêng `onescribe` + user/role riêng, không đụng collection khác |
| 7 | nginx vhost đụng service đang chạy | vhost server_name riêng cho domain OneScribe; không sửa vhost khác |
| 8 | **Secret nhúng trong binary** (compile-time env: OAuth secret, Mongo URI+password, JWT key) → `strings ./binary` moi ra được | **TUYỆT ĐỐI không commit binary vào git**; chmod 600 trên .65; hạn chế quyền đọc file/máy build; rotate secret khi nghi lộ; build ở local tin cậy. **Rust KHÔNG an toàn hơn Node ở điểm này** — trade-off của "server sạch, không env" |
| 9 | **CORS/cookie khi FE gọi `/api`** | FE + `/api` cùng origin (nginx) → same-site cookie, không cần CORS; token/cookie httpOnly + SameSite |
| 10 | **musl static build vướng OpenSSL native** → link fail / cần lib trên server | Dùng **`rustls`** cho mọi TLS (mongodb/reqwest feature `rustls-tls`); verify binary chạy trên .65 KHÔNG cần lib ngoài — **gate Phase 1** |
| 11 | **Compile Rust chậm → AI iterate lâu** | `incremental=true`, workspace tách crate (`core-split` build 1 lần), dev chạy `cargo run` (glibc) nhanh, chỉ build musl khi deploy |

**Lưu ý so với pivot cũ:** ffmpeg stream-copy RTF >100× (cắt 60' xong trong giây) → KHÔNG đè box như ASR đã làm sập pivot cũ. Rủi ro chính chuyển từ CPU → **disk + băng thông**.

## 9. Success metrics / Acceptance criteria (MVP)

- [ ] Login Google → vào app; user chỉ thấy file của mình.
- [ ] Upload file 60' → cắt theo phút/MB/số phần → tải zip kết quả; box không đầy disk/degrade.
- [ ] Ghi mic trên web → upload → hiện trong lịch sử → nghe lại; cắt live theo phút cho ra nhiều đoạn.
- [ ] Ghi màn hình + system-audio OK trên Chrome/Edge; browser khác disclosure "không hỗ trợ".
- [ ] File quá hạn N ngày bị purge (kiểm disk); vượt quota bị chặn upload.
- [ ] Auth chặn anonymous; ownership `/files/:id` `/jobs/:id`; upload size cap + magic-byte + rate-limit; không secret trong repo.
- [ ] TranscribeWorker là stub rõ ràng; cắm engine sau không phải sửa queue/storage/API.

## 10. Build order sơ bộ (4 phase)

| # | Phase | Nội dung | Gate |
|---|---|---|---|
| 1 | Nền server + auth + deploy | **Cargo workspace** (`core-split` + `onescribe-be` axum) tách khỏi FE (Vite); BE **cargo musl static binary** (env compile-time, `rustls`) + **systemd** (127.0.0.1); MongoDB (DB `onescribe`); Google OAuth2 + verify id_token (JWKS) + session/JWT; storage FS; **nginx vhost listen :80, 2 block** (`/` static, `/api` proxy localhost); CF DNS proxied (lo TLS edge) | **Binary chạy trên .65 KHÔNG cần lib/env/node_modules**; Login → /api/me OK; FE static served; BE không public |
| 2 | Cắt file (Mode B) | upload resumable + SplitWorker ffmpeg + job queue + tải zip | E2E upload→cắt→tải |
| 3 | Ghi (Mode A) | getUserMedia/getDisplayMedia + MediaRecorder + ghi liền→server cắt + lịch sử + player | Ghi→upload→cắt→nghe lại |
| 4 | Retention + hardening | cron purge + quota + rate-limit + size cap + magic-byte + allowlist option | Purge+quota hoạt động |

## 11. Next steps & dependencies

1. `/ck:plan` từ report này (plan dir mới).
2. Cần user cung cấp: Google Cloud OAuth client (hoặc tạo mới), domain web (`scribe.inetdev.io.vn`?) trỏ CF DNS proxied → nginx .65, **MongoDB connection string + DB/user riêng** cho `onescribe`, quyết định allowlist/domain + N ngày retention + quota GB/user.
3. Test webm `-c copy` sớm (Phase 3 gate) + verify **musl static binary chạy trên .65 không cần lib** (Phase 1 gate).
4. Chuẩn bị nginx vhost (**listen :80**, server_name domain OneScribe, `proxy_pass` tới BE `127.0.0.1:PORT`) — CF proxied lo TLS, không cần cert; không đụng vhost service khác.
5. Setup Cargo workspace: tách thuật toán cắt trong `src-tauri/src/splitter.rs` thành crate `core-split` (gỡ Tauri) dùng chung desktop + web BE.

## Unresolved questions

1. Domain chính thức cho web (`scribe.inetdev.io.vn`?).
2. N ngày retention cụ thể + quota GB/user (đề xuất: 14 ngày, 5GB/user — chờ user chốt).
3. Allowlist email/domain cho OAuth "mọi tài khoản Google" (có giới hạn không, hay audit-only?).
4. Số phận desktop app: maintenance hay tiếp tục song song?
