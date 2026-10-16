# Journal — đóng plan refactor UI/UX theo hệ sinh thái hoandat (16/10/2026)

**Kết quả.** Năm phase web đã xong và lên prod trong ngày (deploy 15:13, 15:51, 16:43, 17:05). Bản cuối là `index-BodpZ39F.js` (commit 84456a1). Phase 6 đóng mà chưa QA tay trên iPhone, vì chủ sản phẩm không cắm máy nữa.

**Đã làm.** Có khung Localman (top bar, account menu, footer) và data router với URL riêng cho từng trang. Trạng thái ghi và tải lên được giữ khi đổi trang, kèm chặn rời trang khi đang ghi. Giao diện có từ điển tiếng Việt cho mọi chuỗi hiển thị, kèm test chặn chuỗi cứng. Bộ Playwright e2e chụp màn hình sáng/tối và chạy axe.

**Bài học.**
- Script thay chuỗi hàng loạt đã chạy trên diff có lẫn thay đổi chưa commit của agent khác, làm hỏng 12 tệp và một dòng import. Từ nay chỉ thay chuỗi trên cây đã commit và đọc diff trước khi lưu.
- Deploy FE phải giữ `assets/` của bản cũ: rsync không dùng `--delete` cho thư mục đó. Nếu không, tab đang mở sẽ lỗi khi tải chunk lazy. Skill ops đã ghi cách này.
- Lúc đóng plan, Cloudflare tới .65 chập chờn (522/525, 000) ở cả scribe, auth và localman, trong khi gọi ngay trên máy vẫn 200. Lỗi nằm ở đường mạng, không nằm ở app.

**Còn lại.** QA tay trên iPhone với bản cuối: đăng xuất từ account menu, theme sáng/tối, safe area (lệnh ở phase-06). Bước tiếp theo trong lộ trình là gói "Sau cuộc họp", cần plan riêng.
