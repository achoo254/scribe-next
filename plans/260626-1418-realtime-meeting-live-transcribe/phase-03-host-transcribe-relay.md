---
phase: 3
title: "Host Transcribe Relay"
status: in-progress
priority: P1
effort: "6-9d"
dependencies: [1, 2]
---

# Phase 3: Host Transcribe Relay

> **Tiến độ (cook, verified trên macOS):** Code DONE toàn tầng.
> - **Sidecar:** `stream_session.py` (vad-cut + max-cap 8s, port từ spike), `stream_server.py` (tiến trình riêng `--stream`: router thread + per-track worker + model registry — fix **C2**), `model_paths.py` (DRY batch/stream). **13 test pass** (session 6, server 6, integration 1) qua fake-model (không cần tải PhoWhisper).
> - **Rust:** `meeting_audio.rs::MeetingTranscriber` (writer thread + bounded channel drop-on-full + backpressure, reader demux → `meeting_caption_msg` giữ riêng `transcribe_msg` batch = **H4**, base64 tự cài, sanitize track id, EOF→`sidecar_crashed` = **H3**, mic revocation = mic_lost). `cargo check`✓ + 3 test pass.
> - **FE:** `use-live-caption.ts` (AudioWorklet tap → invoke, publish host-only, verify sender identity = **C7**), `CaptionPanel/Overlay.vue`, tích hợp MeetingRoomView + lưu transcript `.txt` (Strategy A). `vue-tsc`+`vite build`✓.
> **CHƯA verify (checklist Windows — WebView2 only):** AudioWorklet tap thật trong WebView2, invoke PCM round-trip runtime, E2E ≥2 client caption đúng người + latency, `tauri build` nsis, compile `#[cfg(windows)]`.

## Overview

Nối kết quả Phase 1 (streaming ASR) với Phase 2 (media): **host nhận audio mọi track qua LiveKit → đẩy vào sidecar streaming PhoWhisper (active-speaker gated) → publish caption (kèm tên người nói) qua LiveKit data channel** để mọi người (desktop + web) nhận và hiển thị realtime.

<!-- Updated: Validation Session 1 — chốt scope recording -->
**Scope recording (đã chốt validation):** MVP dùng **Strategy A — caption-as-record** (ghép caption final → lưu `.txt`/`.srt` phía client, KB-scale, là tùy chọn). **Strategy B (ghi audio/MP4 + batch transcribe medium/large sau họp) DEFER sau MVP** — không làm trong phase này. Quy mô MVP **≤ 8 người** → host transcribe tối đa ~8 track (gate active-speaker, ASR đồng thời ~1–2).

## Requirements

- **Functional:**
  - Host subscribe **audio track của tất cả participant**; route PCM xuống sidecar streaming.
  - Mỗi caption gắn **identity người nói** (track = participant → tên có sẵn, không cần diarization).
  - Publish caption (partial cập nhật + final chốt) qua LiveKit **data channel** (reliable cho final, lossy OK cho partial).
  - Desktop hiển thị caption overlay/panel realtime; lưu transcript ghép (tùy chọn, phía client).
- **Non-functional:** chỉ host transcribe; gate bằng active-speaker (tối đa N track ASR đồng thời theo spec Phase 1); host rớt → caption dừng sạch (không treo phòng).

## Architecture

```
HOST (desktop)
  LiveKit: TrackSubscribed(audio, participantX)
        │  WebView2 JS: Web Audio API tap remote audio track → PCM16k
        ▼  (qua Tauri command / IPC)
  Rust transcribe.rs: route PCM theo track-id → sidecar
        │  cmd mới: {"cmd":"stream_start","track":X} / {"cmd":"stream_pcm",...} / {"cmd":"stream_end"}
        ▼
  sidecar main.py (mở rộng streaming từ Phase 1): per-track ASR → {type:"caption",track,kind,text,t0,t1}
        ▲ emit lên JS
        │
  LiveKit publishData(caption JSON, reliable) ──► server relay ──► mọi participant
```

