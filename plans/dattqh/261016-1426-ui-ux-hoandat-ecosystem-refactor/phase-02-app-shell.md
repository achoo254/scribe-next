---
phase: 2
title: "Khung app: top bar, account menu, footer, layout"
status: completed
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

## Kết quả (16/10)

Đã làm xong trong `web/`. Chưa commit và chưa deploy: deploy prod chờ điều phối viên, theo điều kiện deploy của plan.

**Đã chép từ Localman** (`localman-next/apps/web/src/components/layout/`, mỗi tệp ghi nguồn ở đầu tệp)
- `app-header.tsx`: thanh trên cùng sticky, `min-h-(--topbar-h)`, `bg-bg/82 backdrop-blur-md`. Bấm wordmark (kèm BrandMark) về `/` và chọn lại Ghi âm. Nav là các nút ghost, trang đang mở có `aria-current="page"` (`bg-primary/10 text-primary`). Nav vẫn đổi view trong trang, chưa đổi URL. Tab Transcript ẩn khi `!canTranscribe`. Dưới 640 px (mọi điện thoại, kể cả app iPhone), nav wide bị ẩn và thay bằng một hàng segmented gọn ở ngay dưới thanh, vẫn nằm trong phần sticky. Thanh có `pt-[env(safe-area-inset-top)]`.
- `account-menu.tsx`, `account-controls.tsx`, `idp-account-items.tsx`, `user-avatar.tsx`: avatar dùng chữ cái đầu của email, vì phiên chỉ có email. Menu gồm email, "Tài khoản hoandat" (`<idp>/account`), "Ứng dụng" (`<idp>/account#account-apps`) và "Đăng xuất". Trên app iPhone nút mở menu ở dạng gọn (chỉ avatar). Hai link mở bằng `openExternal` (`window.open(url, "_blank", "noopener,noreferrer")`, chỉ nhận http(s)). Trên iPhone, Capacitor tự chuyển `window.open` và `target=_blank` sang trình duyệt hệ thống (`WebViewDelegationHandler.createWebViewWith` → `UIApplication.shared.open`), nên không phải thêm plugin nào.
- `app-footer.tsx`: hai link IdP (`<a target="_blank" rel="noopener noreferrer">`) và theme toggle, chuyển từ thanh trên cùng xuống. Footer có `env(safe-area-inset-bottom)`. Không có form góp ý, không có chọn ngôn ngữ.
- `skip-link.tsx`: `MAIN_CLASS` thêm `grid gap-4` để xếp các khối của trang.
- `lib/external-link.ts`: chép `safeExternalUrl` và `openExternal` từ `platform/navigation.ts`.

**Đã bỏ** feature flags, chuông thông báo, workspace switcher, Docs, plan, `tenant-info` và `use-logout` của Localman, các mục admin/notifications/home trong menu, và route-progress (để Phase 3).

**`<idp>`**: `lib/idp.ts` giữ giá trị mặc định `https://auth.hoandat.com` và đọc `VITE_IDP_ORIGIN` (chỉ lấy origin). `vite.config.ts` cho build thất bại nếu biến có giá trị nhưng không phải origin http(s) (đã thử với `ftp://bad`: build báo lỗi). Biến có ghi trong `web/.env.example` và có kiểu ở `vite-env.d.ts`.

**Đăng xuất và xác nhận**
- `use-sign-out.ts` gọi `logout()` sẵn có của `auth-context.tsx` (`endSessionUrl` trên web, `HoandatAuth` và `Recorder.signOut` trên iPhone), rồi chuyển về `/login`.
- Trạng thái "đang ghi / đang chờ tải" lấy qua `layout/session-activity.ts`, chỉ là tín hiệu tạm cho tới Phase 3. Recorder và phần tải lên vẫn nằm trong trang của chúng. Mỗi trang báo phần việc của mình qua `useReportSessionActivity`: `BrowserRecordPage` báo khi đang ghi, đang bắt đầu hoặc đang lưu; `NativeRecordCard` báo khi đang ghi và số đoạn chờ tải (`uploadsWaiting`); `SplitPage` báo khi đang tải tệp. AppShell cộng các báo cáo lại và truyền xuống account menu. Không thay đổi recorder nào.
- Khi có việc đang chạy, menu mở `ConfirmModal`, ghi rõ từng việc sẽ bị ảnh hưởng. Focus mặc định nằm ở nút "Huỷ". Nếu người dùng xác nhận trên web, app dừng và lưu bản ghi trước (`recorder.stop()`, chờ finalize xong) rồi mới xoá phiên, để không có chunk nào bị 401. Trên iPhone, `Recorder.signOut` tự dừng phiên ghi; các đoạn chưa tải vẫn nằm trên máy và tải tiếp sau khi đăng nhập lại.

