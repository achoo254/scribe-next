# Red-team: plan 261016-1426 refactor UI/UX Scribe

Phạm vi: `plan.md` và phase 1–6, đối chiếu `web/src`, `web/ios/App/App`, `deploy/nginx-scribe.conf`, `server/scribe-be/src/auth.rs` và `localman-next/apps/web`. Không đảo quyết định nào trong bảng của Jev; các finding dưới đây chỉ ra chỗ cách làm hiện tại không chạy được, kèm cách sửa.

## Critical

**C1. `route-progress` cần data router, mà Scribe đang dùng `<BrowserRouter>`.** Ở Localman, `components/ui/route-progress.tsx:2,51` gọi `useNavigation()` và ghi chú dòng 48 nói rõ "Needs a data router"; `components/layout/app-shell.tsx:1` dùng `useMatches`; `router.tsx:1-4` dùng `createBrowserRouter`. Scribe thì dùng `main.tsx:15` `<BrowserRouter>` và `app.tsx:35` `<Routes>`. Phase 2 (dòng 16) đặt route progress vào khung rồi deploy prod, nên app sẽ ném lỗi "useNavigation must be used within a data router" và trắng trang. Plan không có bước nào chuyển sang data router. Phase 3 cũng cần data router cho `useBlocker` (xem H5).
→ Sửa: Phase 3 thêm hẳn một bước chuyển sang `createBrowserRouter` + `RouterProvider`, kèm sửa `test/render-with-providers.tsx` cho test. Chuyển route progress từ Phase 2 sang Phase 3. Nếu route không lazy và không có loader thì bỏ hẳn route progress, vì thanh đó sẽ không bao giờ hiện.

## High

**H1. Có hai lượt cắt độc lập chứ không phải một.** `record-page.tsx:44` (cắt sau khi ghi) và `split-page.tsx:31` mỗi nơi gọi `useSplitJob()` riêng. Nếu gộp vào một `SplitJobProvider`, `record-page.tsx:73` `split.reset()` sẽ xoá lượt cắt đang chạy của trang Cắt file.
→ Sửa: provider giữ hai instance có tên riêng (`recordingSplit`, `fileSplit`), test chéo hai trang.

**H2. Tải lên của trang Cắt file sẽ mất khi đổi route.** `split-page.tsx:30-35` giữ `useChunkedUpload`, `file`, `criterion` ở state cục bộ. `use-chunked-upload.ts` không huỷ khi unmount, nên `ChunkedUpload` vẫn chạy ngầm (tối đa 500 MB, nginx:53) nhưng không còn ai nhận kết quả, và "Tải tiếp" cần lại đúng đối tượng `File` cũ. Danh sách provider ở Phase 3 dòng 16 bỏ sót phần này.
→ Sửa: đưa upload + tệp đã chọn vào provider của trang Cắt file, hoặc chặn chuyển route khi `uploading` (cần C1).

**H3. Đăng xuất trong menu khi đang ghi.** Hiện nay recorder web dừng nhờ unmount (`use-recorder.ts:18`, `app-top-bar.tsx:17-18`). Nếu provider nằm "dưới AuthProvider" và bọc ngoài `<Routes>`, `logout()` (`auth-context.tsx:78-99`) không dừng recorder web: mic vẫn mở và các chunk nhận 401 khi `endSessionUrl` null. Trên iPhone, `RecorderPlugin.swift:194-197` gọi `signOut` sẽ `recorder.stop()` mà không hỏi lại. Đưa đăng xuất vào menu làm cú bấm nhầm dễ xảy ra hơn.
→ Sửa: đặt provider trong layout route nằm dưới `RequireAuth`; hỏi xác nhận khi `recording || uploadsWaiting > 0`; thêm test "đăng xuất khi đang ghi".

