# Nghiệm thu web prod 19/10/2026

Thử trên Chrome (máy Mac) tại scribe.hoandat.com bằng tài khoản free thứ hai của chủ dự án (đăng nhập SSO sẵn có), tệp 2 phút cắt từ cuộc họp Viettel.

| Hạng mục | Kết quả |
|---|---|
| Tải tệp lên (trang Ghi âm) | Đạt: hiện "TỆP TẢI LÊN" trong Lịch sử ghi, dung lượng 967 KB / 200 MB |
| Chặn tài khoản free mới 24 giờ | Đã gỡ theo yêu cầu (`TRANSCRIBE_FREE_DELAY_HOURS=0`, prod 08:33); trước khi gỡ thì báo đúng "dùng được từ 08:29 ngày 20/10" |
| Chép lời ElevenLabs | Đạt: 2 phút xong khoảng 20 giây, trừ 2/120 phút |
| Người nói | Đạt: 4 người, nhãn đổi đúng chỗ; đổi tên áp cho bản chép lời |
| Biên bản | Đạt: dùng tên đã đổi, quyết định và việc cần làm kèm mốc giây |
| Hỏi đáp | Đạt: trả lời "mười tám" kèm nguồn 0:15 |
| Dịch sang Tiếng Anh | **Lỗi rồi sửa**: model gộp câu ngắn (44/47 dòng) → cả lô hỏng. Sửa `4517531`: lệch dòng thì chia đôi lô; prod 08:39; dịch lại đạt (log: 47 → 23+24 → …) |
| Tìm kiếm không dấu | Đạt: "the tin dung" ra câu ở 1:45 |
| Tải xuống | Đạt: DOCX 35 KB, Markdown (gốc và tiếng Anh, có tên người nói và biên bản), VTT |
| Xoá nhanh + Hoàn tác | Đạt (bấm hoàn tác sau 0,8 s, tệp còn trên server sau 7 s); để quá 5 giây thì xoá thật đúng thiết kế |
| Thẻ dùng lượng | Đạt sau tải lại; lúc đổi gói bằng lệnh admin khi trang đang mở thì thẻ giữ số cũ tới lần tải lại |

Chưa thử: Xoá tất cả, liên kết và gửi chat, iPhone (màn hình khoá, quay video, phát nối đoạn) — cần chủ dự án.
