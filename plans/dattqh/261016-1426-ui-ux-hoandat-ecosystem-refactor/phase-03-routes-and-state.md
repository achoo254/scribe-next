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
