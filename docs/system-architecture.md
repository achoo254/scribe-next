# System Architecture — Realtime Meeting + Live Transcribe

> Kiến trúc module họp video realtime + live-transcribe tiếng Việt của OneScribe.
> Conferencing = LiveKit SFU (không tự viết); transcribe = hero (host desktop transcribe mọi track).

## Thành phần

```
┌─ Desktop (Tauri + Vue 3) ─ HOST ────────────────┐        ┌─ Web (Vite + Vue 3) ─ GUEST ─┐
│  livekit-client ─ camera/mic + subscribe        │        │  livekit-client              │
│  use-livekit-room · use-live-caption            │        │  useLiveKitViewer            │
│  AudioWorklet tap → invoke Rust                 │        │  useCaptionFeed (render)     │
│         │ Uint8Array PCM 16k                    │        │         ▲ data channel        │
│         ▼                                        │        │         │                     │
│  Rust MeetingTranscriber (tiến trình riêng)     │        └─────────┼─────────────────────┘
│    writer thread + bounded channel (drop-full)  │                  │
│         │ base64 PCM (line-JSON stdio)          │                  │
│         ▼                                        │                  │
│  sidecar `main.py --stream` (stream_server)     │                  │
│    router thread → per-track worker → PhoWhisper│                  │
│    small (vad-cut + max-cap 8s) → caption       │                  │
│         ▲ meeting_caption_msg                    │                  │
│  host publishData(caption, reliable) ───────────┼──► LiveKit SFU ──┘ relay data → mọi client
│  host POST /captions (host-token) ──────────────┼──► meeting-server caption store (TTL 1 ngày)
└──────────────────────────────────────────────────┘
                                                     ▲ token/history (HTTPS)
                    meeting-server (Node): /token/{host,guest} · /captions/:room · /webhook
                    LiveKit SFU (native) · coturn (TURN ephemeral) · Caddy/nginx (TLS/wss)
```

## Luồng dữ liệu

1. **Media:** camera/mic publish qua `livekit-client` → LiveKit SFU forward tới mọi participant (simulcast/adaptive). Media KHÔNG qua meeting-server; server KHÔNG lưu media.
2. **Transcribe (chỉ host):** host tap audio mọi track (AudioWorklet 16kHz mono) → `invoke` Rust (`meeting_transcribe_pcm`, Uint8Array) → `MeetingTranscriber` forward (base64, line-JSON) → sidecar `stream_server` → per-track `StreamSession` (vad-cut + max-cap ~8s, PhoWhisper-small int8) → caption.
3. **Phân phối caption:** host `publishData(caption JSON, reliable)` qua LiveKit data channel → mọi client render. Host cũng `POST /captions/:room` lên store (người vào trễ đọc lịch sử trong TTL).
4. **Late-joiner:** client join → `GET /captions/:room` (JWT hợp lệ) → seed lịch sử → nghe realtime tiếp.

## Cô lập tiến trình (chống chặn/deadlock)

- **Streaming = tiến trình sidecar RIÊNG** với batch Mode C (`transcribe`). Batch `BatchedInferencePipeline.transcribe()` blocking KHÔNG chặn caption vì ở process khác.
- Trong `stream_server`: luồng đọc stdin CHỈ định tuyến (không chạy ASR); mỗi track có worker thread + hàng đợi riêng; nhiều worker ghi stdout serialize bằng lock.
- Trong Rust `MeetingTranscriber`: writer thread sở hữu `ChildStdin`, nhận qua **bounded channel** — PCM dồn nhanh hơn sidecar đọc → **drop-on-full + backpressure**, KHÔNG block command Tauri, KHÔNG deadlock.
- Model `WhisperModel(small,int8)` nạp 1 lần, chia sẻ mọi track, transcribe serialize bằng lock (active-speaker gating giữ số track nói đồng thời ~≤2).

## Mô hình bảo mật

| Mối đe dọa | Kiểm soát |
|---|---|
| Client tự xin `role=host` | meeting-server **gán role server-side**: `/token/guest` hard-code guest; `/token/host` cần `HOST_SECRET` |
| Nhiều host/phòng | **host-lock** (1 host/phòng); request thứ 2 (identity khác) → `409`; nhả qua webhook `participant_left` hoặc TTL |
| Guest bơm caption giả | Token guest **`canPublishData=false`** (C7); client render caption **chỉ từ host** (identity LiveKit ký + role trong metadata, KHÔNG tin field payload tự khai) |
| Đọc/bơm trộm caption phòng khác | Caption store bắt buộc **JWT hợp lệ + room khớp** (C8); chỉ role=host được ghi; room id nên là **UUID opaque** |
| Lạm dụng TURN làm open proxy | **TURN credential ephemeral** (HMAC hết hạn ~5'), KHÔNG tĩnh |
| Path traversal qua track id | Sanitize `[A-Za-z0-9_-]{1,64}` ở cả Rust lẫn sidecar |
| Spam token endpoint | Rate-limit token-bucket per IP |

## Retention & privacy

- **Server KHÔNG lưu media** (video/audio ephemeral, chỉ relay). Xác nhận bằng cấu hình LiveKit (không bật egress/recording).
- **Chỉ lưu TEXT caption**, TTL **1 ngày** rồi purge (dung hòa "không lưu media": text KB-scale, không phải media).
- MVP dùng **caption store in-memory** (single-instance, local-first) → mất khi restart server. Interface store-agnostic → drop-in Redis (`RPUSH`+`EXPIRE`+`LRANGE`) khi cần bền qua restart / multi-instance.
- Recording client-side (`.txt`/`.srt`) = **Strategy A (caption-as-record)**, tùy chọn, phía client. Strategy B (ghi audio/MP4 + batch medium/large sau họp) = defer sau MVP.

## Ràng buộc nền tảng

- Meeting MVP **Windows-first** (WebView2). macOS (WKWebView + entitlements) defer sau MVP.
- Phần runtime phụ thuộc WebView2 (audio tap getUserMedia, quyền camera/mic) **chỉ verify được trên Windows** — xem checklist trong report Phase 3.
- Quy mô MVP **≤ 8 người/phòng**; host tối thiểu laptop văn phòng i5 (no GPU).

## Điểm nối mã nguồn

| Tầng | File |
|---|---|
| Sidecar streaming | `sidecar/transcriber/{stream_server,stream_session,model_paths}.py`, `main.py --stream` |
| Rust bridge | `src-tauri/src/meeting_audio.rs` (`meeting_transcribe_*`), `transcribe.rs::resolve` |
| Desktop FE | `src/composables/use-live-caption.ts`, `src/components/meeting/Caption{Panel,Overlay}.vue`, `src/views/MeetingRoomView.vue` |
| Shared | `packages/meeting-shared/src/*` (types, meeting-client, caption-feed, use-livekit-viewer) |
| Web client | `apps/web/src/*` |
| Server | `server/meeting-server/{index.mjs,lib/*}` |
| Infra | `infra/{coturn,caddy,systemd,livekit}/*`, `infra/run-local.sh` |
