---
title: "Realtime Meeting (camera/mic) + Live Transcribe tiếng Việt"
description: "Mở rộng OneScribe thành nền tảng họp video (LiveKit SFU) có live-transcribe tiếng Việt tích hợp; host transcribe đa-track, server pure-relay, web client Vue 3"
status: pending
priority: P2
branch: "main"
tags: [webrtc, livekit, asr, phowhisper, vue, conferencing]
blockedBy: []
blocks: []
created: "2026-06-26T07:19:08.047Z"
createdBy: "ck:plan"
source: skill
---

# Realtime Meeting (camera/mic) + Live Transcribe tiếng Việt

## Overview

Mở rộng OneScribe (Tauri + Vue 3 + ViUI, transcribe offline PhoWhisper) thành **nền tảng họp video realtime có transcribe tiếng Việt tích hợp**. Conferencing = commodity → dùng **LiveKit SFU mã nguồn mở**, KHÔNG tự viết. Transcribe = hero → **host (desktop) transcribe mọi track**, push caption lên server relay; web client (Vue) chỉ xem video + đọc caption.

**Định vị:** không cạnh tranh UX họp với Meet/Zoom; điểm bán hàng là **live-transcribe tiếng Việt per-speaker + privacy (server không lưu media)**.

## Quyết định kiến trúc (đã chốt — nguồn chân lý)

| # | Quyết định |
|---|---|
| 1 | SFU = **LiveKit** self-host (Apache 2.0). KHÔNG mesh, KHÔNG tự viết SFU. |
| 2 | Server **KHÔNG lưu media** (video/audio ephemeral, chỉ relay). |
| 3 | Server giữ **text transcript** KB-scale, **retention 1 ngày → purge**. |
| 4 | Chỉ **host** (desktop, có model PhoWhisper) transcribe **mọi track**, gate bằng **active-speaker (VAD)**. |
| 5 | Host **cố định 1 người/phòng**; web participant KHÔNG thể là host. |
| 6 | Host rớt → caption **dừng** (chấp nhận, KHÔNG chuyển quyền host). |
| 7 | Web participant = consumer thuần (xem video + đọc caption). |
| 8 | Recording MP4/audio = tùy chọn, **phía client/host**, KHÔNG ở server. |
| 9 | FE thống nhất **Vue 3 + Vuetify + ViUI** cho cả desktop lẫn web client. |
| 10 | Streaming ASR: **vad-cut (utterance-based) + max-utterance cap ~8s** — _cập nhật Session 2: LocalAgreement đo thực tế NO-GO trên CPU yếu (re-transcribe → RTF 5x; xem spike §9), thay bằng vad-cut_. Cap số stream ASR đồng thời **≤2** (Whisper có overhead encoder ~cố định/lần gọi → không cắt vụn). `WhisperModel`(PhoWhisper, KHÔNG BatchedInferencePipeline) + VAD webrtcvad/built-in (KHÔNG silero). Live = small ("tham khảo"); biên bản chuẩn = medium/large batch (tái dùng Mode C). |

## Phases

| Phase | Name | Status |
|-------|------|--------|
| 0 | [Bot-join / Loopback Value Validation](./phase-00-bot-join-loopback-validation.md) — **VALUE GATE, làm trước nhất** | Pending |
| 1 | [Spike Streaming ASR](./phase-01-spike-streaming-asr.md) — **FEASIBILITY GATE** | In Progress (G1 GO + G3+G4 GO; G2 chưa chốt, G5 kém — proxy TTS; G6 CODE DONE 2026-06-29, chờ QA runtime) |
| 2 | [LiveKit Media Core](./phase-02-livekit-media-core.md) | In Progress (slice nền: FE+CSP H1+quyền media H2+infra dev done; compile✓ + code-review✓ C1 fixed; chờ QA E2E + xác nhận no-white-screen) |
| 3 | [Host Transcribe Relay](./phase-03-host-transcribe-relay.md) | Pending |
| 4 | [Web Client Vue](./phase-04-web-client-vue.md) | Pending |
| 5 | [Server Infra Retention](./phase-05-server-infra-retention.md) | Pending |
| 6 | [AI Biên bản / Tóm tắt sau họp (Claude)](./phase-06-ai-meeting-minutes-claude.md) — ⏸️ **POST-COMPLETION, deferred** | Deferred |

## Thứ tự & phụ thuộc