- **Mở rộng sidecar:** thêm chế độ streaming song song với batch `transcribe` cũ (KHÔNG phá Mode C). Quản nhiều stream theo `track` id, mỗi stream 1 state **vad-cut + max-utterance cap ~8s** (port từ spike Phase 1 `engine.VadCutStrategy`; KHÔNG LocalAgreement — đo NO-GO trên CPU). Cap số stream ASR đồng thời ≤2.
- **Audio tap:** ưu tiên Web Audio API (`MediaStreamTrackProcessor`/`AudioWorklet` trong WebView2) lấy PCM từ remote track → gửi xuống Rust qua Tauri command (hoặc xử lý thẳng trong JS nếu khả thi). Chốt đường đi cụ thể dựa trên kết quả spike Phase 1.
- **Active-speaker gating:** dùng `ActiveSpeakersChanged` của LiveKit + VAD (webrtcvad/built-in, KHÔNG silero) ở sidecar → chỉ ASR track đang nói; giới hạn số stream đồng thời theo spec Phase 1.

## Related Code Files

- Modify: `sidecar/transcriber/main.py` — thêm `stream_start`/`stream_pcm`/`stream_end` (tách module `stream_session.py` nếu >200 dòng).
- Create: `sidecar/transcriber/stream_session.py` — state per-track (port hóa từ spike Phase 1).
- Modify: `src-tauri/src/transcribe.rs` — API streaming (route PCM theo track, quản nhiều session); giữ nguyên path batch cũ.
- Create: `src/composables/use-live-caption.ts` — tap remote audio → gửi Rust; nhận event caption → publishData; subscribe data channel → render.
- Create: `src/components/meeting/CaptionPanel.vue` + `CaptionOverlay.vue` (hiển thị caption realtime kèm tên).
- Read for context: outputs Phase 1 (`stream_asr_spike.py`, báo cáo), `src/composables/use-livekit-room.ts` (Phase 2).

## Implementation Steps

1. Port code spike Phase 1 thành `stream_session.py` production-grade; thêm lệnh streaming vào `main.py` (song song batch, không phá `transcribe`).
2. Mở rộng `transcribe.rs`: nhận PCM theo track-id từ frontend, forward xuống sidecar; route caption output ngược lên qua event `transcribe_msg` (kèm field `track`).
3. `use-live-caption.ts` (host): với mỗi `TrackSubscribed` audio → tap PCM (Web Audio/AudioWorklet) → gửi Tauri command; nghe active-speaker để bật/tắt gate.
4. Host nhận event caption từ Rust → `room.localParticipant.publishData(JSON, reliable)` (final) / lossy (partial).
5. Mọi client (host + sau này web): subscribe `DataReceived` → parse caption → cập nhật store → `CaptionPanel`/`CaptionOverlay` render kèm tên người nói + timestamp.
6. Tùy chọn lưu transcript phía client: ghép caption final theo thời gian → `.txt`/`.srt` (tái dùng helper format từ `main.py`).
7. Xử lý host rớt: ngừng publish caption, các client hiển thị trạng thái "caption tạm dừng"; phòng media vẫn chạy (LiveKit độc lập).
8. Test: 3–4 người nói luân phiên + chồng, kiểm tra caption đúng người, latency, không kẹt khi 1 người rời giữa chừng.

## Success Criteria

- [ ] Host transcribe được audio của **mọi** participant (kể cả người chỉ có audio), caption gắn đúng tên người nói.
- [ ] Caption hiện realtime trên desktop với latency ≈ kết quả Phase 1 (≤4s).
- [ ] Active-speaker gating hoạt động: không ASR track im lặng; số stream đồng thời ≤ ngưỡng spec Phase 1.
- [ ] Host rời phòng → caption dừng sạch, media phòng không bị treo, client báo trạng thái rõ.
- [ ] Path batch Mode C cũ vẫn chạy nguyên (không regression).
- [ ] (Tùy chọn) lưu được transcript ghép phía client đúng thứ tự + tên.

## Risk Assessment

