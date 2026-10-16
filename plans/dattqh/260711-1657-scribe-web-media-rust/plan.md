---
title: "Scribe Web — media ghi+cắt (BE Rust/axum, bỏ AI)"
description: "Web hóa Scribe làm ghi+cắt media (bỏ AI transcribe); BE Rust/axum → 1 static musl binary env-compile-time, FE Vue3/Vite+design system, MongoDB, deploy box .65 qua nginx (:80, CF proxied lo TLS) + systemd; frontend nền tảng chừa chỗ cắm transcribe sau"
status: done
priority: P2
branch: "main"
tags: [web, rust, axum, vue, mongodb, ffmpeg, oauth, no-ai, self-contained-musl]
blockedBy: []
blocks: []
created: "2026-07-11T09:59:16.886Z"
createdBy: "ck:plan"
source: skill
---

# Scribe Web — media ghi+cắt (BE Rust/axum, bỏ AI)

## Overview

Web hóa Scribe: **ghi + cắt media, BỎ hoàn toàn AI transcribe** (box .65 không GPU). Định vị = **frontend nền tảng cho tương lai** — chừa chỗ cắm transcribe (worker stub) khi có hạ tầng GPU. Nguồn chân lý: [brainstorm report](../../reports/brainstorm-260711-1627-scribe-web-media-no-ai-report.md).

**Kiến trúc:** FE Vue3/Vite+design system (static, nginx serve) · BE **Rust/axum → 1 static musl binary** (env nhúng compile-time, localhost-only, **systemd**) · MongoDB (DB riêng `$DB_NAME`) · ffmpeg server-side `-c copy` · CF chỉ DNS proxied (lo TLS ở edge) → **nginx listen :80** `/` (FE) + `/api` (proxy 127.0.0.1 BE). Repo AI-built 100% → ưu tiên correctness + compiler safety.

## Quyết định đã chốt (nguồn chân lý — KHÔNG tự đảo)

| # | Quyết định |
|---|---|
| 1 | BE **Rust/axum**, KHÔNG Node; FE Vue3/Vite+design system tách static |
| 2 | BE = **1 static musl binary**; env nhúng **compile-time** (`env!`/build.rs); server KHÔNG env/node_modules/runtime; TLS out-bound (Google JWKS/OAuth) = **rustls** (tránh OpenSSL để static musl sạch) |
| 3 | **MongoDB** (đã có .65, DB riêng `$DB_NAME`), KHÔNG SQLite; job queue coll `jobs` + `find_one_and_update` atomic claim |
| 4 | Deploy: **CF chỉ DNS proxied → nginx listen :80** (CF lo TLS edge, không tunnel, không cert origin); BE localhost + nginx `/api` proxy; **systemd** quản lý binary |
| 5 | Cắt file: server-side **ffmpeg `-c copy`**; logic tách crate **`core-split`** (Cargo workspace, share desktop `splitter.rs`) |
| 6 | Ghi (mic/màn hình/system audio) → **upload lưu server + lịch sử**; cắt live = **ghi liền → server cắt** (không mất frame) |
| 7 | Auth **Google qua Firebase Authentication** (FE Firebase JS SDK `signInWithPopup` → Rust BE verify Firebase ID token qua `securetoken` JWKS → session cookie httpOnly riêng; MongoDB users; allowlist option); mọi tài khoản Google, multi-user theo email. *(Đổi 2026-07-11 từ OAuth2 native Rust — user chốt Firebase; Firebase tự quản OAuth client → bỏ `oauth2` crate + `GOOGLE_CLIENT_ID/SECRET`. Xem Phase 2.)* |
| 8 | AI tương lai: **khung job/queue + storage sẵn** + trait `Worker` (`TranscribeWorker` stub) |
| 9 | Retention **14 ngày auto-purge + quota 5GB/user + size cap 2GB/file** (đã chốt ở validate 2026-07-11) |

## Phases

