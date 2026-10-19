# Review xoá tài khoản và trang pháp lý (b5c5bd6)

Đọc mã, không chạy test. Không Critical/High. Phân quyền (chỉ xoá chính mình), CSRF, giới hạn đường dẫn đĩa, độ phủ các collection, trần chi phí: đạt.

## Medium
- **M1** `account.rs:272-291`, `account.rs:201`, `auth.rs:139`: xoá bị ngắt hoặc lỗi giữa chừng báo thất bại nhưng vẫn chạy tiếp sau; người dùng đăng nhập lại trong lúc đó bị xoá theo. Sửa: chạy tách khỏi request, trả "đang xoá", chặn đăng nhập khi có dấu xoá.
- **M2** `account.rs:61`, `realtime_token.rs:160-170`: xoá token chép trực tiếp chưa quyết toán làm `admin usage --recompute` lệch. Sửa: ẩn danh hoá như các sổ khác.
- **M3** `RecorderPlugin.swift:218-227`: đoạn ghi còn chờ trên iPhone sống sót, tải vào tài khoản mới khi đăng nhập lại. Sửa: xoá kho đoạn khi thiết bị bị thu hồi / tài khoản bị xoá.
- **M4** `account-api.ts:33-36`: DELETE có thân JSON chưa từng chạy qua CapacitorHttp; nếu mất thân thì iPhone không xoá được (vi phạm 5.1.1(v)).

## Low
L1 nhật ký còn `chatId` và `error` có tên tệp · L2 thu hồi thiết bị ở IdP không thử lại · L3 phiên đã xoá còn dùng được ~30 s từ cache · L4 quét toàn collection mỗi giờ (thiếu index `userId`) ·
L5 test purge bị `#[ignore]` · L6 xoá rồi đăng ký lại được phút mới (Jev: giữ) · L7 chính sách: nhà cung cấp chữ theo cấu hình, bản sao lưu, sổ ẩn danh giữ vô thời hạn, quyền lưu vào Ảnh · L8 kiểm tiền tố đường dẫn khi xoá tệp.
