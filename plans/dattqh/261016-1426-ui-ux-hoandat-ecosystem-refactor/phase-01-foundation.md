---
phase: 1
title: "Nền: sync primitive, i18n, Playwright"
status: completed
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

## Kết quả (16/10)

Đã làm xong trong `web/`, chưa commit và không deploy (giao diện người dùng thấy không đổi).

**Đã chép**
- `components/ui/`: `dropdown-menu.tsx`, `menu-styles.ts`, `select.tsx`, `textarea.tsx`, giữ nguyên nội dung Localman và ghi nguồn ở đầu tệp. Thêm `@radix-ui/react-dropdown-menu@^2.1.25`, đúng bản Localman đang khoá. Localman không có test cho bốn tệp này. Chưa trang nào import chúng nên bundle prod không đổi.
- `modal.tsx` và `toast.tsx` giờ trùng từng dòng với Localman: nhãn "Đóng", "Thông báo", "Đóng thông báo" lấy qua `useT()`, chữ giữ nguyên.
- `tabs.tsx` giữ bản của Scribe (`overflow-y-hidden`, commit 778cd7e, để ẩn thanh cuộn dọc ở hàng tab).
- i18n: `translate.ts`, `message-types.ts`, `active-translator.ts`, `rich-text.tsx` (kèm test của nó), `i18n-context.ts`, `i18n-provider.tsx`; `locales.ts` thay cho `@localman/shared/constants`. Kiểu khoá lấy từ `messages/vi` (3 khoá mẫu đều đang được dùng). `I18nProvider` bọc ngoài cùng trong `app.tsx`; router vẫn là `BrowserRouter`.

**Bỏ qua và lý do**
- `load-messages.ts`, `setLocale`, `locale-preference`, `account-locale-sync`, `language-switcher`: chỉ có một ngôn ngữ nên không có gì để nạp hay chuyển. Từ điển `vi` import tĩnh để tab đang mở không bị 404 chunk sau deploy (M5). Khi thêm en/zh thì chép lại từ Localman.
- `route-progress.tsx` để Phase 3 (C1). `use-delayed-flag`, `PanelProgress`, `SkeletonRegion` đã có sẵn (L1).

**Playwright** (`web/e2e/`, `npm run test:e2e`)
- Vite riêng ở cổng 5180 (`reuseExistingServer: false`, tự tắt khi chạy xong). `page.route` trả `/api/*` theo dữ liệu mẫu có kiểu lấy từ `src/` và chặn mọi origin khác, nên không cần backend 8787 và không gọi tới prod. `VITE_NATIVE_API_ORIGIN` được trỏ về chính cổng 5180.
- Stub Capacitor (`support/capacitor-stub.ts`) dựng `webkit.messageHandlers.bridge`, `PluginHeaders`, `nativePromise` và `nativeCallback` để app render nhánh iOS.
- Chạy trên hai project: `chromium-desktop` và `webkit-iphone` (iPhone 15). Ảnh chụp sáng/tối có ngưỡng `maxDiffPixelRatio` 0,002, đồng hồ cố định ngày 16/10/2026, `reducedMotion` và chờ font. axe chặn mọi lỗi WCAG A/AA mức serious/critical. Ảnh mốc tạo trên macOS (hậu tố `-darwin`), tổng 10 ảnh.
- `npm run check:dist` (`e2e/check-dist.ts`) chạy cuối `npm run build` và báo lỗi nếu `dist/` chứa `scribe-e2e-stub` hoặc `__scribeE2eNative`. Đã thử cài một marker vào dist: lệnh trả exit 1.

**Kiểm tra**
- lint: 158 tệp, sạch. typecheck (src + scripts + e2e): sạch. check:tokens: 82 cặp tương phản đạt, có 9 cảnh báo APCA (đã có từ trước).
- vitest: 37 tệp, 190 test đạt, trong đó 4 tệp và 18 test i18n là mới. `dictionaries.test` có 5 test và đã thử lỗi: thêm một khoá thừa có chuỗi rỗng thì 2 test fail.
- e2e: 16 đạt, 2 bỏ qua có chủ đích (nhánh iOS không chạy trên desktop).
- build: đạt, không có cảnh báo; dist không chứa stub.