| Phase | Name | Status |
|-------|------|--------|
| 1 | [Foundation & deploy gate](./phase-01-foundation-deploy-gate.md) — workspace + core-split + axum skeleton + **musl static binary chạy trên .65** (GATE) + FE scaffold | ✅ **DONE** — GATE passed (`$DEPLOY_DOMAIN/api/health` 200 qua CF). Còn: desktop rewire core-split (slice riêng) |
| 2 | [Auth Google (Firebase)](./phase-02-auth-google-oauth.md) — Firebase Auth `signInWithPopup` + Rust verify Firebase ID token (`securetoken` JWKS) + session cookie httpOnly + MongoDB users + FE login/guard | ✅ **DONE** — login Google thật OK (user upsert Mongo, /api/me trả email, audit-log) |
| 3 | [Mode B — Cắt file](./phase-03-mode-b-c-t-file.md) — upload resumable + job queue + SplitWorker (core-split) + tải zip + FE Cắt | ✅ **DONE** (code+deploy+review-fix) — live; FE design system; **chờ user QA cắt file thật** (success-path) |
| 4 | [Mode A — Ghi](./phase-04-mode-a-ghi.md) — getUserMedia/getDisplayMedia + MediaRecorder + upload + ghi liền→server cắt + lịch sử + player | ✅ **DONE** (code+review-fix+deploy+E2E live) — streaming append + files stream/Range + webm remux+copy verified trên .65; chờ user QA mic/màn hình thật |
| 5 | [Retention & hardening](./phase-05-retention-hardening.md) — cron purge + quota + rate-limit + size cap + magic-byte + allowlist + security pass | ✅ **DONE** (code+review-fix+deploy) — sweep retention 14d (disk→doc, dọn orphan+job cũ) + rate-limit real-IP + /api/me/usage + QuotaBar |

## Thứ tự & phụ thuộc

- **Phase 1 = GATE de-risk:** chứng minh musl static binary (rustls, không lib ngoài) chạy sạch trên .65 sau nginx/systemd TRƯỚC khi xây tính năng. NO-GO → xử lý toolchain trước.
- Phase 2 → 3 → 4 tuần tự (3/4 cần auth+storage của 2; 4 tái dùng SplitWorker của 3).
- Phase 5 chốt cuối (retention/hardening áp lên mọi phase trước).
- **Gate kỹ thuật Phase 4:** test ffmpeg cắt webm `-c copy` (MediaRecorder webm hay thiếu cue/duration) — xác định lệnh tin cậy ở ĐẦU Phase 4 (Phase 3 dùng file upload có cues chuẩn nên KHÔNG dính lỗi này).

## Dependencies

- **Liên quan (không block):** `plans/260707-scribe-web-pivot` (CANCELLED — pivot cũ server-side ASR, plan này thay hướng bỏ AI). `plans/260613-1130-split-record-implementation` (desktop maintenance — Phase 1 sẽ **extract `core-split` từ `src-tauri/src/splitter.rs`**; desktop sẽ dùng crate chung, cần regression test giữ hành vi Mode B desktop).
- **User cung cấp trước implement:** domain web + CF DNS proxied, MongoDB URI + DB/user `$DB_NAME`, chốt N ngày retention + quota GB/user + allowlist. *(Auth: Firebase project `$FIREBASE_PROJECT_ID` ĐÃ provision 2026-07-11 — Google provider bật + authorized domain thêm; KHÔNG cần tự tạo Google Cloud OAuth client.)*

## Research

- [researcher-rust-be-stack-report.md](./research/researcher-rust-be-stack-report.md) — axum/mongodb-rustls/oauth/musl/systemd/nginx
- [researcher-web-capture-ffmpeg-report.md](./research/researcher-web-capture-ffmpeg-report.md) — MediaRecorder/getDisplayMedia/ffmpeg webm split/upload

## Gates & câu hỏi chờ

**Trạng thái:** plan + 5 phase + research = ✅. **Red-team = ✅ xong** (15 finding + 1 bổ sung, tất cả accepted + applied, xem `## Red Team Review`). **Validate = ✅ xong** (5 quyết định chốt, xem `## Validation Log`). **Auth = ✅ chuyển Firebase 2026-07-11** (project `$FIREBASE_PROJECT_ID` provisioned, Google provider bật — xem Phase 2). **Sẵn sàng `/ck:cook`** sau khi user cung cấp prerequisite (Mongo URI, CF DNS record — liệt kê ở Validation Log; OAuth client KHÔNG còn cần do Firebase quản).

