# Journal: bước 2 — key server, sổ phút, hoandat-auth, mở chép lời (2026-10-16)

Plan `plans/dattqh/261016-1114-server-key-metering-and-hoandat-auth/` hoàn tất trong một ngày, 9 phase.

## Kết quả
- Chép lời bằng key OpenAI của server, sổ phút giữ chỗ trước rồi quyết toán theo giây thật đã gửi; trần $10 quỹ free, $30 tổng.
- Đăng nhập chuyển từ Firebase sang hoandat-auth (web confidential + iPhone PKCE), phiên phía server, back-channel logout; domain `scribe.hoandat.com`; db/service/thư mục đổi tên `scribe`, bản cũ và domain cũ đã xoá.
- Chống lạm dụng gói free (30 phút/ngày, chờ 24 giờ, `admin block`, báo qua bot chat.hoandat.com); bỏ giới hạn theo IP; bỏ mảnh hoàn toàn im lặng (không tính phút).
- 7c mở chép lời cho mọi người lúc 14:19 theo quyết định của chủ sản phẩm.

## Bài học (đã kiểm bằng log/đo)
- hoandat-auth với code flow chỉ trả `email`/`email_verified` ở `userinfo`, không ở ID token — tài liệu IdP từng ghi sai; mọi đăng nhập bị coi là chưa xác minh.
- CSP `form-action` áp cả cho chuyển hướng sau POST form: thiếu scheme riêng (`com.hoandat.scribe:`) thì WebKit chặn quay về app sau trang đồng ý.
- Server .65 gọi các app của chính nó qua Cloudflare, đường đi chậm tới ~5 s; mức chờ 2,5 s mặc định của oidc-provider làm rơi back-channel logout. Giả thuyết IPv6 đầu tiên sai — đã đo IPv4 và Node để bác bỏ rồi revert.
- oidc-provider v9 đổi `httpOptions` thành `fetch`; runtime không báo lỗi với khoá lạ nên phải đọc tài liệu đúng phiên bản.
