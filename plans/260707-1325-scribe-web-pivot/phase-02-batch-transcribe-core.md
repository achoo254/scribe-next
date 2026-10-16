---
phase: 2
title: "Batch Transcribe Core"
status: pending
effort: "M"
priority: P1
dependencies: [1]
---

# Phase 2: Batch Transcribe Core

## Overview

Nền web app: server Node (mở rộng meeting-server) + upload/ghi mic → job queue → PhoWhisper-medium batch → transcript; FE Vue cơ bản; auth JWT nội bộ pluggable; deploy staging .65 với systemd guardrails. Đây là core value của MVP.

## Requirements

- Functional: upload audio/video (webm/mp3/mp4/wav/m4a) hoặc ghi mic (MediaRecorder) → job → transcript xem/tải trên web; job list + status polling. **Auth ở edge (basic-auth), không login trong app.**
- Non-functional: queue concurrency=1, medium cố định, `nice -n 10`, model unload sau idle; audio XÓA ngay khi transcript xong (ephemeral); cây process bị chặn bởi systemd CPUQuota/MemoryMax.

## Architecture

```
Cloudflare ($DEPLOY_DOMAIN, TLS + basic-auth edge) → cloudflared tunnel → localhost
Browser (apps/web + vue-router)
  ├─ POST /api/uploads (multipart)                   ┐ scribe-server (Node ESM,
  ├─ GET  /api/jobs, /api/jobs/:id (+transcript)     │ raw http — GIỮ pattern zero-framework
  └─ (meeting routes GIỮ NGUYÊN từ meeting-server)   ┘ của server/meeting-server/index.mjs)
                     │ spawn per-job, ĐÓNG stdin sau done → process thoát → nhả RAM
                     ▼
      python3 sidecar/transcriber/main.py  ← protocol JSON stdio batch ĐÃ CÓ: cmd `transcribe`,
      (medium, threads=2 qua env)             events `progress`/`done`/`error` (main.py:7-8)
```

- **[RT-9] `main.py` KHÔNG tự thoát sau `done`** — loop `for line in sys.stdin` (main.py:114) chờ lệnh kế. "Spawn tự nhả RAM" chỉ đúng nếu runner **đóng stdin hoặc gửi `shutdown` rồi kill** sau `done`. Runner phải làm việc này tường minh.
- **[RT-1] threads:** truyền `SCRIBE_CPU_THREADS=2` (cần patch main.py:65-68 — sidecar sửa nhỏ) hoặc `taskset`; KHÔNG dựa `os.cpu_count()`.
- Job store: SQLite **WAL** (`node:sqlite` nếu Node ≥22.5, fallback `better-sqlite3` — kiểm `node --version` trên box ở bước 7) — jobs(id, file, status, created), transcripts(job_id, text, srt_path, expires_at). **Không `owner_user` ở MVP** (single-tenant; thêm khi SSO Phase 5). **[RT-9] Boot reconciliation:** lúc khởi động, mọi row `status='running'` → mark `failed` (orphan sau restart/OOM).
- Cấu trúc server: tách module trong `server/scribe-server/`: `index.mjs` (compose + http), `meeting.mjs` (move handler 236 dòng từ meeting-server — **GIỮ export `createApp(cfg, deps)` contract** để 10 test cũ chạy nguyên, xem RT-6), `auth.mjs` (chỉ hook `currentUser()` trả tenant tĩnh — chỗ cắm SSO Phase 5), `uploads.mjs`, `jobs.mjs` (queue + spawn runner), `db.mjs`.
- FE: `apps/web` thêm vue-router + views: `TranscribeJobsPage.vue` (upload + list + detail), `RecordMicPage.vue` (MediaRecorder → upload lúc dừng). **Không `LoginPage.vue`** (auth ở edge). Style: plain Vue + `theme.css` token (như hiện trạng; design system npm vẫn defer).
- Decode audio: PyAV (`av==17`) decode webm/opus trực tiếp — KHÔNG cần ffmpeg CLI. **[RT-5] Đây là bề mặt RCE (decode input không tin cậy):** xem Security bên dưới.

## Auth & Security (validate 2026-07-07: basic-auth edge, single-tenant)