### Cook progress — 2026-07-11 (round: foundation + auth code, chưa test live)
- **Tạo:** Cargo workspace (`exclude src-tauri`), `crates/core-split` (extract thuần từ splitter.rs, 9 unit test), `server/scribe-be` (axum: config/state/error/session/jwks/auth/db/routes/models + build.rs), `web/` (Vue3+Vite+Router+Firebase auth: firebase.ts/use-auth/router/LoginView/HomeView).
- **Verify:** `cargo check --workspace` ✅ · `cargo test -p core-split` 9/9 ✅ · `vue-tsc --noEmit` ✅ · `vite build` ✅. Desktop `src-tauri` KHÔNG đụng (0 regression).
- **Code-review (code-reviewer):** 0 Critical, 2 High + 1 Medium — **đã fix hết**: H1 unique index `firebaseUid` + retry dup-key (chống user trùng/lách quota); H2 cooldown refetch JWKS 30s (chống DoS amplification endpoint public); M1 gitignore `.session-key` + `/target`. Report: `reports/from-code-reviewer-to-cook-scribe-web-auth-foundation-review.md`.
- **HOÃN (ngoài scope round):** desktop rewire dùng core-split (cần regression test runtime Mode B); deploy gate musl/.65/nginx/systemd (chờ WSL musl + SSH); live E2E auth (chờ Mongo URI .65 + Firebase login thật). design system FE chưa cài (dùng Vue thuần tạm — quyết định design-system hoãn tới Phase 3/4).
- **Deviation ghi nhận:** (a) red-team #13 đạt bằng `option_env!` fallback thay `default-members` (verify `cargo check` không cần secret OK); (b) ~~FE Vue thuần thay design system~~ → **ĐÃ tích hợp design system 2026-07-11 (tối)**: `@hobui/ui` + `@hobui/tokens` + Vuetify3 cài từ npm public (không cần registry riêng như tưởng), theme màu brand từ MCP design system; `.mcp.json` project trỏ MCP design system (gitignore). Commit `96d8d62` (design system) + `9ed2784` (fix iconset).

**Câu hỏi validate — ĐÃ CHỐT:** (1) domain `$DEPLOY_DOMAIN`; (2) allowlist MỞ mọi tài khoản Google (audit-only); (3) retention 14 ngày; (4) quota 5GB/user + size cap 2GB/file; (5) cắt webm = remux+copy (verify ở gate Phase 4).

**Finding đã đưa vào plan (không cần hỏi lại, chỉ verify khi cook):** MediaRecorder webm thiếu cues → `-c copy` không cắt đúng mốc (Phase 4 gate); chunked upload phải append disk không buffer RAM (Phase 3); systemd thay PM2; nginx listen :80 (CF lo TLS).

## Red Team Review

### Session — 2026-07-11
**Findings:** 15 (15 accepted, 0 rejected) — sau khử trùng lặp từ 30 finding thô của 4 reviewer (Security Adversary, Failure Mode Analyst, Assumption Destroyer, Scope & Complexity Critic).
**Severity breakdown:** 5 Critical, 7 High, 3 Medium.
**Nguyên tắc:** không finding nào đảo quyết định user đã khoá (Rust/axum, musl static, secret ngoài compile-time, MongoDB, CF DNS-only, allowlist, retention/quota). Finding #4 chỉ refine cách sinh JWT signing key, không đụng "server sạch, không env".

