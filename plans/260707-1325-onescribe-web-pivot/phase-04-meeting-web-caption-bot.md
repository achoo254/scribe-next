---
phase: 4
title: "Meeting Web Caption Bot"
status: pending
effort: "L"
priority: P2
dependencies: [3]
---

# Phase 4: Meeting Web Caption Bot

## Overview

Meeting hoàn toàn trên web: room UI đầy đủ (publish camera/mic) + **caption bot server-side** thay vai trò host-desktop-transcribe của plan 260626. Bot Python join phòng LiveKit qua `livekit-rtc`, subscribe audio tracks, active-speaker gating, feed vào đường live ASR Phase 3. **Phase rủi ro cao nhất — có mini-spike gate riêng ở bước 1.**

## Requirements

- Functional: user tạo/join phòng trên web (không cần desktop); caption realtime đúng người nói cho ≤8 người; caption chỉ từ bot (chống giả mạo — giữ tinh thần C7).
- Non-functional: bot dùng chung cap ≤2 stream ASR toàn cục (stream-registry Phase 3); MVP giới hạn **1 phòng có caption đồng thời**; host-lock C6 của token service chuyển ngữ nghĩa: không còn host desktop, "host" = người tạo phòng (quyền quản phòng, KHÔNG transcribe).

## Architecture

```
LiveKit SFU (ĐÃ chạy .65:7880) ── webhook room_started/room_finished ──► onescribe-server
   (⚠️ webhook CHƯA config, xem RT-14)                                     │ spawn/kill per room
                                                                            ▼
apps/web MeetingRoomPage (publish     meeting_bot.py (`livekit-rtc`):
 camera/mic ĐÃ CÓ ở use-livekit-viewer) join room (token role=bot, subscribe-only, hidden)
                                          ├─ audio 48k → resample 16k mono
                                          ├─ active-speaker gating ≤2 (code MỚI — chỉ có DESIGN
                                          │   ở plan 260626, KHÔNG có code để port, RT-8)
                                          └─ PCM → WS /ws/live nội bộ (track id namespaced, RT-3)
                                              → cùng stream process small (Phase 3, cap dùng chung)
                                              → caption → [RT-4] đường tới client: xem dưới
```

Chọn "bot = 1 client WS nội bộ" thay vì bot tự nạp model: 1 model process duy nhất, cap chia sẻ tự nhiên, bot chỉ là I/O.

### [RT-4 — CRITICAL] Luồng caption bot→client CHƯA tồn tại — phải thiết kế lại
Code hiện tại KHÔNG có đường hợp lệ từ bot tới client:
- `caption-feed.ts:44` `onData` chỉ render nếu sender có `metadata.role==="host"` (data channel).
- Caption store POST chặn `role!=="host"` (`index.mjs:147`).
- Token bot dự kiến `canPublishData=false` → không đẩy được data channel.

**Chốt đường A (khuyến nghị):** bot dùng **identity ký metadata `role="host"`** (bot LÀ transcriber chính thức của phòng, không phải guest) + `canPublishData=true`; bot publish caption qua data channel → `caption-feed.ts` render KHÔNG cần sửa; caption store POST cũng qua bot (role=host hợp lệ) cho late-joiner. Chống giả mạo giữ nguyên: guest vẫn `canPublishData=false`, chỉ bot (server cấp token role=host, secret) publish được. **Ngữ nghĩa "host" đổi:** host = bot server-side (transcriber), KHÔNG phải người tạo phòng. Người tạo phòng = quyền quản (kick/lock), role riêng, KHÔNG publish caption.
**Cần sửa `caption-feed.ts`** chỉ nếu muốn tách role `transcriber` khỏi `host` — MVP dùng lại `host` để zero-change client.

## Related Code Files

- Create: `sidecar/transcriber/meeting_bot.py` (+ `requirements-bot.txt`: `livekit`, `websockets`); `server/onescribe-server/bot-manager.mjs` (spawn/kill theo webhook + **idle-timeout fallback**, restart-on-crash) + tests; `apps/web/src/pages/CreateRoomPage.vue`.
- Modify: `server/onescribe-server/meeting.mjs` (token bot role=host server-side-only + secret, RT-4; **webhook: `verifyWebhook` hiện CHỈ xử lý `participant_left` (index.mjs:169) — `room_started`/`room_finished` là code MỚI**, RT-14; thêm verify `exp`/nonce + rate-limit `/webhook`); `apps/web/src/MeetingRoomPage.vue` + `packages/meeting-shared/src/use-livekit-viewer.ts` (**publish camera/mic path — kiểm code đã có tới đâu, plan 260626 Phase 4 nói web = consumer thuần** nên có thể phải thêm); `apps/web/src/router.ts`. `caption-feed.ts` chỉ sửa nếu tách role transcriber (RT-4, MVP không cần).
- **[RT-14] Config LiveKit webhook:** `infra/livekit/livekit.yaml` hiện chỉ có `devkey: secret` (dev), KHÔNG có section `webhook`. Phải thêm `webhook: {api_key, urls}` trỏ onescribe-server + **key production thật (không devkey)** trên box .65. Không có = bot không bao giờ spawn.
- Đọc tham khảo: plan 260626 `phase-03-host-transcribe-relay.md` (gating/track-id = **design-only, viết mới sang Python**, RT-8), `server/dev-token/generate-token.mjs` (dev role, không production).

