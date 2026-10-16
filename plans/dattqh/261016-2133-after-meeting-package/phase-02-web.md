---
phase: 2
title: "Web: bảng biên bản, nút đánh dấu, gửi + liên kết chat"
status: completed
effort: "L"
---

# Phase 2: Web

Dùng khung và pattern đã có: `layout/page-header.tsx`, `components/ui/*`, `i18n/messages/vi/*` (mọi chuỗi; test `hard-coded-strings` chặn chuỗi cứng), React Query, data router.

- **Trang đọc bản chép lời (`pages/transcript/`):**
  - Khối "Biên bản" có ba trạng thái:
    - chưa có: nút "Tóm tắt";
    - đang chạy: trạng thái chờ;
    - có rồi: tóm tắt, quyết định, việc cần làm; mỗi mục có mốc giây bấm được (cuộn tới câu và phát từ giây đó, như đoạn hiện có).
  - Nhãn "Đã cũ" khi `stale`, kèm nút "Tóm tắt lại". Lỗi hạn mức hiện giờ mở lại.
  - Dấu mốc hiện trong dòng thời gian (bên cạnh câu gần nhất) và trong một danh sách gọn; xoá được.
  - Nút "Gửi sang chat": chưa liên kết thì mở hộp hướng dẫn liên kết.
- **Trang ghi (`pages/record/`, cả `native-record-card.tsx`):** nút "Đánh dấu" khi đang ghi. Giây tính từ đầu phiên; với iPhone, cộng các đoạn trước. Hiện số mốc đã đánh. Ghi lỗi thì hàng đợi thử lại, không chặn ghi.
- **Account menu:** mục "Liên kết chat.hoandat.com" và "Gỡ liên kết chat". Mở link ngoài qua `lib/external-link.ts` để iPhone mở bằng trình duyệt hệ thống. Khi trang quay lại tiêu điểm thì kiểm lại trạng thái liên kết.
- **Test:** vitest cho từng khối mới; Playwright stub API cho biên bản, dấu mốc và gửi; cập nhật ảnh chụp sáng/tối; axe sạch.
- Cổng: `pnpm lint && pnpm typecheck && pnpm check:tokens && pnpm vitest run --maxWorkers=2 && pnpm build && pnpm test:e2e`.

## Kết quả (16/10/2026)
- **Trang đọc:** khối "Biên bản" (`pages/transcript/minutes-card.tsx`) có ba trạng thái (nút "Tóm tắt", đang chạy, có biên bản), nhãn "Đã cũ" kèm "Tóm tắt lại"; lỗi `SUMMARY_DAILY_LIMIT`, `SUMMARY_MONTHLY_LIMIT`, `CAP_REACHED` hiện giờ mở lại theo `retryAt`. Bấm mốc giây thì phát từ giây đó, cuộn và đặt focus vào câu tương ứng. Dấu mốc hiện cạnh câu đang nói lúc bấm và trong danh sách "Dấu mốc" (nhảy tới, xoá).
- **Gửi sang chat:** đã liên kết thì gửi, chưa thì nút đổi thành "Liên kết chat" và mở hộp hướng dẫn (`components/chat-link-dialog.tsx`); `CHAT_NOT_LINKED`/`CHAT_BLOCKED` đưa về trạng thái chưa liên kết. Máy chủ tắt liên kết (`enabled: false`) thì ẩn cả mục menu lẫn nút gửi.
- **Liên kết chat:** account menu có "Liên kết chat.hoandat.com" hoặc "Gỡ liên kết chat" kèm giờ liên kết (`linkedAt`), chỉ cho tài khoản được chép lời. Deep link mở bằng `openExternal` ngay trong cú bấm (không bị chặn popup, iPhone mở trình duyệt hệ thống). Trạng thái được đọc lại khi cửa sổ lấy lại focus hoặc hiện lại, và mỗi 3 giây sau khi mở link tới `expiresAt`, để bắt bước "Xác nhận liên kết" trong chat.
- **Trang ghi:** nút "Đánh dấu" khi đang ghi, cho cả trình duyệt (khoá `f:<fileId>` từ `/api/recordings/init`) và iPhone (khoá `s:<sessionId>`; giây = tổng độ dài các đoạn đã đóng (sự kiện `segment` của plugin) cộng thời gian vào đoạn đang ghi). Hàng đợi `features/transcript-docs/bookmark-queue.ts` gửi nền, mạng/429/5xx thì thử lại sau 2–30 giây, bị từ chối thì báo lý do trong một dòng nhỏ; không bao giờ chặn việc ghi.
- `ApiError` mang thêm `code` và `quota` (`usedMinutes`/`limitMinutes`/`retryAt`).
- **Kiểm tra:** lint 0 lỗi (216 tệp); typecheck 0 lỗi; check:tokens đạt; vitest 50 tệp, 275 test đạt; build đạt, không cảnh báo; e2e 67 đạt, 7 bỏ qua (nhánh chỉ có ở Chromium hoặc ở iPhone). Ảnh chụp cập nhật: `reader-*`, `account-menu-*`; ảnh mới `reader-minutes-*`, `native-recording-*` (sáng/tối, đã xem). axe sạch trên trang đọc có biên bản, hộp liên kết, thẻ ghi iPhone.
- Chưa thử trên máy thật: bấm deep link trong WebView iPhone, và đánh dấu trong lúc ghi nền trên iPhone.