**H4. Bộ i18n chép sang thiếu tệp và lấy tiếng Anh làm gốc.** `i18n-context.ts:1-4` import `@localman/shared/constants`, `messages/en/dictionary` và `translate.ts`. `message-types.ts:22` định nghĩa `SourceMessages = typeof en`. `dictionaries.test.ts:47-55` so bản dịch với `en`. `i18n-provider.tsx:10-14` kéo theo `account-locale-sync`, `locale-preference`, `active-translator`. Nếu chỉ có `vi`, test "đạt với một khoá mẫu" (Phase 1 dòng 20) chạy nhưng không chứng minh gì.
→ Sửa: liệt kê đủ `translate.ts`, `message-types.ts`, `rich-text.tsx`; lấy `vi` làm kiểu nguồn; bỏ đồng bộ locale; import `vi` tĩnh, không tách chunk (xem M5); đổi test thành "mọi khoá có chữ, placeholder khớp".

**H5. Sửa dở trong transcript sẽ mất.** `transcript-reader.tsx:89-93` (`editing`, `titleDraft`, `rerun`) hiện sống được qua lần đổi tab nhờ force-mount (`app-shell.tsx:96-108`). Với URL, bấm nav là unmount và mất bản đang sửa.
→ Sửa: `useBlocker` khi đang sửa (cần C1), cộng test Playwright.

**H6. Giả định "vuốt lùi theo history" trên WKWebView là sai.** Capacitor mặc định tắt `allowsBackForwardNavigationGestures`; `MainViewController.swift` và `capacitor.config.ts` đều không bật. iPhone cũng không có nút back hệ thống.
→ Sửa: giữ `BackButton` trong trang (`transcript-reader.tsx:410`) cho mọi trang con. Nếu muốn có cử chỉ vuốt thì bật trong `MainViewController` (thay đổi native, chỉ có hiệu lực sau khi cài lại qua cáp) và ghi rõ vào Phase 3.

**H7. Test Phase 3 không chứng minh được "đủ chunk".** Phase 3 dòng 22 dùng recorder giả, nên chỉ chứng minh instance còn sống, không chứng minh chunk tải lên đủ.
→ Sửa: giữ vitest để kiểm "cùng instance, không gọi `dispose`". Thêm Playwright Chromium chạy `--use-fake-device-for-media-stream`, stub route đếm số request chunk trước/sau ba lần đổi route.

## Medium

**M1. Phụ thuộc ẩn khi chép khung Localman.** (Đường dẫn đúng là `components/layout/`, không phải `layout/`.)
- `app-header.tsx:2-10` kéo `useFeatures`, `NotificationBell`, `workspace-switcher`, `LanguageSwitcher`.
- `account-menu.tsx:13,18,66,136` kéo `NOTIFICATION_SETTINGS_PATH` và `flags.mockMcp`.
- `account-controls.tsx:2` kéo `use-logout`.
- `idp-account-items.tsx:8-9` kéo `tenant-info` (gọi `/api/v1/tenant`, Scribe không có endpoint này) và `platform/navigation`.
- `copy-field.tsx:14` kéo `platform/clipboard`.
- Danh sách chép còn thiếu `skip-link.tsx`.

→ Sửa: Phase 2 ghi từng import và thứ thay thế; đầu tiên chép `openExternal` cùng `safeExternalUrl`.

**M2. Nguồn URL của IdP.** `auth.rs:245-251` trả `authorizationEndpoint/clientId/redirectUri/scope`, không có issuer, và mỗi lần gọi lại chạy OIDC discovery. Mục "Ứng dụng" cũng chưa rõ đích: hoandat-auth có `/apps` (danh mục ứng dụng) và `/account#account-apps` (ứng dụng đã cấp quyền, đích Localman dùng ở `idp-account-items.tsx:32-37`).
→ Sửa: dùng hằng build `VITE_IDP_ORIGIN` (kiểm ở `vite.config.ts` `REQUIRED_ENV`), chọn rõ đích cho "Ứng dụng".

**M3. Viewport iPhone trong Playwright không phải native shell.** `isNativeShell()` (`lib/api.ts`) trả false, nên `NativeRecordCard`, đăng xuất qua `HoandatAuth` và safe area không bao giờ được render trong test.
→ Sửa: thêm một project dùng `addInitScript` stub `window.Capacitor` + plugin, đặt trong `e2e/`. Thêm bước CI chạy `grep` trên `dist/` để chắc không có chuỗi stub nào lọt vào bản build.