## Implementation Steps

1. **Mini-spike GATE (làm trước, ~1 buổi):** `meeting_bot.py` bare-bones join phòng staging, subscribe 1 audio track, resample, dump PCM ra file → verify bằng tai + RTF pipeline nối vào WS live. FAIL (livekit-rtc không chạy nổi trên box/glibc cũ) → phương án B: bot chạy trên máy khác trỏ về, hoặc defer meeting caption. KHÔNG build tiếp khi spike chưa xanh.
2. **[RT-4] Token bot** cấp qua internal secret (không public); bot `canSubscribe=true, canPublish=false, **canPublishData=true**` (để publish caption data channel), metadata ký `role="host"`, hidden participant. Guest giữ `canPublishData=false` (chống giả mạo).
3. `meeting_bot.py` (code MỚI): track lifecycle theo participant vào/ra; **active-speaker gating viết mới** (energy/VAD per-track, top-2, RT-8 — không có code port); map track→identity→tên; đẩy PCM qua WS nội bộ với **track id = `<room>-<identity>` sanitize khớp `_TRACK_RE`** (`[A-Za-z0-9_-]`, RT-3/RT-9 — identity SSO có `.`/`@` phải thay, tránh collision).
4. `bot-manager.mjs`: webhook `room_started` → spawn bot (chỉ khi chưa có phòng caption active — MVP giới hạn 1); `room_finished` **VÀ idle-timeout fallback** (RT-14 — mất room_finished = zombie chiếm slot ASR toàn cục) → kill; crash → respawn ≤3 lần rồi báo "caption tạm dừng".
5. Caption chain: bot publish caption qua **data channel** (role=host metadata) → `caption-feed.ts` render (zero-change) + POST caption store cho late-joiner (role=host hợp lệ). Client CHỈ render nguồn có role=host ký server (2 lớp chống giả mạo, C7 giữ nguyên).
6. FE: CreateRoomPage (tạo phòng + share link), MeetingRoomPage publish camera/mic + caption overlay (caption-feed.ts đã có), trạng thái "caption bởi bot (small — tham khảo)".
7. E2E staging: 3 browser (2 máy + mobile) vào phòng, nói tiếng Việt xen kẽ → caption đúng tên, độ trễ ghi nhận; đo RAM/CPU box suốt 15'.
8. Cập nhật `docs/system-architecture.md`: sơ đồ meeting mới (bot thay host), đánh dấu host-relay desktop = legacy.

## Success Criteria

- [ ] Mini-spike bước 1 GO: bot join + PCM sạch + pipeline nối được (có file/số chứng minh).
- [ ] **Caption bot HIỂN THỊ trên client web** (RT-4 — đường data channel role=host hoạt động end-to-end), đúng người nói; ≤8 người media ổn.
- [ ] 2 người nói chồng → gating ≤2 ASR track; đếm chung cap với phiên live recording (RT-8) → vượt thì từ chối có thông báo.
- [ ] Guest inject data channel caption giả → client KHÔNG render (guest `canPublishData=false` + role check).
- [ ] Bot crash → respawn ≤30s; `room_finished` HOẶC idle-timeout → bot biến mất (RT-14, không zombie chiếm slot); 2 session/room trùng tên track → không lẫn caption (RT-3).
- [ ] Webhook LiveKit đã config + key production (RT-14); box: phòng 8 người + caption 15' — RAM đúng ngân sách, service khác không degrade.

## Risk Assessment

- **`livekit-rtc` Python trên box** (wheel manylinux, glibc AlmaLinux/CentOS?) chưa từng thử → mini-spike bước 1 là gate cứng.
- Resample 48k→16k chất lượng kém làm ASR tệ → dùng `livekit.rtc.AudioResampler` hoặc soxr, verify bằng tai ở spike.
- 1 phòng caption MVP có thể gây bất ngờ UX → hiển thị rõ trạng thái "phòng khác đang dùng caption" khi bị từ chối.
- Ngữ nghĩa "host" đổi so plan 260626 → cập nhật docs + token tests, tránh lẫn C6 host-lock cũ (lock giờ áp cho bot, không phải người).