- **Phase 0 (VALUE GATE) làm trước nhất** — validate rẻ (1-2 ngày) bằng loopback + sidecar có sẵn rằng transcribe tiếng Việt cuộc họp "đủ dùng". NO-GO → xem lại có nên xây platform.
- **Phase 1 (FEASIBILITY GATE)** — nếu host không transcribe nổi streaming đa-track ở latency chấp nhận trên laptop văn phòng, cả mô hình phải xem lại. Gate sống/chết kỹ thuật.
- Phase 2 độc lập Phase 1 (song song được), nhưng Phase 3 phụ thuộc CẢ 1 và 2.
- Phase 4 phụ thuộc 2 (room/media) + 3 (luồng caption).
- Phase 5 (infra cứng: auth, TURN, retention) chốt sau khi 2–4 chạy thông end-to-end.
- **Cổng tái đánh giá:** sau Phase 0 GO và sau Phase 1 GO đều dừng kiểm lại trước khi cấp ngân sách phase tiếp (tránh auto-commit toàn bộ).
- **Phase 6 (AI biên bản — Claude) = POST-COMPLETION:** KHÔNG nằm trong chuỗi build MVP. Chỉ khởi động **sau khi Phase 0–5 đã triển khai hoàn thiện & chạy ổn định production**. Là hậu xử lý transcript (KHÔNG phải ASR — Claude không transcribe audio); phụ thuộc transcript từ Phase 3 + retention Phase 5.

## Rủi ro lớn nhất (chi tiết trong từng phase)

