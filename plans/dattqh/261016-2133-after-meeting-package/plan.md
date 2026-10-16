---
title: "Gói Sau cuộc họp: tóm tắt, đánh dấu, gửi biên bản sang chat"
status: completed
created: 2026-10-16
tier: L
---

# Gói "Sau cuộc họp"

## Kết quả mong muốn
Sau khi chép lời xong, người dùng có:
- **biên bản:** tóm tắt, các quyết định và việc cần làm (người làm và hạn nếu có nói), mỗi mục kèm mốc giây để nhảy tới;
- **dấu mốc:** các mốc đã bấm "Đánh dấu" khi đang ghi, hiện trong bản chép lời;
- **gửi biên bản:** một nút gửi sang chat riêng của họ với bot Scribe trên chat.hoandat.com.

## Quyết định (Jev, 16/10/2026)
| Câu hỏi | Chọn | Xác suất |
|---|---|---|
| Khi nào tóm tắt | Khi người dùng bấm "Tóm tắt", có tạo lại | 0,53 (sát auto 0,47; chọn vì rẻ hơn) |
| Model | Mini không suy luận, ngữ cảnh dài: `gpt-4.1-mini` (key server có quyền, đã kiểm) | 0,92 |
| Đo lường | Bộ đếm riêng N lần/ngày cho free + chi phí token tính vào trần tổng tháng và cảnh báo | 0,96 |
| Màn hình khoá iPhone | Nút trong app/web bây giờ; đánh dấu từ màn hình khoá/AirPods để đợt có máy | 1,00 |
| Liên kết chat | Backend long-poll `GET /bot/v1/updates`, không thêm endpoint public | 0,79 |
| Gửi tới đâu | Chỉ chat riêng của người dùng với bot | 0,99 |

## Ngoài phạm vi
Đánh dấu từ màn hình khoá hoặc AirPods; gửi vào nhóm; tự động tóm tắt; hỏi đáp trên bản chép lời (thuộc gói "Kho bản ghi").

## Phase
| # | Phase | Tier | Status | Phụ thuộc |
|---|---|---|---|---|
| 1 | [Backend: tóm tắt, dấu mốc, liên kết + gửi chat](./phase-01-backend.md) | L | Completed | — |
| 2 | [Web: bảng biên bản, nút đánh dấu, gửi + liên kết chat](./phase-02-web.md) | L | Completed | 1 |
| 3 | [Cổng kiểm tra, deploy, docs](./phase-03-ship.md) | M | Completed | 2 |

## Tiêu chí nghiệm thu
- Bấm "Tóm tắt" trên một bản chép lời thật (dev) ra biên bản tiếng Việt có mốc giây đúng; bấm lại khi hết lượt trong ngày thì thấy thông báo hạn mức kèm giờ mở lại.
- Đánh dấu trong lúc ghi (web và trong WebView iPhone dùng chung nút) lưu đúng giây và hiện trong bản chép lời.
- Liên kết chat qua deep link, gửi biên bản tới chat riêng; gỡ liên kết thì nút gửi đổi về "Liên kết chat".
- Mọi cổng kiểm tra đạt (clippy, cargo test, lint, typecheck, check:tokens, vitest, build, e2e) trước khi deploy.
