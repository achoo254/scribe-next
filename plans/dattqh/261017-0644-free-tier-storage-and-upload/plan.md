---
title: Gói free — xoá nhanh, xoá tất cả, tải tệp lên, 7 ngày, 200 MB
status: completed
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
| 2. Review, deploy (cần chủ dự án cho phép), migration hạn tệp free | M | done |

## Tiêu chí nhận
- Free: tệp mới hết hạn sau 7 ngày, quota 200 MB (tệp đơn tối đa 200 MB); plus: 14 ngày, 5 GB. `/api/me/usage` và `retentionDays` trả theo gói.
- Xoá từng bản không hộp xác nhận, có hoàn tác 5 giây; "Xoá tất cả" có hộp xác nhận, xoá sạch đĩa + Mongo + index.
- Tải tệp âm thanh từ máy (trang Ghi âm) → hiện trong lịch sử → chép lời, tìm kiếm, biên bản như bản ghi.
- Cổng: clippy 0, cargo test, lint, typecheck, vitest, build, e2e.

## Bổ sung 06:52
- Hiện gói đang dùng (Free/Plus, hạn nếu có) và gom phần dùng lượng thành một thẻ gọn: lưu trữ, phút chép lời tháng, phút hôm nay (free), ghi chú tự xoá.

## Review 17/10 07:20
Không Critical/High. Sửa: M1 hạ gói giữa lúc ghi làm mất cả bản ghi lúc finalize; M2 ngày tự xoá hiện theo gói hiện tại thay vì `expiresAt`;
M3 xoá tất cả chưa tha phiên iPhone đang ghi; Low: `keepalive` cho xoá thật, làm mới lịch sử khi dừng vì đầy. Chi tiết: `reports/code-reviewer-261017-0714-free-tier-review.md`.
Chủ dự án 07:16: tệp trang Cắt file không hiện trong "Lịch sử ghi" (và không bị "Xoá tất cả").

## Deploy 17/10 07:30
Chủ dự án cho phép ("deploy luôn khi xong"). Build từ `218e33a` (code `8b51576`). Sao lưu `/root/scribe-backup-20261017-072952`
(binary, dist, `files-expiry-before.json`). Binary sha256 `7ad3e9760931192c…`, `index-DCrYGjs5.js`. Health ok, `NRestarts=0`, không ERROR/WARN.
`admin shorten-free-retention` (xem trước): 0 user free, 0 tệp — cả 57 tệp prod của user duy nhất đang ở gói plus, nên không chạy `--apply`.
Test Mongo dev 13/13 đạt (test OpenAI thật cần key, bỏ qua).
- 07:35: trang Cắt file không liệt kê tệp tải lên từ trang Ghi âm (`658af64`, chỉ backend). Sao lưu `/root/scribe-backup-20261017-073535`, sha256 `bcc0434c6cefb1d3…`, health ok.