| # | Finding | Severity | Disposition | Applied To |
|---|---------|----------|-------------|------------|
| 1 | TTL index Mongo đua với sweep → orphan disk chắc chắn; bỏ TTL native, sweep là nguồn xoá duy nhất | Critical | Accept | Phase 5 |
| 2 | Rate-limit tin `CF-Connecting-IP` nhưng không chặn truy cập thẳng origin → bypass; thêm `set_real_ip_from` + firewall dải CF | Critical | Accept | Phase 1 (nginx/firewall) + Phase 5 |
| 3 | Quota chỉ enforce ở finalize → DoS disk; check tăng dần khi append + giới hạn session mở | Critical | Accept | Phase 3 + Phase 5 |
| 4 | JWT_SECRET nhúng compile-time (HS256) → forge session bất kỳ; tự sinh key runtime `chmod 600` + TTL ngắn | Critical | Accept | Phase 2 |
| 5 | ffmpeg/ffprobe không có bước cài trên .65 → SplitWorker chết runtime | Critical | Accept | Phase 1 + Phase 3 |
| 6 | core-split extract thiếu: `ffprobe_path`, spawn Tauri-sidecar→tokio mất path-resolve + `CREATE_NO_WINDOW`, "0 regression" chỉ verify tay | High | Accept | Phase 1 |
| 7 | Upload protocol chưa xong: resumable persistence "chốt ở cook" + Phase 4 recording không có `totalChunks` | High | Accept | Phase 3 + Phase 4 |
| 8 | `sessionId` chunked-upload do client gửi → IDOR/path-traversal | High | Accept | Phase 3 |
| 9 | Không kiểm `email_verified` từ id_token → allowlist domain bị vòng | High | Accept | Phase 2 |
| 10 | systemd unit thiếu CPUQuota/MemoryMax → ffmpeg chiếm cả box shared | High | Accept | Phase 1 |
| 11 | oauth2 crate v5 typestate rewrite (không "minor") → snippet v4.4 không compile | High | Accept | Phase 2 → **MOOT 2026-07-11** (bỏ `oauth2` crate khi chuyển Firebase Auth) |
| 12 | plan.md gán nhầm gate webm cho Phase 3 (thật ở Phase 4) | High | Accept | plan.md (đã sửa) |
| 13 | Workspace `env!()` làm `cargo check --workspace`/rust-analyzer fail trên máy dev thiếu secret | Medium | Accept | Phase 1 |
| 14 | Không có số size-cap cụ thể xuyên suốt plan | Medium | Accept | Phase 3 |
| 15 | ffmpeg chạy input không tin cậy không sandbox → allowlist BẬT mặc định + timeout + pin version | Medium | Accept | Phase 3 + Phase 5 |

