---
title: "OneScribe Web — media ghi+cắt (BE Rust/axum, bỏ AI)"
description: "Web hóa OneScribe làm ghi+cắt media (bỏ AI transcribe); BE Rust/axum → 1 static musl binary env-compile-time, FE Vue3/Vite+ViUI, MongoDB, deploy box .65 qua nginx (:80, CF proxied lo TLS) + systemd; frontend nền tảng chừa chỗ cắm transcribe sau"
status: pending
priority: P2
branch: "main"
tags: [web, rust, axum, vue, mongodb, ffmpeg, oauth, no-ai, self-contained-musl]
blockedBy: []
blocks: []
created: "2026-07-11T09:59:16.886Z"
createdBy: "ck:plan"
source: skill
---

# OneScribe Web — media ghi+cắt (BE Rust/axum, bỏ AI)

## Overview

Web hóa OneScribe: **ghi + cắt media, BỎ hoàn toàn AI transcribe** (box .65 không GPU). Định vị = **frontend nền tảng cho tương lai** — chừa chỗ cắm transcribe (worker stub) khi có hạ tầng GPU. Nguồn chân lý: [brainstorm report](../../reports/brainstorm-260711-1627-onescribe-web-media-no-ai-report.md).

**Kiến trúc:** FE Vue3/Vite+ViUI (static, nginx serve) · BE **Rust/axum → 1 static musl binary** (env nhúng compile-time, localhost-only, **systemd**) · MongoDB (DB riêng `onescribe`) · ffmpeg server-side `-c copy` · CF chỉ DNS proxied (lo TLS ở edge) → **nginx listen :80** `/` (FE) + `/api` (proxy 127.0.0.1 BE). Repo AI-built 100% → ưu tiên correctness + compiler safety.

## Quyết định đã chốt (nguồn chân lý — KHÔNG tự đảo)

| # | Quyết định |
|---|---|
| 1 | BE **Rust/axum**, KHÔNG Node; FE Vue3/Vite+ViUI tách static |
| 2 | BE = **1 static musl binary**; env nhúng **compile-time** (`env!`/build.rs); server KHÔNG env/node_modules/runtime; TLS out-bound (Google JWKS/OAuth) = **rustls** (tránh OpenSSL để static musl sạch) |
| 3 | **MongoDB** (đã có .65, DB riêng `onescribe`), KHÔNG SQLite; job queue coll `jobs` + `find_one_and_update` atomic claim |
| 4 | Deploy: **CF chỉ DNS proxied → nginx listen :80** (CF lo TLS edge, không tunnel, không cert origin); BE localhost + nginx `/api` proxy; **systemd** quản lý binary |
| 5 | Cắt file: server-side **ffmpeg `-c copy`**; logic tách crate **`core-split`** (Cargo workspace, share desktop `splitter.rs`) |
| 6 | Ghi (mic/màn hình/system audio) → **upload lưu server + lịch sử**; cắt live = **ghi liền → server cắt** (không mất frame) |
| 7 | Auth **Google OAuth2** (mọi tài khoản Google; allowlist/domain option chống lạm dụng); multi-user theo email |
| 8 | AI tương lai: **khung job/queue + storage sẵn** + trait `Worker` (`TranscribeWorker` stub) |
| 9 | Retention **N ngày auto-purge + quota disk/user** (đề xuất 14 ngày, 5GB/user — chờ chốt cuối) |

## Phases

| Phase | Name | Status |
|-------|------|--------|
| 1 | [Foundation & deploy gate](./phase-01-foundation-deploy-gate.md) — workspace + core-split + axum skeleton + **musl static binary chạy trên .65** (GATE) + FE scaffold | Pending |
| 2 | [Auth Google OAuth](./phase-02-auth-google-oauth.md) — OAuth2 + verify id_token (JWKS) + session/JWT + MongoDB users + FE login/guard | Pending |
| 3 | [Mode B — Cắt file](./phase-03-mode-b-c-t-file.md) — upload resumable + job queue + SplitWorker (core-split) + tải zip + FE Cắt | Pending |
| 4 | [Mode A — Ghi](./phase-04-mode-a-ghi.md) — getUserMedia/getDisplayMedia + MediaRecorder + upload + ghi liền→server cắt + lịch sử + player | Pending |
| 5 | [Retention & hardening](./phase-05-retention-hardening.md) — cron purge + quota + rate-limit + size cap + magic-byte + allowlist + security pass | Pending |

## Thứ tự & phụ thuộc

- **Phase 1 = GATE de-risk:** chứng minh musl static binary (rustls, không lib ngoài) chạy sạch trên .65 sau nginx/systemd TRƯỚC khi xây tính năng. NO-GO → xử lý toolchain trước.
- Phase 2 → 3 → 4 tuần tự (3/4 cần auth+storage của 2; 4 tái dùng SplitWorker của 3).
- Phase 5 chốt cuối (retention/hardening áp lên mọi phase trước).
- **Gate kỹ thuật Phase 3:** test ffmpeg cắt webm `-c copy` (MediaRecorder webm hay thiếu cue/duration) — xác định lệnh tin cậy trước khi làm Phase 4 ghi.

## Dependencies

- **Liên quan (không block):** `plans/260707-onescribe-web-pivot` (CANCELLED — pivot cũ server-side ASR, plan này thay hướng bỏ AI). `plans/260613-1130-split-record-implementation` (desktop maintenance — Phase 1 sẽ **extract `core-split` từ `src-tauri/src/splitter.rs`**; desktop sẽ dùng crate chung, cần regression test giữ hành vi Mode B desktop).
- **User cung cấp trước implement:** domain web + CF DNS proxied, Google Cloud OAuth client (id/secret/redirect URI), MongoDB URI + DB/user `onescribe`, chốt N ngày retention + quota GB/user + allowlist.

## Research

- [researcher-rust-be-stack-report.md](./research/researcher-rust-be-stack-report.md) — axum/mongodb-rustls/oauth/musl/systemd/nginx
- [researcher-web-capture-ffmpeg-report.md](./research/researcher-web-capture-ffmpeg-report.md) — MediaRecorder/getDisplayMedia/ffmpeg webm split/upload

## Gates & câu hỏi chờ (CHƯA chạy — làm sau khi có giờ)

**Trạng thái:** plan + 5 phase + research = ✅ xong. **Red-team = ⏸ CHƯA chạy** (dừng giữa chừng theo yêu cầu). **Validate = ⏸ CHƯA chạy**. Chạy `/ck:plan red-team` + `/ck:plan validate` với plan này trước khi `/ck:cook`.

**Câu hỏi cần chốt ở validate (business/external — chưa quyết):**
1. Domain web (`scribe.inetdev.io.vn`?) + CF DNS proxied → nginx :80 .65.
2. Google Cloud OAuth client (id/secret/redirect URI) + có bật allowlist domain (`@inet.vn`?) hay mở hẳn.
3. MongoDB URI + tạo DB/user riêng `onescribe` trên .65.
4. N ngày retention + quota GB/user (mặc định đề xuất 14 ngày, 5GB/user).
5. **[finding research]** Cắt recording webm: chấp nhận **remux+copy** (sai số mốc vài giây, nhẹ CPU) hay cần **re-encode** (đúng chính xác nhưng 2-3× CPU trên box không GPU)? → xác nhận ở gate test Phase 4.

**Finding đã đưa vào plan (không cần hỏi lại, chỉ verify khi cook):** MediaRecorder webm thiếu cues → `-c copy` không cắt đúng mốc (Phase 4 gate); chunked upload phải append disk không buffer RAM (Phase 3); systemd thay PM2; nginx listen :80 (CF lo TLS).