- **Tap PCM từ remote WebRTC track trong WebView2** là phần khó/chưa có tiền lệ trong stack. Giảm thiểu: validate đường đi này NGAY trong spike Phase 1 (không để tới đây mới biết).
- **Throughput PCM JS→Rust→Python** có thể nghẽn IPC. Giảm thiểu: gửi PCM nén/giảm sample, chunk hợp lý; cân nhắc xử lý audio thẳng trong sidecar nếu Rust có thể nhận RTP.
- **Audio remote đã qua Opus + mất gói** → chất lượng kém hơn mic gốc. Chấp nhận cho live; biên bản chuẩn dùng batch sau họp.
- **Quá tải host khi nhiều người nói** → áp ngưỡng active-speaker + degradation (theo Phase 1). Hiển thị cảnh báo "máy host quá tải" thay vì treo.

## Red Team Hardening (Session 1)

- **[C2] Streaming ASR PHẢI chạy process/thread riêng, không qua vòng đơn luồng của `main.py`.** `main.py:125` là `for line in sys.stdin` đơn luồng; `BatchedInferencePipeline.transcribe()` blocking → chạy batch Mode C là **chặn đứng streaming**. Và `transcribe.rs:183-188` `send()` giữ `Mutex<Option<Proc>>` + `write_all()` blocking → khi PCM dồn nhanh hơn sidecar đọc (pipe Windows ~64KB đầy trong ~1s ở 2 track), `write_all` block giữ Mutex → **deadlock toàn app**. Fix: thread nền sở hữu ChildStdin + **bounded channel (drop-on-full + emit `backpressure` warning)**; tách session streaming khỏi batch.
- **[Model registry] KHÔNG dùng chung global `_model`.** `main.py:28-29,73-82` reload huỷ model cũ khi đổi size → streaming(small) + batch(medium) giẫm nhau. Dùng dict `{"stream": WhisperModel_small, "batch": BatchedInferencePipeline_medium}`, cap 2 model, không reload huỷ.
- **[C7] Chống caption injection.** Tắt `canPublishData` cho guest trong JWT grant — **chỉ host được publish data**. Client render caption CHỈ khi `DataReceived.participant.identity === host-identity` (LiveKit cấp, sender KHÔNG spoof được) — KHÔNG tin field `track`/tên do payload tự khai. Defense-in-depth: host ký HMAC caption bằng room-secret.
- **[H4] Tách event channel.** Reader thread `transcribe.rs:166-177` hiện emit mọi dòng qua `transcribe_msg` → đụng `use-transcribe.ts:103-115` (case "done" không guard `!m.file` → batch counter sai). Emit 2 kênh riêng: `transcribe_batch_msg` (Mode C) vs `meeting_caption_msg` (streaming), demux theo `v["type"]` ở Rust.
- **[H3] Sidecar crash giữa họp.** `send()` không gọi `ensure_spawned()`; reader thread chết im khi stdout EOF. Fix: sau vòng `for` reader thread emit `{"type":"sidecar_crashed"}`; `use-live-caption.ts` bắt → dừng đẩy PCM, `ensure_spawned()` + gửi lại `stream_start` cho track active, hiện overlay "đang khởi động lại transcribe".
- **[Path traversal] Sanitize ở Rust.** `track` id chỉ `[a-zA-Z0-9_-]`; mọi path (`model_dir`/`out_dir`) canonicalize + verify nằm trong `app_data_dir()` trước khi gửi sidecar (`main.py:63,106-113` mkdir/write_text không kiểm).
- **[Mic revocation] Theo dõi `track.onmute/onunmute`** trên mic host (Windows 11/IT policy thu hồi quyền giữa chừng → track muted im lặng, VAD thấy lặng → caption ngừng không báo). Khi mute → publishData `{type:"host_mic_lost"}` để mọi người thấy "transcribe tạm dừng".

### Success Criteria bổ sung
- [ ] Chạy batch Mode C trong lúc đang stream meeting KHÔNG làm caption đứng (process tách).
- [ ] Đẩy PCM nhanh hơn sidecar xử lý KHÔNG deadlock app (bounded channel + drop).
- [ ] Guest KHÔNG publish được caption giả (verify bằng identity LiveKit).
- [ ] Kill sidecar giữa họp → UI báo + tự khởi động lại, không treo im.