<!-- Updated: Validation Session 1 — auth = basic-auth edge, single-tenant; ownership defer Phase 5 -->
- **Auth = basic-auth tầng edge** (Cloudflare Access / cloudflared), KHÔNG JWT login app-level cho MVP. App coi mọi request đã-qua-edge là **1 tenant chung** → **bỏ `auth.mjs` JWT + bỏ `owner_user`/ownership check** ở Phase 2 (chỉ 1 identity → RT-7/RT-10 moot). Interface auth vẫn để 1 hook `currentUser()` trả tenant tĩnh — Phase 5 thay bằng SSO + multi-user + ownership. **Meeting (Phase 4) giữ token LiveKit riêng** (basic-auth edge không thay token SFU).
- **RT-5 (GIỮ — auth edge KHÔNG cứu decode) — cô lập ASR worker:** runner spawn python với **env tối thiểu (chỉ model path + threads)**, KHÔNG kế thừa `LK_API_SECRET`/secret qua `EnvironmentFile`. Systemd hardening: `NoNewPrivileges=yes`, `PrivateTmp=yes`, `ProtectSystem=strict`, `ProtectHome=yes`, `ReadWritePaths=/opt/scribe`, drop caps, `MemoryMax`/`MemorySwapMax`/`CPUQuota`.
- **RT-5 — validate upload:** MIME allowlist + **magic-byte sniff** + size cap; tên file server-generate; đường lưu canonicalize (chặn traversal).
- **Rate-limit:** sau Cloudflare → XFF là IP thật của CF edge; **trust Cloudflare IP ranges** rồi lấy client IP từ `CF-Connecting-IP`/XFF (không `TRUST_PROXY=false` như bản LAN — nếu không mọi request chung 1 bucket).

## Related Code Files

