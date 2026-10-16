---
phase: 4
title: "Web Client Vue"
status: in-progress
priority: P2
effort: "5-8d"
dependencies: [2, 3]
---

# Phase 4: Web Client Vue

> **Tiến độ (cook, verified trên macOS):** Code DONE theo phương án **sibling** (user chốt: KHÔNG monorepo restructure src-tauri ngay — tránh rủi ro H6 không verify được `tauri build` trên mac).
> - `packages/meeting-shared/` (consume qua vite alias `@scribe/meeting-shared`): `types.ts`, `meeting-client.ts` (token/history/pushCaption), `caption-feed.ts` (**C7** guard host sender), `use-livekit-viewer.ts` (TURN qua rtcConfig). Desktop cũng dùng chung (đã bỏ duplicate `use-caption-feed.ts`).
> - `apps/web/` (Vite + Vue 3, plain + token brand CSS — KHÔNG kéo `@hobui/ui` private registry để build được trên mac): `MeetingRoomPage.vue` (join guest qua token-service → grid + caption + fetch history), `VideoTile.vue`. Guest-only (không host/transcribe). `vue-tsc`+`vite build`✓ (581KB).
> **Defer (không đổi quyết định plan, chỉ hoãn thực thi):** monorepo workspaces đầy đủ (dời src-tauri → apps/desktop) + dùng `@hobui/ui` npm cho web — làm khi verify được `tauri build` trên Windows.
> **CHƯA verify:** getUserMedia thật trên Chrome/Edge/Safari + mobile; E2E host desktop + ≥2 web guest.

## Overview

Xây **web client riêng (Vue 3 + Vuetify + design system)** cho khách join bằng trình duyệt — consumer thuần: **xem video + đọc caption realtime**, không transcribe. Thống nhất 1 FE framework (Vue) với desktop, **tái dùng tối đa** design system + composable + component meeting từ Phase 2/3.

## Requirements

- **Functional:** mở link phòng → nhập tên → bật camera/mic (publish) → thấy/nghe mọi người → đọc caption realtime kèm tên người nói. KHÔNG có nút host/transcribe.
- **Non-functional:** chạy trên Chrome/Edge/Safari hiện đại; UI design system đồng nhất thương hiệu; responsive (desktop + mobile browser); web client KHÔNG bao giờ được là host (chặn cả ở UI lẫn token).

## Architecture

```
Monorepo / shared layer (Vue)
  packages/meeting-shared:  use-livekit-room.ts, use-live-caption.ts (phần subscribe/render),
                            ParticipantTile.vue, MeetingControls.vue, CaptionPanel.vue
        ├── apps/desktop (Tauri)  → host + transcribe (Phase 2/3)
        └── apps/web (Vite)       → consumer thuần (phase này)
```

- **Tái dùng code:** tách phần dùng chung (kết nối LiveKit, render tile/caption) ra layer chia sẻ; web chỉ bỏ phần host-transcribe (vốn phụ thuộc Tauri/sidecar — không có trên web).
<!-- Updated: Validation Session 1 — khóa monorepo workspaces -->
- **Cấu trúc (ĐÃ CHỐT validation): monorepo workspaces** — `apps/desktop` (Tauri) + `apps/web` (Vite) + `packages/meeting-shared`. DRY, một nguồn component. Chấp nhận chi phí refactor cấu hình Tauri/Vite hiện tại sang workspaces (đo + xử lý trong bước 1). Phương án "web app riêng" đã loại.
- **Design system trên web:** project ngoài repo design system → cài npm `@hobui/ui @hobui/tokens vuetify @tabler/icons-vue`, import `{ Ui* } from '@hobui/ui'` + `@hobui/ui/style.css` (theo hướng dẫn MCP design system).
- **Caption phía web:** chỉ subscribe data channel + render (logic transcribe nằm ở host) → web nhẹ, không cần model.

## Related Code Files

- Create: `apps/web/` (Vite + Vue 3 + design system) — `MeetingRoomPage.vue`, router, entry, `vite.config.ts`.
- Create/Refactor: `packages/meeting-shared/` — di chuyển `use-livekit-room.ts`, `ParticipantTile.vue`, `MeetingControls.vue`, `CaptionPanel.vue` ra dùng chung.
- Modify: `package.json` gốc (khai báo workspaces); `apps/desktop` (app Tauri di chuyển vào) trỏ vào `packages/meeting-shared`.
- Create: endpoint cấp token cho web guest (role = consumer, canPublish video/audio, KHÔNG host) — phối hợp Phase 5.
- Read for context: outputs Phase 2/3, hướng dẫn design system (resource `meta/instructions` của MCP design system).

## Implementation Steps

