---
phase: 2
title: "Web: bảng biên bản, nút đánh dấu, gửi + liên kết chat"
status: pending
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
