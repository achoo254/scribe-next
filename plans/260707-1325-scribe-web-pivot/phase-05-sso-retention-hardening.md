---
phase: 5
title: "SSO Retention Hardening"
status: pending
effort: "M"
priority: P2
dependencies: [2]
---

# Phase 5: SSO Retention Hardening

## Overview

Đóng vòng production-ready: cắm SSO tập trung (khi có contract), retention purge tự động, metrics vào VictoriaMetrics sẵn có trên box, siết bảo mật bề mặt public. Chạy được song song sau Phase 2 (không cần chờ 3/4), riêng mục SSO chờ dependency ngoài.

## Requirements

- Functional: login qua SSO tập trung (fallback JWT nội bộ nếu contract chưa có); transcript tự purge theo retention; dashboard metrics cơ bản.
- Non-functional: không secret trong repo; mọi endpoint public có rate-limit; disk không phình vô hạn.

## Architecture

- Auth: nâng từ **basic-auth edge single-tenant (MVP)** lên **SSO tập trung multi-user** — `auth-sso.mjs` verify token/session SSO (endpoint contract user cung cấp); `currentUser()` (hook Phase 2) trả user thật; **thêm cột `owner_user` + ownership check `/api/jobs/:id`** (RT-7 kích hoạt khi multi-user). env `AUTH_PROVIDER=edge-basic|sso`.
- Retention: timer trong server (setInterval + SQLite `expires_at`) — KHÔNG cần cron hệ thống; purge transcripts hết hạn + uploads mồ côi >24h + job records cũ.
- Metrics: endpoint `/metrics` (text Prometheus format, hand-rolled — đếm jobs, queue depth, stream active, RTF gần nhất, purge count) → thêm scrape target vào vmagent trên box (`/etc/…vmagent` config — sửa khi deploy).

## Related Code Files

- Create: `server/scribe-server/{auth-sso.mjs,metrics.mjs,retention.mjs}` + tests.
- Modify: `server/scribe-server/{index.mjs,auth.mjs,db.mjs}`; `infra/systemd/scribe-server.service` (env AUTH_PROVIDER); `docs/deployment-guide.md` + `docs/system-architecture.md` (kiến trúc web mới, đánh dấu desktop maintenance); `plans/260626-1418-realtime-meeting-live-transcribe/plan.md` (đã đánh dấu superseded — verify nhất quán).
- Đọc: rate-limit + `TRUST_PROXY` code sẵn có trong meeting.mjs (tái dùng, không viết mới).

## Implementation Steps

1. **[Chờ contract]** Lấy spec auth SSO từ user (endpoint verify? OIDC? cookie domain?). Chưa có → giữ `AUTH_PROVIDER=edge-basic` (basic-auth Cloudflare, single-tenant), mục này dời không chặn các mục dưới. Khi bật sso: thêm `owner_user` + backfill + ownership check.
2. `retention.mjs`: purge loop 1h/lần; theo `expires_at` (env `TRANSCRIPT_RETENTION_DAYS`, mặc định 30). **[RT-11] Purge cả FILE trên disk** (`.txt/.srt` sidecar ghi ra `out_dir` — `main.py:98-104`), KHÔNG chỉ row SQLite → nếu bỏ sót, transcript đầy đủ nằm lại trên box chung sau retention. **`segments_json` không có nguồn** (`done` chỉ trả `text`+`txt_path`+`srt_path`, không segments) → hoặc parse lại từ `.srt`, hoặc patch sidecar `done` carry segments (sidecar sửa nhỏ, cross-ref Decision #8). Uploads mồ côi >24h; VACUUM tuần. Test clock giả.
3. `metrics.mjs`: counters/gauges kể trên; `/metrics` chỉ bind qua auth nội bộ hoặc IP localhost (vmagent cùng box). Thêm scrape config + verify số hiện trong VictoriaMetrics.
4. Hardening sweep: rate-limit login/upload/token/**webhook** routes (tái dùng bucket meeting); header bảo mật qua reverse-proxy (HSTS, nosniff); audit env/secret (không hardcode; `.env` ngoài repo, mẫu `.env.example`; **ASR worker env tối thiểu** — RT-5); kiểm upload magic-byte + size + tên server-generate + **ownership `/api/jobs/:id`** (RT-7, verify lại từ Phase 2).
5. Chạy `/ck:security-scan` (hoặc review-security skill) trên `server/scribe-server` + FE — fix findings Critical/High.
6. Load-test nhẹ: 5 user upload đồng thời + 2 phiên live → box ổn; ghi số vào deployment-guide (egress/CPU/RAM thực đo).
7. Docs: cập nhật system-architecture (sơ đồ web pivot), deployment-guide (runbook .65: deploy, rollback, logs, metrics), README (định vị mới: web-first, desktop maintenance).

## Success Criteria

- [ ] `AUTH_PROVIDER=sso` hoạt động với contract thật (kèm ownership multi-user) HOẶC ghi rõ "chờ contract, edge-basic single-tenant đang chạy" trong report phase.
- [ ] Transcript quá hạn bị xóa đúng lịch **gồm file `.txt/.srt` trên disk** (RT-11), không chỉ SQLite; disk usage ổn định qua 1 tuần.
- [ ] Metrics hiện trong VictoriaMetrics trên box (screenshot/query kèm report).
- [ ] Security scan: 0 finding Critical/High chưa xử lý (đính kèm số lượng theo severity).
- [ ] Docs cập nhật: system-architecture + deployment-guide + README khớp thực tế deploy.

## Risk Assessment

- Contract SSO không bao giờ đến → internal JWT là trạng thái chấp nhận được vô hạn (đã pluggable); KHÔNG reverse-engineer bundle JS staging của SSO.
- Purge xóa nhầm transcript đang xem → purge chỉ theo `expires_at` tuyệt đối, không theo "last access"; UI hiện ngày hết hạn.
- Sửa config vmagent trên box đụng hệ giám sát chung → backup config trước, thêm 1 scrape job riêng biệt, không sửa job sẵn có.