1. Refactor sang **monorepo workspaces** (đã chốt): di chuyển app Tauri hiện tại vào `apps/desktop`, dựng `packages/meeting-shared`, cấu hình workspaces ở `package.json` gốc. Xác minh `npm run tauri dev` vẫn chạy sau refactor.
2. Scaffold `apps/web` (Vite + Vue 3), cài design system qua npm, set theme scribeDark giống desktop.
3. Tái dùng (hoặc port) `use-livekit-room.ts` + tile/controls; loại bỏ mọi nhánh host-transcribe khỏi web build.
4. `MeetingRoomPage.vue`: nhập tên → lấy token guest → connect → publish camera/mic → grid + `CaptionPanel` (subscribe data channel, render caption + tên).
5. Chặn host trên web: token guest không có quyền/cờ host; UI ẩn nút host/transcribe.
6. Responsive + mobile browser: kiểm tra layout, getUserMedia trên Safari/iOS, autoplay policy (cần user gesture).
7. Test xuyên client: host (desktop) + 2 khách web cùng phòng → mọi người thấy video + đọc caption realtime; khách web nói → host transcribe được (xác nhận luồng Phase 3).

## Success Criteria

- [ ] Khách mở link trên Chrome/Edge/Safari → join phòng, publish camera/mic, thấy/nghe mọi người.
- [ ] Caption realtime hiển thị trên web kèm đúng tên người nói (gồm cả giọng khách web — host transcribe hộ).
- [ ] Web client KHÔNG thể trở thành host (chặn ở token + UI).
- [ ] FE web dùng cùng design system/theme với desktop (đồng nhất thương hiệu); ESLint design system pass.
- [ ] Code chung desktop/web không bị fork trùng lặp (đạt DRY ở mức đã quyết định ở bước 1).
- [ ] Hoạt động trên mobile browser (responsive + getUserMedia).

## Risk Assessment

- **Refactor desktop sang monorepo** có thể đụng cấu hình Tauri/Vite hiện tại (đường dẫn `src-tauri`, alias, sidecar path trong `transcribe.rs::find_script`). Giảm thiểu: refactor từng bước + chạy `tauri dev` xác minh sau mỗi bước; **tag git checkpoint TRƯỚC refactor + commit nhỏ atomic** để `git revert`/reset về tag khi hỏng (workflow đã chốt Session 2: code+push thẳng `main`, KHÔNG nhánh/PR).
- **Safari/iOS WebRTC + autoplay** khắt khe (cần user gesture, codec). Giảm thiểu: test sớm trên thiết bị thật; nút "Tham gia" làm gesture mở media.
- **Design system npm version lệch** giữa desktop (alias nội bộ) và web (npm) → UI lệch. Giảm thiểu: khóa version `@hobui/ui`/`@hobui/tokens` thống nhất.
- **getUserMedia cần HTTPS** trên web (trừ localhost). Giảm thiểu: phối hợp Phase 5 (TLS/domain).

## Red Team Hardening (Session 1)

Monorepo refactor (user giữ trong MVP) đụng NHIỀU đường dẫn cứng — checklist bắt buộc bước 1, verify end-to-end TRƯỚC khi sang bước 2:

- **[H6] Đường dẫn gãy khi dời `src-tauri` vào `apps/desktop/`:**
  - `tauri.conf.json:10` `frontendDist: "../dist"` → cập nhật theo vị trí mới + đặt `build.outDir` rõ trong `vite.config.ts`; nếu sai, installer ship **màn hình trắng KHÔNG báo lỗi** (frontendDist chỉ check lúc bundle).
  - `src-tauri/src/transcribe.rs:108-117` `find_script()` walk ancestor tìm `sidecar/transcriber/main.py` — verify `sidecar/` vẫn là ancestor của exe sau khi dời; chạy `tauri dev` xác nhận.
  - `sidecar/transcriber/build_pyinstaller.ps1:24` `$binDir = "..\..\src-tauri\binaries"` → **sửa thành đường dẫn mới** (`..\..\apps\desktop\src-tauri\binaries`) nếu không **release build copy binary sai chỗ**, `externalBin` (`tauri.conf.json:30`) không thấy.
  - Cập nhật `package.json` gốc thêm `workspaces`; `Cargo.toml` workspace; đường dẫn trong 3 file `capabilities/*.json`.
  - **Verify cả `npm run tauri dev` LẪN `npm run tauri build` (release) end-to-end ngay sau refactor.** Trên `main` (không nhánh): **`git tag pre-monorepo-refactor` TRƯỚC khi dời file** → hỏng thì `git reset --hard` về tag.
- **[H5] `UiRow`/`UiCol` mismatch:** ui local (16 component) thiếu Row/Col mà `@hobui/ui` npm có → shared `packages/meeting-shared` dùng Row/Col sẽ gãy desktop build. Chốt: monorepo này nên cho desktop **cũng import `@hobui/ui` npm** (đồng bộ component set với web), bỏ alias local — giải quyết luôn H5.
- **[.npmrc] Chống typosquat + pin:** thêm `.npmrc` `@hobui:registry=https://<private-registry>`; pin chính xác `@hobui/ui@<ver>` (package mới, mới 2 version) ở cả apps/web + apps/desktop; review peerDep `apexcharts` (kéo ~300KB không cần cho meeting) — chỉ cài nếu dùng.

### Success Criteria bổ sung
- [ ] Sau monorepo refactor: `tauri dev` + `tauri build` (release, có sidecar binary đúng chỗ) đều chạy — không màn trắng, không thiếu binary.
- [ ] Desktop + web dùng cùng nguồn `@hobui/ui` (component set khớp), `.npmrc` private registry + version pin.