**M4. Native còn trạng thái gắn vào việc mount.** Mỗi lần mount, `use-native-recorder.ts:70-98` gọi `syncUploads`, mà hàm này `clearFailures` + `queue.start` (`RecorderPlugin.swift:182-187`). `native-record-card.tsx:29` giữ `minutes` cục bộ, nên khi remount lúc đang ghi sẽ hiện sai độ dài đoạn.
→ Sửa: provider giữ cả `minutes` và hiệu ứng sync, chỉ chạy một lần cho mỗi email.

**M5. Deploy khi người dùng đang mở tab.** §6 dùng `rsync -a --delete` nên chunk hash cũ bị xoá. Nếu sau này route hoặc từ điển `vi` thành lazy thì tab cũ đổi trang sẽ gặp 404. Điều kiện "không có phiên ghi" mới chỉ có ở Phase 3.
→ Sửa: không dùng lazy route/từ điển, hoặc giữ `assets/` cũ khi rsync; áp điều kiện hàng đợi rỗng + không có phiên ghi cho mọi phase có deploy (2, 4, 5).

**M6. Phase 2 deploy giao diện iPhone mà không thử trên máy thật.** Lần cài lại đầu tiên là ở Phase 3.
→ Sửa: cài qua cáp ngay ở Phase 2, hoặc ghi rõ là chấp nhận rủi ro.

## Low

- **L1.** Phase 1 dòng 16 không cần: `lib/use-delayed-flag.ts` và `panel-progress.tsx` đã trùng với Localman (diff 0 dòng); `SkeletonRegion` đã có (`skeleton.tsx:82`).
- **L2.** E2E của Localman chạy cả stack, chỉ Chromium, không có ảnh mốc (`e2e/playwright.config.ts`). Cách stub + ảnh mốc là cách mới, không phải bản chép. Cần ghi nền tảng sinh ảnh mốc (macOS), `workers: 2` và việc cài WebKit.
- **L3.** Khoá transcript có `:` (`s:aaaaaaaa-1111`, `app-shell.test.tsx:21`): phải `encodeURIComponent` khi dựng `/transcripts/:key`.
- **L4.** Nginx chưa có CSP. Đây không phải lỗi do plan gây ra, nhưng link ra ngoài phải có `noopener` và lọc giao thức như `openExternal`.
- **L5.** Cảnh báo `beforeunload` là hành vi mới (hiện chưa có) và không chạy trong WKWebView.

## Claim đã kiểm, đúng

- `tokens.css` chỉ khác 3 dòng chú thích. `button`, `card`, `switch`, `input`, `empty-state`, `form-field`, `progress-bar` diff 0; `modal` lệch 6 dòng, `toast` 9, `tabs` 2. Đúng là còn thiếu `dropdown-menu`, `select`, `textarea`, `route-progress`, `copy-field`.
- Khung hiện tại rộng 920 px (`max-w-230`); `--container: 1120px` và `max-w-page` đã có sẵn.
- Force-mount, tab Transcript ẩn khi `!canTranscribe`, `OpenTranscriptContext` (`app-shell.tsx:36-109`).
- Nginx đã có SPA fallback và `index.html` để `no-cache` (dòng 80-82).
- Luồng `endSessionUrl` trên web và `HoandatAuth.signOut` trên iPhone (`auth-context.tsx:86-98`).
- Bản cài iPhone tự mang `dist` riêng (`ios-device.sh`: `npm run build && cap sync`), nên bản cũ vẫn chạy được với cùng API.
- `reconcile` truyền `activeSessionId` nên phiên đang ghi không bị tính là mất.
- Deep link `com.hoandat.scribe:` chỉ là callback của ASWebAuthenticationSession; web không xử lý, nên giữ nguyên không làm hỏng gì.

## Câu hỏi còn mở

- "Ứng dụng" nên trỏ tới `/apps` hay `/account#account-apps`?
- Có muốn bật cử chỉ vuốt lùi native (cần cài lại app qua cáp) hay chỉ dùng nút back trong trang?

Status: DONE_WITH_CONCERNS
Summary: Plan khả thi nhưng có một lỗi chắc chắn làm sập prod: route-progress cần data router trong khi Scribe đang dùng `BrowserRouter`. Danh sách provider ở Phase 3 còn sót upload, lượt cắt thứ hai và bản sửa transcript; cần bổ sung trước khi bắt đầu Phase 2.