- Create: `server/scribe-server/{index.mjs,meeting.mjs,auth.mjs,uploads.mjs,jobs.mjs,db.mjs}` + tests (**runner custom zero-dep `test/run-tests.mjs` — KHÔNG `node:test`**, RT-15); `apps/web/src/{router.ts,pages/TranscribeJobsPage.vue,pages/RecordMicPage.vue}` (không LoginPage — auth ở edge); `infra/systemd/scribe-server.service` (CPUQuota=200%, MemoryMax + MemorySwapMax **chốt sau Phase 1**, hardening RT-5, env tối thiểu cho worker); `infra/cloudflared/config.yml` (tunnel `$DEPLOY_DOMAIN` → localhost).
- **Expose = Cloudflare tunnel (validate 2026-07-07, Decision #9):** `$DEPLOY_DOMAIN` proxied qua Cloudflare (TLS + basic-auth ở edge), `cloudflared` route tới scribe-server + web static ở localhost. **KHÔNG thêm vhost :443 nginx/Caddy** trên box (không đụng LiveKit wss đang chạy). Caddyfile repo để lại cho môi trường khác, không dùng cho .65.
- **[RT-6] Modify (git mv phá — ENUMERATE đủ consumer):** `git mv server/meeting-server server/scribe-server` phá: `server/dev-token/generate-token.mjs:6` (import `../meeting-server/lib/jwt.mjs`), `infra/run-local.sh:27` (`node server/meeting-server/index.mjs`), `infra/systemd/meeting-server.service:11` (WorkingDirectory) — **cả 3 phải sửa + decommission unit `meeting-server.service` cũ** (nếu không: 2 unit cùng port 8787 conflict). `packages/meeting-shared/src/meeting-client.ts` gọi endpoints (giữ path). `apps/web/src/App.vue,main.ts` (router mount).
- **[RT-6] Rollback + state:** caption store + host-lock **in-memory → mất mỗi restart/deploy** (đã biết, Redis defer); tag checkpoint trước `git mv`, có bước rollback (revert + restart unit cũ) tới khi Phase 5 ổn.
- Delete: `infra/systemd/meeting-server.service` (thay bằng scribe-server.service) — sau khi verify unit mới chạy.

## Implementation Steps

1. `git mv server/meeting-server server/scribe-server`; tách handler (236 dòng) thành `meeting.mjs` **giữ export `createApp(cfg, deps)`**; `index.mjs` compose router path-prefix; sửa 3 consumer (RT-6). Chạy lại 10 test cũ (runner `test/run-tests.mjs`) — PHẢI pass nguyên (regression gate).
2. `db.mjs` + `jobs.mjs`: SQLite WAL + boot-reconcile orphan (RT-9); queue FIFO concurrency=1 + **arbiter: chờ nếu có phiên live/meeting đang mở** (RT-2, cross-ref Phase 3 stream-registry); runner spawn `main.py` (env model path + `SCRIBE_CPU_THREADS=2`, nice 10); parse events **`progress`/`done`/`error`** (main.py:7-8); `done` → lưu transcript, **unlink audio ngay** + **đóng stdin/`shutdown` rồi kill process** để nhả RAM (RT-9).
3. `auth.mjs`: chỉ hook `currentUser()` trả tenant tĩnh (auth thật ở edge basic-auth); rate-limit lấy client IP từ `CF-Connecting-IP` (trust Cloudflare). KHÔNG JWT login app-level.
4. `uploads.mjs`: multipart streaming về `/opt/scribe/uploads/` (KHÔNG buffer RAM), size cap env (mặc định 500MB), **magic-byte sniff** + MIME allowlist, tên file server-generate + canonicalize (chặn path traversal — bài học M-fix plan 260626).
5. FE: router + 2 page; upload có progress; jobs poll 3s; transcript render + copy/download .txt/.srt (srt formatter đã có trong sidecar `fmt_ts`).
6. Tests: unit queue/uploads/boot-reconcile (runner custom `test/run-tests.mjs` zero-dep — RT-15, KHÔNG node:test); integration 1 job end-to-end với fake-model python (pattern test sidecar sẵn có).
7. Deploy staging: rsync repo → `/opt/scribe/app`; `npm ci --omit=dev`; pip deps như Phase 1 (kiểm `node --version` cho SQLite backend); systemd unit scribe-server enable; **`cloudflared` tunnel `$DEPLOY_DOMAIN` → localhost + basic-auth ở edge (Cloudflare Access/tunnel)**; smoke test qua HTTPS.
8. Đo sau deploy: upload file 60' thật → xác nhận acceptance (RAM/load box, transcript OK, audio bị xóa).

## Success Criteria

- [ ] 10 test meeting cũ pass nguyên sau restructure (0 regression); 3 consumer (generate-token/run-local/unit) đã sửa; unit cũ decommission.
- [ ] E2E: upload file 60' tiếng Việt qua web → transcript medium; audio file biến mất khỏi disk sau khi xong; python process thoát (RAM nhả).
- [ ] Ghi mic trên browser → dừng → upload → transcript.
- [ ] Queue: 2 job cùng lúc → tuần tự; job 2 "queued". Restart server giữa job → orphan `running` thành `failed` (boot-reconcile).
- [ ] systemd: `systemctl show` xác nhận CPUQuota/MemoryMax/MemorySwapMax + hardening (NoNewPrivileges/PrivateTmp); worker env KHÔNG chứa secret; kill -9 python giữa job → job fail sạch, server sống.
- [ ] Upload sai magic-byte bị từ chối (RT-5); worker env KHÔNG chứa secret (RT-5).
- [ ] Request không qua edge basic-auth → bị chặn ở Cloudflare; rate-limit tính đúng IP thật qua `CF-Connecting-IP` (không gộp 1 bucket). *(Ownership/multi-user = Phase 5 khi có SSO.)*
- [ ] `vue-tsc` FE + `node test/run-tests.mjs` server + `py_compile` sidecar: 0 error (báo số cụ thể).

## Risk Assessment

- Restructure meeting-server phá deploy staging meeting đang chạy → làm trên nhánh module hóa, giữ nguyên route path + env contract; systemd unit cũ trỏ binary mới; verify bằng test + smoke.
- Node version trên box thấp → bước 7 kiểm `node --version`; <20 thì cài Node 22 LTS qua nvm/fnm hoặc tarball (ghi vào deployment-guide).
- Upload lớn đè disk → size cap + purge uploads mồ côi (cron Phase 5, tạm TTL 24h ở Phase 2).
- SQLite lock khi WS Phase 3 thêm ghi → WAL mode ngay từ đầu.
