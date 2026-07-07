---
title: "OneScribe Web — pivot server-side transcribe (small-live/medium-final)"
description: "Chuyển OneScribe từ desktop offline sang web app trên server staging .65; transcript server-side PhoWhisper (small=live, medium=final, model cố định); tái dùng meeting-server/apps/web/sidecar; meeting web caption bot livekit-rtc"
status: cancelled
priority: P2
branch: "main"
tags: [web-pivot, asr, phowhisper, server-side, livekit, staging-65]
blockedBy: []
blocks: []
cancelledReason: "Phase 1 gate NO-GO (CPU Broadwell .65 không kham realtime streaming) + user quyết giữ desktop offline (2026-07-07)"
created: "2026-07-07T06:48:09.025Z"
createdBy: "ck:plan"
source: skill
---

# OneScribe Web — pivot server-side transcribe (small-live/medium-final)

> ## 🛑 PLAN CANCELLED — 2026-07-07
> **Quyết định user:** hủy pivot web, **giữ mô hình desktop offline**. Không thực thi Phase 2–5.
> **Căn cứ:** Phase 1 GO/NO-GO gate (đo thật trên .65) → **batch GO nhưng live/meeting NO-GO** do CPU Broadwell (không AVX-512/VNNI) không kham realtime streaming (RTF tốt nhất 1.49, `latency_valid: false`); RAM 8GB cũng không đủ deploy sạch. Resize KHÔNG cứu được giới hạn CPU. Tiền đề cốt lõi của pivot (transcribe server-side) sụp → giữ desktop (ASR chạy trên máy user) hợp lý hơn.
> **Bằng chứng:** [reports/phase-01-benchmark-box65-results.md](./reports/phase-01-benchmark-box65-results.md).
> **Hệ quả:** user quyết **bỏ hoàn toàn tính năng họp online** (2026-07-07) → `260626-1418-realtime-meeting-live-transcribe` cũng **CANCELLED**; toàn bộ code meeting + LiveKit trên .65 đã gỡ sạch. OneScribe giữ **desktop offline transcribe**; maintenance theo `260613-1130-split-record-implementation`.
> **Box .65:** artifact benchmark đã dọn sạch (`/opt/onescribe` removed); patch RT-1 `main.py` đã revert (code desktop nguyên trạng).

## Overview

Pivot theo brainstorm ĐÃ DUYỆT: [brainstorm-260707-1325-onescribe-web-pivot-server-side-transcribe-report.md](../reports/brainstorm-260707-1325-onescribe-web-pivot-server-side-transcribe-report.md).

Web app trên server staging .65 (103.72.98.65 — 4 vCPU Broadwell, 7.5GB RAM, 3.2GB available, 60GB disk, ~15 service đang chạy, LiveKit OneScribe staging ĐÃ chạy port 7880). Desktop → maintenance mode. **~70% tái dùng:** engine sidecar Python (vad-cut stream + batch), `server/meeting-server` (token C6/C7/C8, caption store, TURN), `apps/web`, `packages/meeting-shared`, `infra/`.

## Quyết định đã chốt (nguồn chân lý — user duyệt 2026-07-07)

| # | Quyết định |
|---|---|
| 1 | Web THAY THẾ desktop; desktop maintenance (tag release cuối, chỉ fix bug) |
| 2 | Model **cố định không cho chọn**: small = live, medium = final/batch |
| 3 | Loopback web = Chrome/Edge-only (share system audio); Safari/Firefox báo không hỗ trợ |
| 4 | Audio **ephemeral** — xóa ngay sau transcript; transcript có retention |
| 5 | **Auth MVP = basic-auth ở tầng edge** (Cloudflare Access / cloudflared) → **single-tenant** (1 credential chung, mọi transcript chung không gian). OneDash SSO = future (Phase 5, khi multi-user mới thêm ownership). ⇒ RT-7 (IDOR)/RT-10 (empty-password) **moot ở tầng app cho MVP** (chỉ 1 identity) — nhưng RT-5 (upload RCE) vẫn còn, auth edge không cứu decode |
| 9 | **Expose qua Cloudflare tunnel** — web tại `scribe.inetdev.io.vn` (Cloudflare proxied, TLS ở edge). KHÔNG thêm vhost `:443` nginx/Caddy trên box (không đụng LiveKit wss đang chạy). getUserMedia OK (CF cấp HTTPS). XFF từ Cloudflare → trust CF IP ranges cho rate-limit |
| 6 | Meeting GỘP MVP (user override) — phase riêng, gate sau benchmark, trễ thì tách không kéo core |
| 7 | Guardrails BẮT BUỘC: systemd CPUQuota=200% + MemoryMax + **MemorySwapMax** cho cây process; batch queue=1; **arbiter cấm batch chạy khi có phiên live/meeting đang mở (two-pass enqueue chờ)**; live+meeting chung cap ≤2 **ASR track** (đếm ở tầng Node, enforce trước `stream_start`); batch nice 10; **CPU threads truyền tường minh qua env — KHÔNG dựa `os.cpu_count()`** (xem finding RT-1) |
| 8 | **Sidecar CẦN sửa nhỏ** (bác bỏ giả định "không đụng"): thêm điều khiển `cpu_threads` (main.py + stream_server + stream_session), `done` carry segments cho retention, namespace track id per-session. Vẫn giữ 0-regression Mode C bằng test contract |