**Khung**: `app-shell.tsx` gồm skip link, header, `<main id="main">` `max-w-page` (1120 px, thay cho 920 px cũ), quota bar ở đầu `<main>`, rồi tới các trang và footer. Phần Radix Tabs đã bỏ. Các trang vẫn mounted và chỉ ẩn bằng `hidden`, nên đổi trang không làm mất phiên ghi (có cả vitest lẫn Playwright kiểm). Đã xoá `app-top-bar.tsx`. Safe-area trên/dưới chuyển từ `#app` sang header, footer và trang đăng nhập; `#app` chỉ còn safe-area trái/phải. Nếu giữ trên `#app`, khi cuộn nội dung sẽ lộ ra phía trên thanh sticky.

**i18n**: mọi chuỗi mới nằm trong `messages/vi/layout.ts`. Các chuỗi cũ của `ThemeToggle` và `ConfirmModal` ("Huỷ") chưa chuyển, để Phase 5 làm.

**Kiểm tra**
- lint: 175 tệp, sạch. typecheck (src, scripts, e2e): 0 lỗi. check:tokens: 82 cặp tương phản đạt, 9 cảnh báo APCA đã có từ trước.
- vitest `--maxWorkers=2`: 42 tệp, 222 test đạt. Có 6 tệp mới: `app-header`, `account-controls` (menu, link, xác nhận khi ghi, thứ tự lưu bản ghi rồi mới logout), `app-footer`, `external-link`, `idp`, và `app-shell` viết lại. Đã thử bỏ nhánh hỏi xác nhận trong code: 4 test fail đúng như mong đợi.
- build: đạt, không có cảnh báo. `check:dist` sạch (6 tệp).
- e2e: 31 đạt, 3 bỏ qua có chủ đích (nhánh iOS không chạy trên desktop). `shell.spec.ts` mới có 12 ảnh mốc: top bar, menu đang mở và footer, ở sáng/tối, trên Chromium desktop (UI web) và WebKit iPhone 15 (UI native qua stub Capacitor). Spec này còn kiểm đổi trang mà trang cũ không bị unmount, và luồng đăng xuất khi app iPhone đang ghi (stub `Recorder.getStatus` ở trạng thái recording, 2 đoạn chờ): hiện hộp xác nhận, axe sạch, gọi `Recorder.signOut` và `HoandatAuth.signOut`, rồi về `/login`. Khi menu đang mở, axe chỉ quét menu, vì Radix gắn `aria-hidden` cho phần còn lại của trang và axe sẽ báo nhầm `aria-hidden-focus`; trang bên dưới đã được quét ở test top bar.
- Ảnh mốc đã cập nhật có chủ đích: 6 ảnh `record-*` (thanh mới, nav, container 1120 px, footer). Ảnh `login-*` không đổi.

**Còn mở**: chưa thử trên iPhone thật; app đang cài vẫn là bản build cũ cho tới khi cài lại ở Phase 3 (M6). `docs/deploy-web-production.md` và `docs/ios-app.md` chưa nhắc tới `VITE_IDP_ORIGIN`; biến này không bắt buộc vì đã có mặc định.

- Deploy web prod 2026-10-16 15:13: hàng đợi 0, không phiên ghi/tải mở; sao lưu `/root/scribe-backup-20261016-151337/web`; giữ `assets/` cũ; prod phục vụ `index-ch3ZoJBv.js` (khớp build), `/record` 200, `/api/health` ok. App iPhone đang cài chưa đổi (cài lại ở Phase 3).
