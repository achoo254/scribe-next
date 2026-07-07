---
phase: 3
title: "Live Caption Recording"
status: pending
effort: "M"
priority: P2
dependencies: [2]
---

# Phase 3: Live Caption Recording

## Overview

Live caption khi ghi trên web: browser đẩy PCM 16k qua WebSocket → stream ASR small (vad-cut) → caption realtime; bấm dừng → medium final pass thay bản draft (two-pass đã chốt). Thêm ghi màn hình + system-audio (Chrome/Edge).

## Requirements

- Functional: ghi mic/screen/system-audio có caption realtime; dừng → transcript final medium tự enqueue; caption draft đánh dấu "tham khảo (small)".
- Non-functional: cap ≤2 stream ASR đồng thời TOÀN CỤC (chia sẻ với meeting bot Phase 4 — registry chung); latency caption ≤4s sau dứt câu (chuẩn spike); phiên thứ 3 bị từ chối lịch sự.

## Architecture

```
Browser: getUserMedia / getDisplayMedia({audio:true})
  ├─ AudioWorklet → PCM int16 16k mono               ([RT-12] G6 CHƯA verify runtime;
  │     └─ WS binary frames → /ws/live (ticket ngắn hạn)  resample code chưa tồn tại — xem dưới)
  └─ MediaRecorder song song ghi full audio → dừng → upload → job medium (Phase 2 pipeline)

onescribe-server: ws.mjs (thư viện `ws` — ngoại lệ zero-dep có chủ đích)
  └─ bridge WS ↔ persistent `python3 main.py --stream` (spawn 1 lần, small nạp warm)
       protocol stdio ĐÃ CÓ (stream_server.py:11-19): stream_start / stream_pcm(base64) /
       **stream_end** theo track id; events **caption / backpressure / stream_error**
  └─ caption events → WS → FE (caption-feed.ts: reuse ~15 dòng push `addCaption`, RT-15)
```

- **[RT-3] Track id namespace per-session (CRITICAL):** `stream_server.py:123` `stream_start` idempotent + dict global → 2 session trùng id = giọng lẫn feed. WS bridge PHẢI gán track id = `<sessionUUID>:<trackName>` (unique tuyệt đối). `_TRACK_RE` (`stream_server.py:32`) chỉ cho `[A-Za-z0-9_-]{1,64}` → dùng `-` thay `:`, sanitize + map ngược ở Node.
- **[RT-8] Cap ≤2 KHÔNG có trong sidecar** (chỉ comment `stream_server.py:9`) — `stream-registry.mjs` ĐẾM theo **số ASR track active toàn cục** (chia sẻ Phase 4 bot), enforce **trước khi gửi `stream_start`**; vượt → từ chối, không gửi.
- **[RT-12] Worklet resample:** không có code "decimation+low-pass" để port; `use-audio-tap-probe.ts:82` chỉ xin `AudioContext({sampleRate:16000})` rồi hy vọng — G6 "code done, chưa QA runtime". Bước 4 phải **verify thật** trình duyệt cấp đúng 16k; browser không cấp → tự downsample trong worklet (fallback bắt buộc, không giả định).
- Stream process vòng đời: spawn phiên đầu, kill sau idle N phút. **[RT-2] arbiter:** registry từ chối mở phiên live nếu batch đang chạy (đọc `cgroup memory.current` vs `memory.max`, KHÔNG `/proc/meminfo` — không thấy giới hạn cgroup).

## Related Code Files