1. **Streaming ASR đa-track trên CPU host** (Phase 1) — Whisper không streaming bản chất; phải VAD-gate + cửa sổ trượt. Tải tăng theo số người nói đồng thời.
2. **Routing audio remote track (WebRTC Opus) → sidecar Python** (Phase 1/3) — đường WebView2 JS → Rust → PhoWhisper chưa từng có trong stack.
3. **Host = single point of failure** cho caption (chấp nhận theo quyết định #6).
4. **Build thêm 1 web client** (Phase 4) — scope thật, không miễn phí công sức.

## Tham số đã chốt (validation 2026-06-26)

| Tham số | Giá trị | Ảnh hưởng |
|---|---|---|
| **Quy mô MVP** | **≤ 8 người/phòng** | Trần tải transcribe host, simulcast, egress |
| **Máy host tối thiểu** | **Laptop văn phòng** (i5 ~4 nhân, 8–16GB, no GPU) | Ngưỡng GO/NO-GO Phase 1 |
| **Cấu trúc web (Phase 4)** | **Monorepo workspaces** (apps/desktop + apps/web + packages/meeting-shared) | DRY, 1 nguồn component |
| **Recording (Strategy B)** | **Defer sau MVP** — MVP chỉ live caption | Gọn scope Phase 3 |
| **Môi trường infra** | **Cài NATIVE — KHÔNG Docker.** Test LOCAL trên WSL Ubuntu trước rồi mới remote. LiveKit = binary Go đơn (`get.livekit.io`); coturn/redis/caddy = gói `apt`; điều phối bằng **systemd** (Egress/recording cần Docker nhưng đã loại) | Phase 2 + Phase 5 |

## Out of scope (YAGNI — chốt rõ)

- Server-side recording / mixing (MCU) — đã loại.
- Host migration / failover transcribe — đã loại (quyết định #6).
- Lưu media trên server — đã loại (quyết định #2).
- **Recording client (audio/MP4) + biên bản chuẩn batch sau họp — DEFER sau MVP** (validation 2026-06-26).
- Phòng > 8 người — sau khi Phase 1 chứng minh host kham được.
- Cạnh tranh tính năng họp nâng cao (breakout room, ảo hóa nền, reactions) — sau, không phải MVP.

## Validation Log

### Session 1 — 2026-06-26
**Trigger:** `/ck:plan validate` sau khi tạo plan (auto-detect mode, không có gate tự chạy).
**Questions asked:** 4

#### Verification Results
- Claims checked: 8 (file references existing) — Tier: Full (5 phase)
- Verified: 8 | Failed: 0 | Unverified: 0
- Bằng chứng: `src/App.vue`, `src/plugins/vuetify.ts`, `src/assets/styles/viui-tokens.css`, `sidecar/transcriber/main.py`, `src-tauri/src/transcribe.rs`, `src/design-system/vi-ui/` (17 Vi*, không có grid/layout → khớp risk Phase 2), `src/composables/use-*.ts` (convention khớp).

#### Questions & Answers
1. **[Scope]** Số người tối đa/phòng cho MVP? → **≤ 8 người**. Rationale: giữ tải transcribe host + egress trong tầm; mở rộng sau Phase 1.
2. **[Risks]** Máy host tối thiểu để Phase 1 đo GO/NO-GO? → **Laptop văn phòng (i5 ~4 nhân, 8–16GB, no GPU)**. Rationale: đo trên cấu hình xấu nhất thực tế = ngưỡng đậu/rớt.
3. **[Architecture]** Cấu trúc web client Phase 4? → **Monorepo workspaces**. Rationale: DRY, một nguồn component dùng chung desktop/web.
4. **[Scope]** Recording + biên bản chuẩn (Strategy B) trong MVP? → **Defer sau MVP**. Rationale: live caption là core; recording thêm scope, YAGNI.

#### Impact on Phases
- Phase 1: chốt máy host tối thiểu = laptop văn phòng; trần ASR đồng thời tính theo ≤8 người.
- Phase 2: grid + simulcast tối ưu cho ≤8 tile (sửa "4–6" → "tới 8" ở 3 chỗ).
- Phase 3: Strategy A (caption-as-record) giữ; Strategy B (ghi audio/MP4 + batch) DEFERRED; host transcribe ≤8 track.
- Phase 4: khóa monorepo workspaces (gỡ nhánh "web riêng" ở architecture/step 1/risk/related-files).
- Phase 5: đo egress ở quy mô ≤8 người (sửa "10–20").

#### Whole-Plan Consistency Sweep
- Đã re-read `plan.md` + 5 phase sau propagation.
- Quét thuật ngữ cũ (4–6 / 10–20 người, conditional "nếu monorepo", "chốt khi cook") → đã dọn sạch.
- Match còn lại ("web app riêng đã loại") là ghi chú phương án bị loại, đúng ngữ cảnh — không phải mâu thuẫn.
- **Kết quả: 0 mâu thuẫn tồn đọng. Plan nhất quán, đủ điều kiện implement.**

### Session 2 — 2026-06-29
**Trigger:** user trả lời 3 open question sau `/ck:predict` 5-persona.

#### Quyết định
1. **[Platform/macOS]** Meeting MVP = **Windows-first** (chốt). macOS (WKWebView + NSCamera/Mic entitlements) defer sau MVP → đóng open question macOS của Red Team.
2. **[Workflow Git]** **Code + push THẲNG trên `main`, KHÔNG nhánh, KHÔNG PR.** Ghi đè khuyến nghị "nhánh riêng" của Red Team (H6/Phase 4 risk). Giữ INTENT revertability bằng: **tag checkpoint trước mỗi refactor rủi ro + commit nhỏ atomic** → `git revert`/reset về tag khi hỏng.
3. **[Mẫu speech G1/G2]** **KHÔNG có mẫu thật.** → dùng **TTS tiếng Việt đa người (skill ai-multimodal) làm proxy** đo RTF/latency (số LẠC QUAN: không có cross-talk/giọng thật). Cần 1 vòng xác nhận speech thật trước GO cuối nếu khả thi.
4. **[Gate override — Phase 2 trước G6]** User chủ động chọn cook **Phase 2 (LiveKit Media Core)** dù gate Phase 1 chưa đóng (G6 chân audio WebView2 chưa làm). Đã cảnh báo rủi ro (chân audio chưa chứng minh trước khi đổ công media); user chấp nhận. → Phase 2 build trước, G6 vẫn nợ trước khi cấp ngân sách Phase 3.

#### Impact on Phases
- Phase 1: đóng G1/G2 bằng TTS-proxy (ghi rõ "proxy"); G6 (chân WebView2) vẫn cần app chạy.
- Phase 2/4: macOS out-of-scope MVP (chỉ target `nsis`); mọi refactor làm trên `main` + tag checkpoint.
- Phase 4 (monorepo H6): rủi ro cao + không nhánh để revert → BẮT BUỘC tag trước refactor + verify `tauri dev`/`build` release ngay sau.

## Red Team Review

### Session 1 — 2026-06-26
**Reviewers:** 4 (Security Adversary, Failure Mode Analyst, Assumption Destroyer, Scope & Complexity Critic) — Full tier, mỗi reviewer có verification role, mọi finding kèm `file:line`.
**Findings:** 38 raw → 21 unique → 15 trình bày. **Accepted (kỹ thuật/bảo mật):** 18 áp dụng trực tiếp. **Surfaced (đảo quyết định user):** 3 → user quyết.
**Severity:** 8 Critical, 7 High, 6 Medium.
**Reports:** `reports/from-code-reviewer-to-planner-red-team-{security-adversary,failure-mode-analyst,assumption-destroyer,scope-complexity-critic}-plan-review-report.md`

#### Quyết định của user trên 3 finding đảo ngược (guard user decisions)
| Finding | Red-team đề xuất | User chốt |
|---|---|---|
| Overscope vs bot-join | Validate bằng loopback trước | **Chèn Phase 0 bot-join/loopback** (làm trước nhất) |
| Self-host vs LiveKit Cloud | Cloud free tier cho MVP | **Giữ self-host LiveKit** (Decision #1 không đổi) |
| Defer web client/monorepo | Cắt khỏi MVP | **Giữ web client Vue + monorepo** trong MVP |

#### Findings table (accepted → applied)
| # | Finding | Sev | Disposition | Applied To |
|---|---------|-----|-------------|------------|
| C1 | BatchedInferencePipeline ≠ whisper_streaming + dual-model RAM OOM | Critical | Accept | Phase 1 |
| C2 | Sidecar đơn luồng: batch chặn streaming + deadlock PCM backpressure | Critical | Accept | Phase 1, 3 |
| C3 | whisper_streaming không có trên PyPI (install/bundle) | Critical | Accept | Phase 1 |
| C4 | silero-vad kéo PyTorch ~2GB (bloat/conflict ctranslate2) | Critical | Accept | Phase 1 |
| C5 | Phase 1 spike thiếu chân JS→Rust→Python PCM (unknown khó nhất) | Critical | Accept | Phase 1 |
| C6 | Token service nhận role từ client + không enforce 1-host | Critical | Accept | Phase 5 |
| C7 | Caption injection qua data channel (guest giả mạo) | Critical | Accept | Phase 3 |
| C8 | Caption store API không auth (đọc/bơm phòng khác) | Critical | Accept | Phase 5 |
| H1 | CSP=null → XSS → gọi transcribe_files tùy ý | High | Accept | Phase 2 |
| H2 | Camera/mic chưa khai báo capability + macOS WKWebView Info.plist | High | Accept | Phase 2 |
| H3 | Sidecar crash giữa họp không recovery/không event | High | Accept | Phase 3 |
| H4 | Reader thread trộn caption + đụng listener batch | High | Accept | Phase 3 |
| H5 | ViRow/ViCol thiếu ở vi-ui local vs npm → shared build gãy | High | Accept | Phase 2, 4 |
| H6 | Monorepo refactor phá find_script/frontendDist/build_pyinstaller.ps1/capabilities | High | Accept | Phase 4 |
| M1-6 | TURN ephemeral cred; mic revocation onmute; path traversal canonicalize; .npmrc+pin @hobui/viui; resolve() venv path; stream_pcm schema | Medium | Accept | Phase 1/3/4/5 |

### Whole-Plan Consistency Sweep
- Files reread: plan.md + phase-00..05.
- Decision deltas: +Phase 0; silero→webrtcvad/built-in; whisper_streaming pip→copy whisper_online.py; BatchedInferencePipeline→WhisperModel(stream); self-host giữ; web client+monorepo giữ.
- Reconciled stale references: 3 ("Silero VAD" ở phase-01 Related Files, phase-03 architecture, plan.md Decision #10) → đã sửa sang webrtcvad/built-in + LocalAgreement + WhisperModel.
- Tham chiếu "silero/whisper-streaming/role=host" còn lại CHỈ trong `reports/` (artifact lịch sử reviewer) và tên finding C4/C6 — đúng ngữ cảnh, không sửa.
- **Unresolved contradictions: 0.**

### Open question (cần user quyết trước Phase 2)
- ~~**macOS:** Meeting MVP nên Windows-first?~~ → **RESOLVED (Session 2, 2026-06-29): Windows-first.** macOS defer sau MVP (WKWebView + entitlements; `tauri.conf.json` chỉ target `nsis`).
