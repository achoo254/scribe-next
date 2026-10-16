# Journal: gói "Kho bản ghi" lên prod (17/10/2026)

**Kết quả.** Gói có hai phần. Phần thứ nhất là tìm kiếm toàn văn trong mọi bản chép lời của mình, gõ có dấu hay không dấu đều được; bấm một kết quả là mở trang đọc ở đúng giây. Phần thứ hai là hỏi đáp trên cả kho hoặc trong một bản ghi bằng `gpt-4.1-mini`, câu trả lời có trích mốc giây. Gói lên prod lúc 06:23. Backfill chạy xong với 0 phiên vì prod mới có một transcript đã chép xong và transcript đó không có câu nào. Jev quyết bốn điểm thiết kế, sau review quyết thêm mốc gửi nguyên bản ghi cho gói free.

**Bài học.**
- Xoá dữ liệu thì phải xoá khỏi index ngay trong request, không để task nền làm. Nếu task nền lỗi hoặc tiến trình restart, câu đã xoá vẫn tìm thấy được và còn bị gửi cho model.
- Không thay được cả phiên một cách nguyên tử thì ghi thế hệ mới trước, xoá thế hệ cũ sau, rồi để đối soát định kỳ sửa phần bị ngắt. Như vậy không có lúc nào phiên mất hết câu.
- Đối soát phải phân biệt "thiếu câu" với "không có câu": đoạn im lặng chép xong mà không ra câu nào. Trên dev, nhầm hai trường hợp này làm một phiên bị dựng lại ở mỗi lượt quét.
- Trần ứng viên phải lấy theo thứ tự có nghĩa (phiên mới trước). Nếu lấy theo thứ tự khoá thì phiên mới bị bỏ sót.
- Cho model trích số dòng `[n]` của ngữ cảnh rồi server tự đổi thành nguồn thì đáng tin hơn bắt model chép lại khoá phiên.
- Gửi nguyên bản ghi dài cho mỗi câu hỏi có thể làm cạn trần tháng của gói free sau vài câu. Mốc gửi nguyên bản vì thế phải tính theo gói.

**Còn lại.** Chủ sản phẩm cần đăng nhập để thử một lượt tìm kiếm và một lượt hỏi đáp thật trên prod. Trần 2000 ứng viên mỗi lần tìm có thể bỏ sót kết quả cũ khi kho rất lớn.
