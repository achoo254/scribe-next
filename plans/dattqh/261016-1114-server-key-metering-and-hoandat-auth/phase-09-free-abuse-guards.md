---
phase: 9
title: "Chống lạm dụng gói free (trước khi mở 7c)"
status: completed
priority: P1
effort: "M"
dependencies: [5]
---

# Phase 9: Chống lạm dụng gói free

Người dùng chốt 2026-10-16 ("bộ đầy đủ, giữ quỹ $10"; Jev 0,72–0,79 cho từng biện pháp). Lý do: một người tạo nhiều tài khoản (~28) có thể bào cạn quỹ free $10 trong một ngày, chặn người dùng free thật tới hết tháng. Trần tổng $30 vẫn giữ tiền, nhưng không giữ được trải nghiệm. **7c chỉ bật khi phase này xong.** Mọi biện pháp chỉ áp cho gói `free`.

## Files (`server/scribe-be/src/`)

- `models/usage.rs` — thêm sổ ngày: `d:<userId>:<YYYY-MM-DD>` (giới hạn `TRANSCRIBE_FREE_DAILY_MINUTES`, mặc định 30) và `ip:<ip>:<YYYY-MM-DD>` (giới hạn `TRANSCRIBE_FREE_IP_DAILY_MINUTES`, mặc định 60); giữ chỗ cùng chuỗi `reserve` (thứ tự user tháng → user ngày → IP ngày → quỹ free → tổng), hoàn/quyết toán theo cùng `Charge` (lưu thêm ngày + IP trên doc lượt chép).
- IP: lấy từ `audit::CLIENT_IP` (nginx đặt `X-Real-IP`, Cloudflare `CF-Connecting-IP` đã áp ở cấp `http` trên server — kiểm lại trước khi tin); IPv6 gộp theo /64.
- `models/user.rs` — `firstSeenAt` (đặt lần đăng nhập đầu); free chỉ chép được sau `TRANSCRIBE_FREE_DELAY_HOURS` (mặc định 24) — lỗi `ACCOUNT_TOO_NEW` có giờ mở.
- `models/user.rs` + `auth.rs` + `admin.rs` — `blocked: {at, reason}`; `scribe-be admin block <email> [--reason …]` / `unblock`: chặn đăng nhập (xoá mọi session) và chép lời; audit `admin.block`.
- `notify.rs` (mới) — gửi Telegram cho chủ sản phẩm qua bot riêng (token ở tệp `TELEGRAM_TOKEN_PATH`, 600; chat id là biến build), gửi nền, lỗi chỉ log: quỹ free qua 50% và 80%, trần tổng 80%/100%, một IP hoặc tài khoản chạm giới hạn ngày ≥ 3 lần/ngày, key OpenAI hỏng. Mỗi mốc một lần (`transcribe_marks`).
- `error.rs` — mã `DAILY_LIMIT`, `IP_LIMIT`, `ACCOUNT_TOO_NEW` (403 kèm số / giờ mở); web hiển thị câu trung tính có số.

## Verification

- Test Mongo thật: giới hạn ngày và IP không vượt khi đua; hoàn đúng cả sổ ngày/IP; tài khoản mới bị chặn tới đúng giờ; tài khoản bị chặn mất phiên ngay và không xếp được chép lời; gói `plus` không bị các giới hạn này.
- Dev local: gửi Telegram thử tới chat của chủ sản phẩm (người dùng xác nhận đã nhận).
- Không lộ token Telegram trong log/audit.

## Kết quả (2026-10-16 13:12, chưa deploy)

- Làm khác plan: tuổi tài khoản tính từ `createdAt` (user mới đã có sẵn), không thêm `firstSeenAt`. Sổ ngày/IP nằm trong cùng chuỗi `Charge::books()` của giữ chỗ/hoàn/quyết toán; doc lượt lưu `day`/`ip`.
- Thêm `TOO_LONG_FOR_FREE` (review M2): lượt dài hơn giới hạn ngày không bao giờ chạy được thì báo riêng, không hẹn giờ.
- Review `plans/dattqh/reports/code-reviewer-261016-1310-free-abuse-guards.md`: đã sửa H1 (worker bỏ qua tài khoản bị khoá), M1 (quyết toán đi hết các sổ dù một sổ lỗi), M2, các mục thấp (`blocked: true` sửa tay, timeout 10 giây cho Telegram, tên trường log, dùng chung múi giờ). Để lại: M3 (IPv6 /64 dễ né, CGNAT dễ chặn nhầm — chính sách, chờ người dùng), TTL cho doc theo ngày (nhỏ), `recompute` không sửa sổ ngày/IP (sổ tự hết hạn theo ngày).
- Kiểm: clippy `-D warnings` sạch, 33 test thường, 5 test Mongo thật (đua 6 lượt chỉ 3 lượt qua giới hạn ngày; IP; plus không bị giới hạn; hoàn trả cả sổ ngày/IP; khoá/mở khoá), E2E dev 14/14 (tài khoản mới, DAILY_LIMIT, TOO_LONG_FOR_FREE, IP_LIMIT, IP khác, plus, `admin block` xoá phiên, phiên mới bị 401, audit). Lượt giả dừng ở ffmpeg, không gọi OpenAI.
- Chưa: bot Telegram riêng + chat id (người dùng cung cấp), gửi thử `admin notify-test`.

## Bỏ giới hạn theo IP (2026-10-16 13:11)

Người dùng chốt "bỏ quy tắc IP" (Jev 0,75 cho phương án này; giữ 60 phút 0,01, nâng 180 phút 0,23, gộp /56 0,01): người dùng 4G sau NAT nhà mạng chung IPv4 bị chặn nhầm, còn IPv6 /64 dễ né (review M3). Đã gỡ sổ `ip:`, mã `IP_LIMIT`, biến `TRANSCRIBE_FREE_IP_DAILY_MINUTES`, trường `ip` trên doc lượt. Tiền vẫn giữ bởi giới hạn ngày mỗi tài khoản, tài khoản mới chờ 24 giờ, quỹ free $10, trần $30, báo Telegram và `admin block`. Kiểm lại: clippy sạch, 33 test, 5 test Mongo thật, E2E dev 12/12 (ba tài khoản cùng IP đều chép được).
