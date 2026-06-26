---
phase: 2
title: "LiveKit Media Core"
status: pending
priority: P1
effort: "5-8d"
dependencies: []
---

# Phase 2: LiveKit Media Core

## Overview

Dựng **LiveKit SFU self-host** + tích hợp `livekit-client` (JS) vào app Tauri (Vue 3 + ViUI) để có **phòng họp video/audio cơ bản**: tạo/join phòng, bật camera/mic, thấy/nghe nhau, grid layout. Validate WebRTC chạy ổn trong WebView2.

## Requirements

- **Functional:** desktop user tạo phòng (host) hoặc join; publish camera + mic; subscribe video/audio người khác; grid view kèm tên + trạng thái mic/cam; nút mute/cam-off/leave.
- **Non-functional:** chạy mượt **tới 8 người** trong WebView2 (quy mô MVP đã chốt); simulcast bật (tự hạ chất lượng khi mạng yếu); UI dùng **chỉ Vi*** (ViUI), token-only, theme inetOneDark.

## Architecture

```
LiveKit server (binary native trên WSL Ubuntu, KHÔNG Docker)
  - SFU forward video/audio
  - data channel (dùng cho caption ở Phase 3)
  - token-based room access (JWT)
        ▲ WebRTC (WebView2 = Chromium)
        │
Desktop app (Tauri + Vue 3)
  - livekit-client JS SDK
  - MeetingRoomView.vue (grid), ParticipantTile.vue, MeetingControls.vue
  - composable useLiveKitRoom() — connect/publish/subscribe/track events
```

- **Token:** giai đoạn này dev-token tạm (script/endpoint dev); auth thật ở Phase 5.
- **Vue tích hợp:** dùng `livekit-client` thuần (KHÔNG `@livekit/components-react`) → giữ FE thống nhất Vue. Bọc logic vào composable `useLiveKitRoom`.

## Related Code Files

- Create: `infra/livekit/livekit.yaml` + script chạy `livekit-server` (native binary, KHÔNG Docker).
- Create: `src/views/MeetingRoomView.vue`, `src/components/meeting/ParticipantTile.vue`, `src/components/meeting/MeetingControls.vue`.
- Create: `src/composables/use-livekit-room.ts` (connect, publish camera/mic, theo dõi track/participant events).
- Modify: `src/App.vue` (thêm tab/route "Họp"), `package.json` (thêm `livekit-client`).
- Read for context: `src/design-system/vi-ui/` (Vi* có sẵn), `src/plugins/vuetify.ts` (theme), `README.md` (ràng buộc ViUI).

## Implementation Steps

1. Cài **LiveKit native — KHÔNG Docker** trên WSL Ubuntu: `curl -sSL https://get.livekit.io | bash` → chạy nhanh bằng `livekit-server --dev` (devkey/secret), hoặc `livekit-server --config livekit.yaml --bind 0.0.0.0` (cấu hình API key/secret + WebRTC port range). App Tauri (Windows) kết nối tới LiveKit trong WSL qua `localhost` (WSL2 forward port sang Windows) — kiểm port WebRTC/UDP có cần `wsl.conf`/portproxy không. Tài liệu hóa lệnh chạy trong `docs/deployment-guide.md`.
2. Cài `livekit-client`; viết `use-livekit-room.ts`: `connect(url, token)`, `enableCameraAndMic()`, handler `ParticipantConnected/Disconnected`, `TrackSubscribed/Unsubscribed`, `ActiveSpeakersChanged`.
3. `ParticipantTile.vue`: gắn `<video>`/`<audio>` element vào track; hiển thị tên, viền active-speaker, badge mic/cam off (icon Tabler, token màu ViUI).
4. `MeetingRoomView.vue`: grid responsive (Vi* layout), nút điều khiển (`MeetingControls.vue`): mute, cam off, leave.
5. Bật **simulcast** khi publish video; kiểm tra adaptive bitrate khi bóp băng thông (DevTools throttle).
6. Tạo dev-token tạm để test (script Node hoặc lệnh `livekit-cli`); thêm form nhập room name + nút "Tạo phòng / Join".
7. Test tới 8 tab/máy: vào/ra phòng, mute, tắt cam, kiểm tra không leak track/`<video>` khi participant rời.

