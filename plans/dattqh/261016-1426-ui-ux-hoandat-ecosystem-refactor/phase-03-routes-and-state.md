---
phase: 3
title: "Điều hướng URL + provider giữ trạng thái"
status: pending
priority: P1
effort: "L"
dependencies: [2]
---

# Phase 3: Điều hướng URL + provider giữ trạng thái

Hiện `layout/app-shell.tsx` force-mount cả ba tab để phiên ghi, lượt cắt, transcript đang mở sống qua lần chuyển tab. Có URL thì trang bị unmount ⟹ trạng thái phải nằm trên trang.

## Files
- `main.tsx`/`app.tsx`: chuyển `<BrowserRouter>` (`main.tsx:15`) sang `createBrowserRouter` + `RouterProvider`; thêm `route-progress.tsx` (chép Localman). Route `/record`, `/split`, `/transcripts`, `/transcripts/:key` (khoá có `:` ⟹ `encodeURIComponent`), `/` → `/record`; `/transcripts*` chỉ khi `canTranscribe`. Nginx đã có SPA fallback.
- Provider cấp app **dưới `RequireAuth`** (đăng xuất unmount ⟹ dừng mic web; native có xác nhận ở Phase 2):
  - `RecorderProvider`: recorder web (`features/recorder/recorder.ts`) + native (`native-recorder.ts`): trạng thái ghi, đồng hồ, wake lock, tiến độ tải, lựa chọn độ dài đoạn; đồng bộ upload native **một lần mỗi phiên app**, không mỗi lần mount.
  - Hai lượt cắt độc lập (`record-page.tsx:44` cắt bản ghi, `split-page.tsx:31` cắt tệp tải lên): hai instance riêng, `split.reset()` của trang ghi không chạm lượt của trang Cắt file.
  - `SplitPageProvider`: tệp đã chọn, tiến độ tải lên (tới 500 MB), tiêu chí, kết quả (`split-page.tsx:30-35`).
  - Bỏ `OpenTranscriptContext` (mở transcript = điều hướng `/transcripts/:key`).
- Transcript đang sửa (`transcript-reader.tsx:89-93` nhờ force-mount): `useBlocker` hỏi trước khi rời trang khi còn ô chưa lưu.
- Không thêm `beforeunload` (WKWebView không chạy; hành vi mới không cần).
- iPhone: nút quay lại trong trang chi tiết; bật `webView.allowsBackForwardNavigationGestures = true` trong `ios/App/App/MainViewController.swift` (Jev 0,57), cài lại app cuối phase. Deep link `com.hoandat.scribe:` không đổi.

## Verification
- Playwright Chromium với `--use-fake-device-for-media-stream --use-fake-ui-for-media-stream`: bắt đầu ghi → đổi route ×3 → dừng ⟹ một `recordings/init`, số request `chunk` liên tục không trùng index, một `finalize`. Tải tệp lên ở Cắt file → sang Ghi âm → quay lại: tiến độ còn, kết quả tới.
- vitest: native recorder (stub plugin) giữ trạng thái qua remount, upload sync gọi một lần; `useBlocker` chặn khi có ô chưa lưu; `/transcripts/:key` mở đúng bản (khoá có `:`); tab Transcript ẩn với user không có quyền.
- iPhone thật (cài lại qua cáp): ghi → đổi trang → khoá màn hình 2 phút → mở → dừng ⟹ tải lên đủ; tối thiểu một bản chép lời.
- Deploy web prod khi hàng đợi rỗng và không có phiên ghi đang mở (`recording_sessions` chưa finalize).

## Kết quả (16/10)

Đã làm xong trong `web/` và `ios/App/App/MainViewController.swift`. Chưa commit, chưa deploy, chưa cài lại app iPhone.

**Router**
- `main.tsx` tạo `createAppRouter()` (`routes.tsx`, `createBrowserRouter`); `app.tsx` đặt `RouterProvider` dưới I18n/Query/Tooltip/Toast/Auth (các provider này không dùng route). Route: `/` → `/record`, `/record`, `/split`, `/transcripts`, `/transcripts/:key`, `/login`, `/__ui` (chỉ dev), mọi đường khác → `/`. `/transcripts*` bọc `RequireTranscribe`: không có quyền thì về `/record`. Trang không lazy (M5).
- Khoá transcript qua `transcriptPath()` (`layout/app-paths.ts`, `encodeURIComponent`): `s:a1b2…` thành `/transcripts/s%3Aa1b2…`, `useParams` trả lại khoá đã giải mã.
- `route-progress.tsx` chép nguyên từ Localman (có ghi nguồn) và gắn vào `AppShell`. Scribe không có loader hay route lazy, nên hiện tại thanh gần như không bao giờ hiện; nó sẽ có tác dụng khi có loader.
- Nav trên top bar dùng `NavLink` (class giống hệt `NAV_LINK_CLASS` của Localman): `aria-current="page"` theo URL, và vẫn đánh dấu "Transcript" khi đang ở `/transcripts/:key`. Wordmark trỏ `/record`.