- Create: `server/onescribe-server/{ws.mjs,stream-registry.mjs}` + tests; `apps/web/src/pages/RecordLivePage.vue`; `apps/web/src/audio/pcm-worklet.ts` (port từ desktop `src/composables/use-audio-tap-probe.ts` — G6); `apps/web/src/audio/use-live-stream.ts`.
- Modify: `server/onescribe-server/index.mjs` (mount WS upgrade); `apps/web/src/router.ts`; `packages/meeting-shared/src/caption-feed.ts` (nếu cần thêm nguồn WS ngoài LiveKit data channel — giữ interface chung).
- **Sửa nhỏ (Decision #8):** `sidecar/transcriber/stream_server.py` + `stream_session.py` — thêm điều khiển `cpu_threads` qua env (RT-1); cap ≤2 **KHÔNG có sẵn** (chỉ comment `stream_server.py:9` — RT-8), enforce ở Node; timeout flush utterance (RT-3). Đọc tham khảo (không sửa): desktop `src/composables/use-live-caption.ts` (design gating, không có code port).

## Implementation Steps

1. `ws.mjs`: HTTP upgrade + **[RT-13] xác thực bằng ticket ngắn hạn** (issue qua `/api/ws-ticket` cần JWT, ticket TTL ~30s dùng 1 lần) thay `?token=JWT` (JWT 6h lọt access log). Binary frame = PCM chunk, JSON frame = control. Backpressure: sidecar đã **drop-NEWEST** khi queue đầy (`stream_server.py:92-99`, RT-15) — Node cũng bound buffer, không deadlock (C2).
2. Bridge → spawn `main.py --stream` (small, env threads 2), map WS session ↔ **track id namespaced** (RT-3); parse events `caption`/`backpressure`/`stream_error`. **[RT-9/RT-15] respawn là code MỚI cho Node multi-client** (pattern H3/L1 cũ là Rust 1-client — không port trực tiếp): handle EPIPE trên stdin child (không handler = sập server), respawn + báo client.
3. `stream-registry.mjs`: cap toàn cục theo **ASR track** (RT-8); `acquire()/release()` + arbiter RAM (RT-2); vượt cap/RAM → WS close code + "đang bận, thử lại sau".
4. FE `RecordLivePage.vue`: chọn nguồn (mic / màn hình+system-audio); feature-detect `getDisplayMedia` audio — không phải Chrome/Edge → banner "trình duyệt không hỗ trợ ghi âm hệ thống" (Decision #3); caption feed hiển thị draft; dừng → upload full audio → link job final.
5. Two-pass hoàn tất: job final xong → UI thay draft bằng transcript medium, đánh dấu bản chuẩn.
6. Tests: ws bridge với fake stream process (echo caption); registry cap; FE build + vue-tsc.
7. Deploy + đo latency thật trên staging qua HTTPS/WSS **qua Cloudflare tunnel** (`scribe.inetdev.io.vn`, cloudflared hỗ trợ WS upgrade; kiểm CF không buffer/timeout WS dài — chỉnh nếu cần).

## Success Criteria

- [ ] Ghi mic trên Chrome: caption ≤4s sau dứt câu (≥10 câu); **worklet cấp/tự-downsample đúng 16k đã verify** (RT-12).
- [ ] Ghi màn hình + system-audio Chrome/Edge ra caption; Safari/Firefox thông báo đúng.
- [ ] Dừng ghi → job medium tự tạo → transcript final thay draft.
- [ ] **2 session mở track cùng tên → KHÔNG lẫn caption** (namespace per-session, RT-3); phiên vượt cap ASR bị từ chối rõ; batch đang chạy → phiên live bị hoãn (arbiter, RT-2).
- [ ] Kill stream process giữa phiên → client báo lỗi + respawn; **EPIPE stdin không sập server** (RT-9).
- [ ] Box: RAM đúng số Phase 1 đo; service khác không degrade; utterance không bị treo mở vô hạn khi im lặng giữa chừng (timeout flush).

## Risk Assessment

- WSS qua Cloudflare tunnel có thể buffer/giới hạn thời lượng WS → kiểm proxy read timeout của CF (mặc định 100s cho idle), gửi ping/keepalive; đo latency thật.
- **[RT-12] AudioWorklet 16k KHÔNG có code resample sẵn** — `use-audio-tap-probe.ts:82` chỉ set `sampleRate:16000` (chưa QA). Phải viết downsample thật + test trên browser không cấp 16k; đừng giả định "G6 đã chứng minh".
- **[RT-2] RAM:** peak batch+live thật ~2.9–3.2GB (spike) → arbiter đọc **cgroup `memory.current`/`memory.max`** (không `/proc/meminfo`) trước acquire; batch đang chạy → hoãn live. Ghi rõ trong UI.
- **[RT-3] utterance stuck:** gating cắt PCM giữa câu (`stream_session.py:156` chỉ đếm im lặng trên chunk NHẬN được) → thêm timeout flush utterance đang mở khi không có chunk mới; gating là code MỚI (desktop chỉ có design, không có để port).