## Phases

| Phase | Name | Status |
|-------|------|--------|
| 1 | [Benchmark ASR Gate](./phase-01-benchmark-asr-gate.md) — **GO/NO-GO** | ✅ Done — **gate tách đôi** (xem dưới) |
| 2 | [Batch Transcribe Core](./phase-02-batch-transcribe-core.md) | **GO** (chờ user chốt infra) |
| 3 | [Live Caption Recording](./phase-03-live-caption-recording.md) | ⛔ **NO-GO trên .65** (CPU) |
| 4 | [Meeting Web Caption Bot](./phase-04-meeting-web-caption-bot.md) — rủi ro cao nhất | ⛔ **NO-GO trên .65** (phụ thuộc live) |
| 5 | [SSO Retention Hardening](./phase-05-sso-retention-hardening.md) | Pending |

## ⚠️ GATE RESULT — Phase 1 đo thật trên .65 (2026-07-07)

Chi tiết: [reports/phase-01-benchmark-box65-results.md](./reports/phase-01-benchmark-box65-results.md).

- **Batch (Phase 2): GO** — RTF medium 0.485–0.542, small 0.222 (dưới xa gate 3.0). `batch_size=1` giảm peak RAM 4.27→2.19GB, RTF chỉ +12% (đổi `main.py:95` `batch_size=8`→`1`).
- **Live (Phase 3) + Meeting (Phase 4): NO-GO trên .65** — stream small **không config nào đạt RTF<1** (tốt nhất `rtf_wall 1.49`, `latency_valid: false`). Broadwell (không AVX-512/VNNI) không kham realtime streaming; encoder Whisper 30s/utterance. **KHÔNG phải lỗi RAM/swap** (đối chứng clean-swap: RTF 4.71, CPU 166%). Resize KHÔNG cứu (là giới hạn CPU).
- **RAM 8GB không đủ deploy sạch** — production dùng **6.76GB resident, chỉ 920MB free** (con số "avail 2.9GB" ban đầu bị 2GB swap che). Batch medium bs=1 (2.19GB) ép swap; `MemorySwapMax=0` bất khả thi trên 8GB. RT-2 tệ hơn dự kiến (peak 4.27GB @bs=8 vs giả định 1.74GB).
- **⇒ STOP thực thi Phase 2+ — chờ user quyết scope/infra** (batch-only .65 vs box CPU mới cho full scope; resize vs chấp nhận swap).

## Thứ tự & phụ thuộc

- **Phase 1 = GATE:** đo RTF/RAM thật trên chính box .65. NO-GO (medium batch RTF >~3 @2 threads) → dừng, xem lại (resize VM / box khác / hạ scope).
- Phase 2 → 3 → 4 tuần tự (3 cần WS+stream nền của 2; 4 tái dùng đường live của 3 — bot = "1 client WS nội bộ").
- Phase 5 chốt cuối; riêng mục SSO OneDash phụ thuộc contract ngoài (không chặn — interim JWT chạy từ Phase 2).
- Phase 4 trễ → tách ra sau MVP, KHÔNG kéo 2/3 xuống theo (điều kiện user đã chấp nhận khi gộp meeting vào MVP).

## Dependencies

- **Blocks `260626-1418-realtime-meeting-live-transcribe`:** pivot đảo Decision #2/#4 của plan đó (host-desktop-transcribe → server-transcribe; server không lưu media → audio ephemeral). Phase 3 (host relay) + Phase 4 (web client theo mô hình host) của plan cũ = **legacy, không cook tiếp**; QA checklist Windows của nó chỉ còn giá trị cho desktop maintenance. Engine sidecar + meeting-server + apps/web của plan cũ được plan này tái dùng trực tiếp.
- Plan `260613-1130-split-record-implementation` (desktop packaging, 28 checkbox dở): desktop maintenance → phần packaging cross-platform còn lại KHÔNG làm tiếp trừ khi có yêu cầu riêng.

