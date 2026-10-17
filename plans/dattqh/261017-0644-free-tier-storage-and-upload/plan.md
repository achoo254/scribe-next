---
title: Gói free — xoá nhanh, xoá tất cả, tải tệp lên, 7 ngày, 200 MB
status: in-progress
created: 2026-10-17
tier: L
---

# Gói free — lưu trữ và tải tệp lên

Yêu cầu chủ dự án 17/10/2026 06:44. Áp cho **gói free**; gói plus giữ 5 GB và 14 ngày.

## Quyết định
- Phút chép lời free mỗi tháng **giữ 120** (chủ dự án chốt; 480 là tăng chứ không giảm).
- Jev (17/10): xoá tất cả = mọi bản ghi + tệp tải lên kèm transcript/biên bản/index, hộp xác nhận nêu số lượng và dung lượng (0,78);
  tệp free có sẵn rút hạn còn createdAt + 7 ngày sau khi sao lưu (0,98); đang vượt 200 MB thì giữ tệp, chặn ghi/tải mới (1,00);
  tải tệp lên ở trang Ghi âm, hiện trong "Lịch sử ghi", chép lời như bản ghi (0,83).
- Xoá nhanh: Jev nghiêng giữ hộp xác nhận (0,41, độ tin 0,11) — **không theo** vì trái yêu cầu; chọn bấm một lần + "Hoàn tác" 5 giây,
  gọi xoá thật khi hết giờ (0,23).
- Bổ sung của chủ dự án (17/10): hiện gói đang dùng ("Gói Free"/"Gói Plus", kèm hạn) và gộp mức dùng (lưu trữ, phút tháng, phút ngày) vào một thẻ gọn đầu trang.

## Phase
| Phase | Tier | Trạng thái |
|---|---|---|
| 1. Backend + web + test | L | done |
| 2. Review, deploy (cần chủ dự án cho phép), migration hạn tệp free | M | pending |

## Tiêu chí nhận
- Free: tệp mới hết hạn sau 7 ngày, quota 200 MB (tệp đơn tối đa 200 MB); plus: 14 ngày, 5 GB. `/api/me/usage` và `retentionDays` trả theo gói.
- Xoá từng bản không hộp xác nhận, có hoàn tác 5 giây; "Xoá tất cả" có hộp xác nhận, xoá sạch đĩa + Mongo + index.
- Tải tệp âm thanh từ máy (trang Ghi âm) → hiện trong lịch sử → chép lời, tìm kiếm, biên bản như bản ghi.
- Cổng: clippy 0, cargo test, lint, typecheck, vitest, build, e2e.

## Bổ sung 06:52
- Hiện gói đang dùng (Free/Plus, hạn nếu có) và gom phần dùng lượng thành một thẻ gọn: lưu trữ, phút chép lời tháng, phút hôm nay (free), ghi chú tự xoá.
