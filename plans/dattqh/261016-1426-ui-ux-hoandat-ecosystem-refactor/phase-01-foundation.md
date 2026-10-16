---
phase: 1
title: "Nền: sync primitive, i18n, Playwright"
status: pending
priority: P1
effort: "M"
dependencies: []
---

# Phase 1: Nền

## Files
- Chép từ `localman-next/apps/web/src/components/ui/`: `dropdown-menu.tsx`, `menu-styles.ts`, `select.tsx`, `textarea.tsx` (chỉ cái Phase 2–4 dùng; `route-progress.tsx` để Phase 3 vì cần data router). Thêm `@radix-ui/react-dropdown-menu` cùng phiên bản Localman.
- Đồng bộ `modal.tsx`, `toast.tsx`, `tabs.tsx` với Localman (2–9 dòng lệch): giữ phần Scribe cần, ghi lý do.
- i18n: chép `localman-next/apps/web/src/i18n/{i18n-context.ts,i18n-provider.tsx,load-messages.ts,translate.ts,message-types.ts}` (kèm tệp chúng import; thay `@localman/shared/constants` bằng hằng của Scribe). Localman lấy kiểu khoá từ từ điển `en`; Scribe lấy từ `vi` (từ điển duy nhất). Test từ điển viết cho Scribe: mọi khoá dùng trong code có trong `vi`, không chuỗi rỗng, không khoá thừa. Không language switcher khi một ngôn ngữ. `useT()` thay chuỗi viết cứng dần ở Phase 2–5.
- `use-delayed-flag`, `PanelProgress`, `SkeletonRegion` đã có ở Scribe — không chép.
- Playwright (cách làm mới của Scribe; Localman chạy cả stack, không ảnh mốc): `web/e2e/` (`playwright.config.ts`, `support/` chặn `/api/*` bằng `page.route` với dữ liệu mẫu đúng shape type ở `src/lib/api.ts`/`src/auth/auth-api.ts`), Chromium desktop + WebKit viewport iPhone 15, light/dark, `@axe-core/playwright`, font tự host + `reducedMotion: "reduce"` để ảnh ổn định. **Stub Capacitor** (`window.Capacitor.isNativePlatform() = true` + plugin giả `Recorder`/`HoandatAuth`/`NativeFiles`) để render nhánh native (thẻ ghi native, đăng xuất native, safe area) — chỉ trong `e2e/`. Script `npm run test:e2e`. Spec đầu: đăng nhập + trang ghi hiện tại (ảnh gốc để so sau).

## Verification
- lint, typecheck, check:tokens, vitest, `test:e2e` đạt; `dictionaries.test` đạt với một khoá mẫu.
- Không đổi giao diện người dùng thấy ⟹ không deploy phase này (chỉ commit).

## Rủi ro
- Primitive Localman phụ thuộc `cn`, `menu-styles`, i18n keys riêng: chép kèm hoặc thay khoá cho Scribe.
