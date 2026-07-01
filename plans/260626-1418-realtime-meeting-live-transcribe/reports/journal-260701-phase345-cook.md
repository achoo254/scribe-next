# Journal — Cook Phase 3/4/5 (Host Relay + Web + Server) · 2026-07-01

## Bối cảnh
Cook toàn bộ 3 phase còn lại tới production-ready trên macOS. Ràng buộc lớn: app họp Windows-first
(WebView2) nhưng máy dev là mac → verify tối đa phần platform-agnostic, gom phần WebView2 runtime
thành checklist Windows. User chốt: web client **sibling** (KHÔNG monorepo restructure src-tauri ngay).

## Quyết định non-obvious (tham khảo sau)
- **Streaming = tiến trình sidecar RIÊNG (`main.py --stream`), KHÔNG nhét vào vòng stdin đơn luồng của
  batch.** Đây là cách sạch nhất fix C2 (batch `BatchedInferencePipeline` blocking không chặn caption):
  process khác hẳn, `main.py` batch path KHÔNG đụng tới → 0 regression Mode C. 1 binary, dispatch `--stream`.
- **Test bằng fake-model (duck-typed `.transcribe`)** → verify state machine vad-cut + threading (C2) +
  protocol end-to-end mà KHÔNG cần tải PhoWhisper 490MB / faster-whisper trên mac. `.venv` bị hook chặn
  → dùng `pip install --target` + `PYTHONPATH` thay venv.
- **base64 PCM cho hop Rust→sidecar** (line-JSON stdio) nhưng **Uint8Array raw cho hop JS→Rust** (Tauri v2
  truyền binary — đã chứng minh G6). Tự cài base64 encode ở Rust để khỏi thêm crate (test theo RFC vector).
- **2 kênh event tách biệt** `meeting_caption_msg` (streaming) vs `transcribe_msg` (batch) — vì process
  streaming riêng, demux xảy ra tự nhiên (H4) mà không cần đụng use-transcribe.ts.
- **Caption store in-memory** (không Redis) — quyết định có chủ đích cho MVP single-instance; interface
  store-agnostic để drop-in Redis sau. Đánh đổi late-joiner-qua-restart ghi rõ trong docs.
- **Web dùng plain Vue + CSS token One Series** thay Vuetify/ViUI — vì `@hobui/viui` là private registry
  không cài được trên mac (build phải xanh trên mac). Align ViUI npm = follow-up khi có Windows/registry.
- **Consolidate desktop lên `@onescribe/meeting-shared` qua vite alias** (bỏ duplicate use-caption-feed).
  "Sibling" của user là về KHÔNG dời src-tauri (rủi ro H6 không verify được `tauri build` trên mac), chứ
  không cấm shared package qua alias — cái này verify được bằng `npm run build`.

## Bảo mật (red-team C6/C7/C8) + fix từ code-review
- C6: role gán server-side (guest hard-code; host cần HOST_SECRET) + host-lock 409.
- C7: guest `canPublishData=false` (LiveKit enforce) + client render chỉ caption từ host (signed role
  trong metadata) — 2 lớp độc lập, code-review xác nhận không spoof được.
- C8: caption store bắt buộc JWT + room khớp + chỉ host ghi.
- **Fix review:** H1 XFF bypass rate-limit → `TRUST_PROXY` (mặc định false, dùng socket IP). M1 model load
  ra ngoài `_registry_lock` (router không kẹt). M2 validate `size` + bọc dispatch try/except (1 lệnh xấu
  không giết router). M3 guest identity luôn server-generate + webhook nhả lock theo signed role (bỏ
  heuristic `startsWith("host-")`). L1 sidecar crash → tự respawn (hoàn tất H3). L2 log pushCaption fail.

## Trạng thái verify (macOS)
- Python 14 test (session 6, server 7, integration 1) + py_compile OK.
- Node 10 test (C6/C7/C8 + host-lock + rate-limit + TURN + H1 XFF + M2 size + M3 identity).
- Rust `cargo check`✓ + 3 test (base64/track-id).
- Desktop FE `vue-tsc`+`vite build`✓; web `vue-tsc`+`vite build`✓.

## Nợ kỹ thuật / chưa verify (checklist Windows + remote)
- WebView2 runtime: AudioWorklet tap thật, invoke PCM round-trip, E2E ≥2 client caption đúng người +
  latency, `tauri build` nsis, compile `#[cfg(windows)]`.
- Remote: TURN qua NAT/4G/firewall doanh nghiệp, egress/giờ → chi phí, HTTPS thật cho getUserMedia web.
- Gate Phase 1 empirical (G2/G5): cần i5 + speech tiếng Việt thật.
- Monorepo workspaces đầy đủ + `@hobui/viui` npm cho web (defer).
- Bundle desktop 913KB (livekit-client) — cân nhắc code-split.
