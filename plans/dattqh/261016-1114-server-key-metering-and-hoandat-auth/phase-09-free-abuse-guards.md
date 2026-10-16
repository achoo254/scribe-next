---
phase: 9
title: "Chống lạm dụng gói free (trước khi mở 7c)"
status: pending
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