## Ngân sách tài nguyên .65 (số ĐO THẬT từ spike, không phải ước lạc quan — finding RT-2)

| Thành phần | RAM (spike đo) | Khi nào |
|---|---|---|
| Node onescribe-server | ~150MB | thường trú |
| Batch medium int8 | **~1.74GB** | khi có job (load-on-demand, kill process sau done) |
| Stream small int8 | **~940–1031MB** | khi có phiên live/meeting |
| **Peak batch+live đồng thời** | **~2.9–3.2GB** > `MemoryMax` dự kiến | **cgroup OOM-kill giữa job** → arbiter (Decision #7) cấm chạy đồng thời là BẮT BUỘC |

> Nguồn: `plans/260626-1418-realtime-meeting-live-transcribe/reports/spike-streaming-asr-results.md` (stream 940–1031MB, medium ≈1.74GB). Peak thật > 2.5GB → **`MemoryMax=2500M` sẽ OOM**. Fix: (a) arbiter cấm batch+live đồng thời, (b) `MemoryMax` đặt sau khi Phase 1 đo lại + `MemorySwapMax=0` (fail nhanh, không chết chậm qua swap 876MB đã dùng), (c) **resize VM 12–16GB = gần bắt buộc, không còn "nice-to-have"**. `/proc/meminfo` guard KHÔNG thấy giới hạn cgroup → phải đọc `memory.current`/`memory.max` của cgroup.

## Acceptance criteria (MVP)

- [ ] Upload file 60' tiếng Việt → transcript medium hoàn tất; box không OOM; service khác không degrade (load/RAM đo trước–sau).
- [ ] Ghi mic trên web: caption small ≤4s sau dứt câu; bấm dừng → medium final tự chạy thay bản draft.
- [ ] Ghi màn hình + system-audio hoạt động trên Chrome/Edge; browser khác hiện thông báo không hỗ trợ.
- [ ] Meeting ≤8 người trên web + caption đúng người nói (bot small, ≤2 stream, active-speaker gating).
- [ ] Audio bị XÓA sau khi ra transcript (kiểm disk); transcript purge đúng retention — **gồm cả file `.txt/.srt` sidecar ghi ra disk, không chỉ SQLite** (finding RT-11).
- [ ] Auth chặn anonymous; **ownership check `/api/jobs/:id`** (không đọc chéo user — finding RT-7); upload có rate-limit + size cap + magic-byte check; không secret trong repo; **ASR worker KHÔNG thừa kế secret qua EnvironmentFile** (finding RT-5).
- [ ] Không phiên live/meeting nào bị nghe caption của phiên khác (namespace track id per-session — finding RT-3).

## Red Team Review

### Session — 2026-07-07
**Reviewers:** 3 (Security Adversary/Fact Checker, Failure Mode Analyst/Flow Tracer, Assumption Destroyer/Scope+Contract) — Full tier, mọi finding kèm `file:line`, controller tự verify code trước adjudicate.
**Findings:** 27 raw → 15 unique (evidence-filter: 0 reject — tất cả có citation). **Tất cả Accept.** Không finding nào đảo quyết định user (chỉ tấn công cách thực thi).
**Severity:** 5 Critical, 7 High, 3 Medium.
**Reports:** `reports/from-code-reviewer-to-planner-red-team-{security-adversary,failure-mode-analyst,assumption-destroyer}-plan-review-report.md`

| # | Finding | Sev | Applied To |
|---|---------|-----|------------|
| RT-1 | `--cpu-threads` không tồn tại — `main.py:67` hardcode `os.cpu_count()`, không argparse → gate đo sai + sidecar phải sửa | Critical | Ph1, Ph3, Dec#7/#8 |
| RT-2 | RAM peak thật ~2.9–3.2GB > MemoryMax → OOM (vs spike report plan tự trích) | Critical | plan.md budget, Ph1, Dec#7 |
| RT-3 | Cross-user caption leak — track id namespace phẳng + `stream_start` idempotent (`stream_server.py:123`) | Critical | Ph3, Ph4 |
| RT-4 | Luồng caption bot→client Phase 4 KHÔNG tồn tại — bot `canPublishData=false`, store POST role=host (`index.mjs:147`), `caption-feed.ts:44` chỉ render host | Critical | Ph4 |
| RT-5 | Public upload media-decode RCE + secrets vào ASR worker qua EnvironmentFile (`meeting-server.service:12`) | Critical | Ph2, Ph5 |
| RT-6 | `git mv` phá ≥3 consumer chưa liệt kê (`generate-token.mjs:6`, `run-local.sh:27`, unit) + no rollback + createApp contract + store in-memory mất mỗi deploy | High | Ph2 |
| RT-7 | IDOR `/api/jobs/:id` thiếu ownership check | High | Ph2 |
| RT-8 | Cap ≤2 không enforce trong code (chỉ comment); bot nhiều track đếm hụt | High | Ph3, Ph4 |
| RT-9 | Queue no boot-reconcile + `main.py` không exit sau `done` (`:114`) → orphan + không nhả RAM | High | Ph2 |
| RT-10 | Empty-password bypass — validateConfig không require secret mới; limiter XFF | High | Ph2 |
| RT-11 | Transcript `.txt/.srt` trên disk thoát retention; `segments_json` không có nguồn (`done` không carry) | High | Ph2, Ph5, sidecar |
| RT-12 | Claim "worklet→PCM đã chứng minh" sai — G6 chưa QA, resample code không tồn tại (`use-audio-tap-probe.ts:82`) | High | Ph3 |
| RT-13 | JWT WS query string lọt access log (TTL 6h replay) | Medium | Ph3 |
| RT-14 | Webhook chưa config `livekit.yaml` (`devkey:secret`) + replayable + bot lifecycle chỉ dựa room_finished | Medium | Ph4 |
| RT-15 | Fact errors: `stream_end`(≠stream_stop), `progress`/`done`(≠transcribe_msg), Caddy repo vs nginx box, test runner custom(≠node:test), drop-newest(≠drop-oldest), "70% reuse" thổi phồng path live | Medium | tất cả file |

### Whole-Plan Consistency Sweep
- Files reread: plan.md + phase-01..05.
- Decision deltas: sidecar-cần-sửa (đảo "không đụng"); RAM peak 2.2→2.9-3.2GB; protocol `stream_stop`→`stream_end`; events `transcribe_msg`→`progress`/`done`; test `node:test`→custom `run-tests.mjs`; infra nginx→Caddy(repo)/nginx(box) chọn 1; queue drop-oldest→drop-newest; cap theo ASR track; caption bot→client redesign.
- Reconciled stale references: đã sửa tại chỗ trong 6 file (không chỉ annotate).
- **Unresolved contradictions: 0.**

## Unresolved questions

1. ~~Auth~~ → **RESOLVED (validate 2026-07-07): basic-auth edge, single-tenant MVP** (Decision #5). SSO OneDash = Phase 5 future.
2. ~~Domain~~ → **RESOLVED: `scribe.inetdev.io.vn`** (Cloudflare proxied, Decision #9).
3. ~~Reverse-proxy Caddy/nginx~~ → **RESOLVED: Cloudflare tunnel, không đụng nginx :443** (Decision #9).
4. **VM resize** → **DEFER tới Phase 1** (đo RAM peak thật rồi quyết; giữ 8GB + arbiter để đo).
5. Plan 260626 Phase 3 host-relay: legacy (mặc định; code giữ trong repo, không xóa) — chờ xác nhận cuối.

## Validation Log

### Session — 2026-07-07 (sau Red Team)
**Trigger:** `/ck:plan validate` sau red-team. Verification pass skip (Red Team Review đã có evidence file:line, 0 `[UNVERIFIED]`). 4 câu hỏi external/business.

**Decisions:**
1. **RAM/VM:** giữ 8GB, đo RAM peak thật ở Phase 1 rồi mới quyết resize (arbiter đủ an toàn để đo).
2. **Auth:** basic-auth tầng edge → single-tenant MVP; bỏ JWT login + ownership app-level ở Phase 2 (chỉ 1 identity → RT-7/RT-10 moot); SSO OneDash + multi-user + ownership = Phase 5.
3. **Expose:** Cloudflare tunnel tới `scribe.inetdev.io.vn`; KHÔNG thêm nginx/Caddy vhost :443 (RT-15 reverse-proxy → giải bằng tunnel, không phải chọn Caddy vs nginx).
4. **Domain:** `scribe.inetdev.io.vn` (Cloudflare proxied).

**Impact:** Phase 2 (auth đơn giản hóa + deploy qua cloudflared + trust CF XFF); Phase 3 (WSS qua tunnel); Phase 5 (SSO + ownership khi multi-user). RT-5 (upload RCE) GIỮ NGUYÊN — auth edge không cứu media-decode.

### Whole-Plan Consistency Sweep
- Files reread: plan.md + phase-01..05.
- Decision deltas: auth JWT→basic-auth-edge single-tenant; ownership→defer Phase 5; reverse-proxy vhost→Cloudflare tunnel; domain→scribe.inetdev.io.vn.
- Reconciled: Phase 2 auth/deploy, Phase 3 WSS, Phase 5 SSO/ownership sửa cho khớp.
- **Unresolved contradictions: 0.**