**Observation đã fold vào phase (không phải finding riêng):** Docker image ref không nhất quán (chốt `ghcr.io/rust-cross/rust-musl-cross`); SSE cho job cắt tức thời → dùng poll 1-2s, giữ SSE cho re-encode Phase 4; `trait Worker`/`TranscribeWorker` (quyết định #8) thêm ~10 dòng interface ở Phase 3 để honor decision; remux webm nhân đôi disk tạm → check disk-free 2× + xoá `fixed.mkv`; capture web hẹp hơn desktop (mất region-select + silent loopback) → thêm disclosure parity Phase 4.

**Finding bổ sung (sót khi dedup, phát hiện lại lúc apply — đã áp Phase 3):** Job `running` không hồi phục sau BE crash/`systemd Restart` → kẹt vĩnh viễn. Fix: reset `jobs{running}→queued` khi BE start + `heartbeatAt` + sweep requeue job stale.

### Whole-Plan Consistency Sweep
- Files reread: plan.md, phase-01..05.
- Decision deltas checked: 6 (webm gate Phase 3→4; bỏ TTL index native; quota finalize→tăng dần; SSE→poll; sessionId server-gen/stateless resume; recording streaming endpoint riêng).
- Reconciled stale references: 8 — Phase 5 Impl step 1 (TTL→index thường), step 3 (finalize→tăng dần), Related Code Files (TTL + nginx conf), Risk RT-11 bullet; Phase 3 Requirements + Related Code Files + Impl step 2 & 5 (SSE→poll), Risk in-memory session (đã chốt stateless).
- Research reports giữ nguyên (artifact lịch sử — `expire_after`/SSE trong đó phản ánh thời điểm research, không phải quyết định cuối).
- **Unresolved contradictions: 0** → plan nhất quán, sẵn sàng cho bước validate.

## Validation Log

### Session — 2026-07-11
**Verification pass:** BỎ QUA (đã có `## Red Team Review` với evidence codebase; không còn `[UNVERIFIED]` tag).
**Câu hỏi:** 5 (5 đã chốt). Tất cả là quyết định business/external không suy được từ codebase.

| # | Quyết định | Chốt | Áp vào |
|---|-----------|------|--------|
| 1 | Domain web | **`$DEPLOY_DOMAIN`** (CF DNS proxied → nginx :80 .65) | Phase 1 (nginx/CF), Phase 2 (redirect URI) |
| 2 | Allowlist Google | **MỞ mọi tài khoản Google** (audit-log only, KHÔNG chặn domain) — *user override khuyến nghị red-team #15* | Phase 2, Phase 5 |
| 3 | Retention + quota | **14 ngày / 5GB/user** | Phase 5 |
| 4 | Size cap/file | **2GB/file** (reject từ chunk đầu) | Phase 3, Phase 5 |
| 5 | Cắt recording webm | **remux+copy** (sai số vài giây, nhẹ CPU); verify sai số+CPU thật ở gate Phase 4 | Phase 4 |

**⚠️ Trade-off quyết định #2 (open signup):** user chọn mở mọi tài khoản Google thay vì allowlist. Hệ quả — bề mặt abuse disk + ffmpeg chạy input lạ (red-team #15) TĂNG. Tuyến phòng thủ chuyển hoàn toàn sang: quota 5GB/user + size cap 2GB/file + quota tăng dần khi append + rate-limit theo real-IP (cần firewall CF #2) + ffmpeg timeout/version-pin + audit-log mọi email đăng nhập. Allowlist giữ dạng cấu hình compile-time (mặc định TẮT) để bật nhanh nếu bị abuse (đổi = rebuild).

**Prerequisite user cung cấp trước `/ck:cook` (không phải quyết định — là giá trị/secret):**
1. ~~Google Cloud OAuth client~~ → **KHÔNG cần** (chuyển Firebase Auth 2026-07-11): Firebase project `$FIREBASE_PROJECT_ID` đã provision, tự quản OAuth client, Google provider đã bật. FE chỉ cần Firebase web config (public, lấy qua `firebase_get_sdk_config`).
2. MongoDB URI + tạo DB/user riêng `$DB_NAME` trên .65.
3. Tạo CF DNS record proxied cho `$DEPLOY_DOMAIN` → 103.72.98.65; dải IP Cloudflare cho firewall/`set_real_ip_from`.

### Auth pivot — Firebase (2026-07-11)
- User chốt dùng Firebase làm module Google Auth (thay OAuth2 native Rust — đảo decision #7, user tự quyết, không phải audit drift).
- Provisioned: project `$FIREBASE_PROJECT_ID` (num `600080952715`), Web App `1:600080952715:web:41d846fba6732899d3538e`, Google sign-in provider bật (`firebase deploy --only auth`), authorized domain `$DEPLOY_DOMAIN`.
- Kiến trúc mới: FE `signInWithPopup` → Firebase ID token → BE Rust verify (securetoken JWKS, aud=`$FIREBASE_PROJECT_ID`, iss=securetoken, email_verified) → session cookie httpOnly (HS256 key runtime, red-team #4 giữ). Bỏ `oauth2` crate (#11 moot) + `GOOGLE_CLIENT_ID/SECRET`. Giữ MongoDB users + allowlist + email_verified (#9).
- Trade-off chấp nhận: auth phụ thuộc hosted Firebase/GCP (ngược triết lý self-hosted #2 nhưng chỉ ở tầng auth; BE binary vẫn musl static sạch).

### Whole-Plan Consistency Sweep
- Files reread: plan.md, phase-01..05.
- Decision deltas checked: 5 (domain; allowlist MỞ; retention 14 ngày; quota 5GB + size cap 2GB; webm remux+copy).
- Reconciled stale references: 9 — decision table #9, Gates section, Phase 1 deploy (domain), Phase 2 config step + allowlist risk bullet, Phase 3 #14 size cap, Phase 4 webm bullet, Phase 5 retention/quota/allowlist/risk bullet.
- **Unresolved contradictions: 0** → plan nhất quán sau red-team + validate. Sẵn sàng `/ck:cook` khi có prerequisite.
