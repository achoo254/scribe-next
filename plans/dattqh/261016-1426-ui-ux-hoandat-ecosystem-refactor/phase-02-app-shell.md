---
phase: 2
title: "Khung app: top bar, account menu, footer, layout"
status: pending
priority: P1
effort: "M"
dependencies: [1]
---

# Phase 2: Khung app

## Files (`web/src/layout/`, chép từ `localman-next/apps/web/src/components/layout/`)
- `app-header.tsx` theo `localman-next/.../layout/app-header.tsx`: sticky, `min-h-(--topbar-h)`, `bg-bg/82 backdrop-blur-md`, wordmark → trang chủ (`/record`), nav ghost Ghi âm / Cắt file / Transcript (ẩn Transcript khi `!canTranscribe`), bên phải account menu. Bỏ phần Localman không có ở Scribe: feature flags (`config/features`), chuông thông báo, workspace switcher, docs, plan, `tenant-info` (gọi `/api/v1/tenant` — Scribe không có), `use-logout` của Localman (viết hook dùng `auth-context.tsx` hiện có). Chép `skip-link.tsx`.
- `account-menu.tsx`, `account-controls.tsx`, `idp-account-items.tsx`, `user-avatar.tsx`: avatar + email; mục "Tài khoản hoandat" (`<idp>/account`) và "Ứng dụng" (`<idp>/account#account-apps`, như Localman), mở tab mới (iPhone: mở trình duyệt hệ thống). `<idp>` là hằng lúc build (`VITE_IDP_ORIGIN`, mặc định `https://auth.hoandat.com`, khai một chỗ trong `vite.config.ts`/`.env.example`) — không gọi `/api/auth/native/config` (không trả issuer, mỗi lần gọi chạy discovery).
- "Đăng xuất" trong menu: giữ luồng `endSessionUrl` hiện có (web) và plugin `HoandatAuth` (iPhone). **Hỏi xác nhận** khi đang ghi hoặc còn đoạn chờ tải (native dừng phiên ghi khi đăng xuất, `RecorderPlugin.swift:194-197`; web phải dừng mic trước khi xoá phiên).
- `app-footer.tsx`: theme toggle (chuyển từ top bar), link Tài khoản/Ứng dụng; không form feedback (Scribe không có backend feedback), không language switcher (một ngôn ngữ).
- `app-layout.tsx`/`app-shell.tsx`: skip link, `<main>` container `max-w-page` (1120 px) như Localman; quota bar giữ vị trí dưới top bar. **Không** route-progress (cần data router, Phase 3).
- iPhone (`isNativeShell()`): top bar gọn (wordmark + account menu), nav thành hàng segmented nhỏ dưới top bar ở `max-sm`, `env(safe-area-inset-*)` cho top bar/footer.
- Tạm thời nav vẫn đổi tab trong trang (URL ở Phase 3) để phase deploy được độc lập.

## Verification
- vitest cho header/menu/footer (chép test Localman tương ứng, sửa cho Scribe) + xác nhận đăng xuất khi đang ghi; Playwright ảnh chụp top bar/menu/footer light/dark, desktop + WebKit iPhone với stub Capacitor (nhánh native); axe.
- Deploy web prod (§6, điều kiện deploy của plan), kiểm `/`, đăng nhập, đăng xuất từ menu. App iPhone đang cài không đổi tới Phase 3.
