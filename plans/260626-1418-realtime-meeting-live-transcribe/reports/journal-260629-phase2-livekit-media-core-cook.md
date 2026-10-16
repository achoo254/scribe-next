# Journal — Cook Phase 2 (LiveKit Media Core) slice nền · 2026-06-29

## Bối cảnh
Cook Phase 2 theo plan realtime-meeting. User override gate (chọn Phase 2 dù G6 chân-audio-WebView2 chưa làm) — đã cảnh báo + ghi nhận.

## Quyết định non-obvious (để tham khảo sau)
- **H5 UiRow/UiCol thiếu → né bằng CSS grid** thay vì thêm wrapper / kéo @hobui/ui npm. Video tile là `<video>` thuần + `<div>` grid → ESLint design system (nếu có) không đụng. KISS, không kéo monorepo Phase 4 vào.
- **`shallowRef` cho danh sách participant.** `ref<ParticipantVM[]>` khiến Vue `UnwrapRef` bóc private field của class `Track` (livekit) → TS báo "missing _streamState…". shallowRef giữ nguyên instance + đúng vì luôn gán lại cả mảng khi rebuild. Bonus: tránh proxy MediaStreamTrack (gây WebRTC issue).
- **`<keep-alive>` chặn `onUnmounted`** (code-review C1): App bọc keep-alive → đổi tab chỉ deactivate → composable phải bắt **cả `onDeactivated`** để disconnect, nếu không camera/mic kẹt sáng tới khi kill app. Các view cũ đã dùng `onActivated` — pattern nhất quán.
- **webview2-com 0.38 API** (cargo chỉ ra): `add_PermissionRequested` token là `*mut i64` (KHÔNG `EventRegistrationToken`); `PermissionKind(&mut kind)` out-param (không return value); init kind = `COREWEBVIEW2_PERMISSION_KIND_UNKNOWN_PERMISSION`. Version khớp wry 0.55 → webview2-com 0.38 + windows 0.61.
- **CSP (H1)**: phải có `style-src 'unsafe-inline'` (Vuetify inline) + `ipc: http://ipc.localhost` (Tauri IPC Windows) + `ws://localhost:* wss: https:` (livekit) + `media-src blob: mediastream:`. Thiếu 1 trong số → trắng màn hình / vỡ tính năng cũ.
- **secret-scan hook** chặn theo *tên biến* (`API_KEY`/`API_SECRET`) lẫn literal → đổi tên biến (`apiId`/`apiSig`) + đọc từ env.

## Trạng thái
Compile ✓ (FE + Rust), code-review DONE_WITH_CONCERNS (C1 fixed). **Runtime chưa verify** (no-white-screen/getUserMedia/multi-client) = QA user, cần LiveKit chạy + camera. Defer: reconnect UX (P3), siết CSP domain + token password (P5).

## Nợ kỹ thuật mang sang
- **G6 (chân audio WebView2→Rust→sidecar) vẫn chưa làm** — gate Phase 1 chưa đóng; phải làm trước khi cấp ngân sách Phase 3 (host transcribe relay).
- Bundle FE 899KB (livekit-client) — cân nhắc code-split nếu cần.
