---
title: "Gói Kho bản ghi: tìm kiếm toàn văn và hỏi đáp có trích mốc giây"
status: in-progress
created: 2026-10-16
tier: L
---

# Gói "Kho bản ghi"

## Kết quả mong muốn
- **Tìm kiếm** trong mọi bản chép lời của mình. Gõ có dấu hay không dấu đều được. Mỗi kết quả là một câu kèm tiêu đề bản ghi và mốc giây; bấm vào thì mở trang đọc, cuộn tới câu và phát từ giây đó.
- **Hỏi đáp** bằng câu hỏi tự nhiên trên cả kho, hoặc chỉ trong bản ghi đang mở. Câu trả lời tiếng Việt trích nguồn dạng [bản ghi, mốc giây], bấm vào là nhảy tới chỗ đó.

## Quyết định (Jev, 16/10/2026)
| Câu hỏi | Chọn | Xác suất |
|---|---|---|
| Cách tìm kiếm | Collection mỗi câu một doc `{userId,key,fileId,at,text,norm}`, `norm` viết thường và bỏ dấu (đ thành d), text index trên `norm`, cộng khớp cụm từ hoặc tiền tố trong code; dựng lại khi chép xong, khi sửa câu, khi xoá | 0,93 |
| Phạm vi hỏi đáp | Cả kho: tìm đoạn liên quan bằng index rồi đưa vào model. Mở rộng nhỏ: tham số `key` để hỏi trong một bản ghi; bản ghi ngắn thì gửi nguyên bản | 0,41 (one 0,28 · both 0,31) |
| Hạn mức hỏi đáp | Bộ đếm ngày riêng (free 30, trả phí 200) cộng trần chi phí tháng mỗi người và trần tổng, dùng chung với tóm tắt | 0,88 |
| Lịch sử hỏi đáp | Không lưu, chỉ nằm trên trang | 0,66 |

## Ngoài phạm vi
Embedding hoặc vector search; lưu lịch sử hỏi đáp; tìm trong biên bản (chỉ tìm trong câu).

## Phase
| # | Phase | Tier | Status |
|---|---|---|---|
| 1 | [Backend + web: index câu, tìm kiếm, hỏi đáp](./phase-01-build.md) | L | Pending |
| 2 | [Review, deploy, docs](./phase-02-ship.md) | M | Pending |

## Tiêu chí nghiệm thu
- Gõ "hop dong" tìm ra câu có "hợp đồng"; tìm cụm nhiều từ; kết quả có mốc giây, bấm vào là phát đúng chỗ. Index cập nhật sau khi chép lời, sửa câu và xoá bản ghi.
- Hỏi đáp trả lời đúng theo bản ghi thử và trích mốc giây đúng câu nguồn. Khi không có trong bản ghi thì nói rõ là không tìm thấy. Hết lượt thì báo giờ mở lại.
- Dữ liệu cũ đã có trên prod được đưa vào index lúc khởi động (backfill một lần, có cờ đánh dấu đã chạy).
- Mọi cổng kiểm tra đạt.