**Provider dưới `RequireAuth`** (`layout/signed-in-state.tsx` bọc `AppShell`; đăng xuất thì unmount, mic web được trả)
- `pages/record/record-state.tsx`: trên web là `BrowserRecordProvider` (giữ `useRecorder`, lượt cắt sau khi ghi, nguồn ghi, tuỳ chọn cắt, `toggle`, `stopAndSave`); trong app iOS là `NativeRecordProvider` (giữ `useNativeRecorder` và độ dài đoạn). Vì `useNativeRecorder` chỉ được gọi ở provider, `syncUploads` chạy một lần cho mỗi lần đăng nhập, không chạy lại mỗi lần vào `/record` (M4).
- `pages/split/split-state.tsx` (`SplitPageProvider`): giữ upload, đúng đối tượng `File` (cần cho "Tải tiếp"), tiêu chí, giá trị và job của trang Cắt file (H2). Job này độc lập với lượt cắt sau khi ghi, nên `split.reset()` của trang ghi không đụng tới (H1).
- `layout/session-activity.ts` viết lại: không còn cơ chế publish/report, `useSessionActivity()` đọc thẳng các provider. Hộp xác nhận đăng xuất của Phase 2 giữ nguyên (lưu bản ghi web trước, rồi mới logout).
- Đã bỏ `OpenTranscriptContext`. `useOpenTranscript()` giờ điều hướng tới `/transcripts/:key` và trả `null` khi người dùng không có quyền chép lời.
- Các trang không còn prop `active` (trang chỉ mount khi đang hiện). Đã bỏ tham số `active` của `useTranscriptDocsQuery`/`useTranscriptDoc` và effect refetch của `ManagedFilesCard`, vì `refetchOnMount: "always"` đã lo việc này.

**Transcript đang sửa** (H5): `layout/leave-guard.tsx` dùng `useBlocker` và `ConfirmModal`. Khi còn ô câu đang sửa hoặc đang đổi tên, mọi điều hướng trong app (nav, nút back, vuốt lùi) đều hỏi "Rời trang khi chưa lưu?" trước. Xoá transcript thì rời trang không hỏi. Không thêm `beforeunload`. Chuỗi mới nằm ở `messages/vi/layout.ts` (`leaveGuard`).

**iPhone** (H6): trang chi tiết vẫn có nút "Danh sách" (về `/transcripts`). `MainViewController.capacitorDidLoad` thêm `webView?.allowsBackForwardNavigationGestures = true` (kèm `import WebKit`); `webView` là thuộc tính public của `CAPBridgeViewController`, đã có sau `loadView`. Chưa build Xcode: lần build và cài qua cáp làm sau trên máy Mac của người dùng.

**Test**
- vitest `--maxWorkers=2`: 44 tệp, 228 test đạt. Mới: `routes.test.tsx` (`/` → `/record`; khoá có `:` mở đúng bản; mở transcript từ trang khác đổi URL; người không có quyền không có trang Transcript), `record-state.test.tsx` (web: một recorder, không `dispose` qua 4 lần đổi route, tuỳ chọn cắt còn nguyên; native với plugin stub: trạng thái Swift, độ dài đoạn và đồng hồ còn sau remount, `syncUploads` 1 lần, listener gắn 1 lần), `transcript-reader.test` thêm 2 test `useBlocker`. Đã viết lại `app-shell.test` và `app-header.test`. `test/render-with-providers.tsx` chuyển sang `createMemoryRouter`; `test/setup.ts` chép shim `Request` của Localman (undici không nhận AbortSignal của jsdom).
- Playwright: 38 đạt, 4 bỏ qua có chủ đích (3 test nhánh iOS chỉ chạy trên viewport điện thoại; test ghi âm chỉ chạy trên Chromium). Spec mới `routes.spec.ts`:
  - Chromium với `--use-fake-device-for-media-stream --use-fake-ui-for-media-stream`: bắt đầu ghi, đổi route 3 lần, dừng. Kết quả: 1 `recordings/init`, 1 `finalize`, mọi chunk cùng một session, mỗi offset bằng tổng kích thước các chunk trước nên không mất và không trùng, và chunk vẫn tới sau các lần đổi route.
  - Upload 12 MB ở Cắt file: giữ chunk thứ 2 lại, sang Ghi âm, quay lại vẫn thấy 41%. Thả chunk khi đang ở trang khác, quay lại thấy "Đã tải lên". Bấm Cắt, rời trang, quay lại thấy "Kết quả (2 đoạn)". Đếm được 1 init, 3 chunk, 1 job.
  - Nav và back/forward của trình duyệt cập nhật URL; nút "Danh sách"; deep link và reload `/transcripts/s%3A…`; leave guard khi đang đổi tên, với cả nav lẫn back.
  - Không có quyền: không có link Transcript, `/transcripts*` về `/record`.
- Stub API (`e2e/support/api-stubs.ts`) thêm `recordings/*`, `upload/*`, `jobs`, chi tiết transcript-doc và 404 cho media. WebKit không cho Playwright đọc body dạng Blob, nên kích thước chunk upload được suy từ `size` khai lúc init.
- Đã thử phá: cho trang ghi tự tạo provider riêng (state theo lần mount) thì test ghi âm fail; trả code lại thì đạt.
- lint: 185 tệp, sạch. typecheck (src, scripts, e2e): 0 lỗi. check:tokens: 82 cặp đạt, 9 cảnh báo APCA đã có từ trước. build: đạt, 0 cảnh báo; `check:dist` sạch (6 tệp).
- **Ảnh mốc: không cập nhật ảnh nào.** Nav đổi từ `<button>` sang `<a>` nhưng 22 ảnh vẫn khớp.

**Còn mở**: cài lại app iPhone qua cáp và thử tay (ghi → đổi trang → khoá màn hình 2 phút → dừng; vuốt lùi); deploy web prod theo điều kiện của plan. Câu chữ "tab Ghi âm" ở trạng thái rỗng của danh sách transcript để Phase 5 sửa.
