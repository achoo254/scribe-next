# Journal: gói "Sau cuộc họp" lên prod (16/10/2026)

**Kết quả.** Gói có ba phần: biên bản bằng `gpt-4.1-mini` (tóm tắt, quyết định, việc cần làm, có mốc giây), đánh dấu khi đang ghi, và liên kết chat.hoandat.com rồi gửi biên bản sang chat riêng. Gói lên prod lúc 22:31; bản vá lên lúc 22:50. Jev quyết sáu điểm thiết kế.

**Bài học.**
- Jev chọn "xác nhận trên web" cho lỗi liên kết hộ, nhưng phương án đó không chặn được tấn công: kẻ tấn công chính là chủ tài khoản Scribe, nên họ tự xác nhận được. Bước xác nhận phải nằm ở phía nạn nhân, tức trong chat. Trước khi gửi phương án cho Jev chọn, cần tự kiểm từng phương án có chặn được đúng mối đe doạ không.
- Handler HTTP gọi LLM sau khi đã giữ chỗ tiền thì phải chạy trong task riêng. Cloudflare cắt ở khoảng 100 giây và client có thể rời trang; future bị huỷ thì sổ chi phí lệch.
- Chỉ hoàn lượt khi nhà cung cấp chắc chắn chưa tính tiền. Nếu không, một tài khoản free có thể đốt hết trần tổng của tháng.
- Trên iPhone, đồng hồ phải lấy từ phía native lúc bấm. Sự kiện từ WKWebView bị dồn lại khi khoá màn hình, nên đồng hồ tự cộng trong JavaScript bị sai.
- Dev và prod dùng chung token bot, nên chỉ prod bật `CHAT_LINK_POLL`.

**Còn lại.** Chủ sản phẩm cần thử đầu cuối việc liên kết và gửi biên bản trên prod, và build lại app iPhone. Đánh dấu từ màn hình khoá hoặc AirPods để đợt có máy. Gói tiếp theo là "Kho bản ghi".