## Success Criteria

- [ ] Tới 8 người vào 1 phòng, thấy + nghe nhau trong app Tauri, không giật ở mạng LAN.
- [ ] Mute/cam-off/leave hoạt động; tile cập nhật đúng khi người khác vào/ra.
- [ ] Simulcast bật, video tự hạ chất lượng khi throttle mạng (không freeze/đứng hình).
- [ ] UI chỉ dùng Vi* + token (ESLint `vi-ui/no-raw-vuetify` pass), không hardcode màu/spacing.
- [ ] Không rò rỉ MediaStream/`<video>` element sau khi participant disconnect.

## Risk Assessment

- **WebView2 quyền camera/mic** — Tauri cần cấu hình permission (capabilities) cho getUserMedia. Giảm thiểu: kiểm tra sớm bước 2, cấu hình `capabilities/`.
- **WebRTC port/NAT khi test máy thật** — LAN OK, qua internet cần TURN (để Phase 5). Giảm thiểu: Phase 2 chỉ cần LAN/localhost.
- **WSL2 ↔ Windows UDP forwarding** — LiveKit chạy trong WSL Ubuntu, app Tauri ở Windows; WebRTC dùng **UDP** mà NAT của WSL2 hay vướng với dải port UDP. Giảm thiểu: cấu hình LiveKit dùng dải port hẹp + `netsh portproxy`/`wsl.conf` nếu cần, hoặc dùng TCP/TURN local cho dev; xác minh sớm ở bước 1.
- **Đồng bộ vòng đời track ↔ Vue reactivity** — dễ leak `<video>`. Giảm thiểu: cleanup trong `onUnmounted` + handler Unsubscribed.
- **ViUI có thể thiếu component cho grid video** — dùng Vi* layout + element thuần `<video>` (không phải UI control) là chấp nhận được.

## Red Team Hardening (Session 1)

- **[H1] Set CSP TRƯỚC khi nhúng bất kỳ JS ngoài (livekit-client).** `tauri.conf.json:24` hiện `"csp": null` (tắt hẳn) → XSS qua tên participant/caption có thể gọi `window.__TAURI__.invoke('transcribe_files', {paths:[...]})` (`commands.rs:146-153` nhận `paths: Vec<String>` không whitelist) → sidecar mở file/UNC path tùy ý. Đặt CSP tối thiểu: `default-src 'self'; connect-src 'self' wss://* https://*; media-src 'self' blob:`. Render caption/tên bằng **template interpolation Vue** (KHÔNG `v-html`).
- **[H2] Cấu hình quyền camera/mic — KHÔNG để getUserMedia fail im lặng.** `capabilities/default.json` hiện KHÔNG có media permission; WebView2 (Windows) cần Rust hook `PermissionRequested` (qua `WebviewWindowBuilder`/plugin) để grant `mediaStream`. **macOS dùng WKWebView** (không phải WebView2) → cần `NSCameraUsageDescription` + `NSMicrophoneUsageDescription` trong `bundle.macOS` + entitlements; hiện `tauri.conf.json` chỉ target `nsis` (Windows) → **macOS chưa có pipeline, đánh dấu out-of-scope MVP hoặc bổ sung**. `getUserMedia({video,audio})` thành công là **acceptance criterion bắt buộc** của bước 2, không phải "kiểm tra sớm".
- **[H5] `ViRow`/`ViCol` KHÔNG có trong `src/design-system/vi-ui/`** (16 component, `index.ts` không export Row/Col) nhưng grid cần. Chốt 1 trong 2 TRƯỚC khi dựng grid: (a) thêm wrapper `ViRow`/`ViCol` vào vi-ui local; (b) chuyển desktop import `@hobui/viui` npm (đồng bộ với web, gắn với monorepo Phase 4). Nếu dùng raw `v-row` → ESLint `vi-ui/no-raw-vuetify` reject.

### Success Criteria bổ sung
- [ ] `getUserMedia` cấp quyền OK trong WebView2 (Windows) — test thật, không giả định.
- [ ] CSP đã set (khác null) trước khi tích hợp livekit-client; không `v-html` cho caption/tên.
- [ ] Grid dùng Vi* hợp lệ (ViRow/ViCol đã có nguồn), ESLint ViUI pass.
