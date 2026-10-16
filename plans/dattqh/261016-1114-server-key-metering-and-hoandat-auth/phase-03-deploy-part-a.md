---
phase: 3
title: "Deploy phần A"
status: pending
priority: P1
effort: "S"
dependencies: [1, 2]
---

# Phase 3: Deploy phần A

Theo skill `scribe-ops-prod` §6. Thao tác ghi prod: chỉ khi người dùng đồng ý cho lần deploy này.

## Tasks

1. Người dùng cung cấp key OpenAI của mình qua đường không hiện giá trị (tệp tạm 600 hoặc trình quản lý mật khẩu); tôi không in, không lưu vào repo.
2. Trên `.65`: ghi key vào `$DEPLOY_APP_DIR/.openai-key` (chủ `$DEPLOY_USER`, 600) bằng stdin; `OPENAI_KEY_PATH` trong `.env.production` trỏ tới đó.
3. Hàng đợi rỗng, sao lưu `/root/onescribe-backup-<ts>/` (binary, dist, dump `transcripts` + `users`).
4. Build + deploy; kiểm `/api/health`, log "chép lời bật" (không có giá trị key), `/api/me` của một user được phép có `transcription`.
5. `scribe-be admin set-plan <email> plus` cho từng email trong `TRANSCRIBE_EMAILS` (danh sách lấy từ `.env.production`, không in ra báo cáo ngoài số lượng).
6. QA nhanh: chép một bản ghi ngắn trên web, sổ phút tăng đúng; journal sạch 30 phút.
7. Cập nhật `docs/deploy-web-production.md` (tệp key, CLI admin, bỏ bước "chờ hàng đợi rỗng" vì restart không còn làm hỏng lượt — giữ ghi chú lượt đang chạy sẽ chạy lại), skill `scribe-ops-prod`.

## Rollback

Binary + dist cũ từ thư mục sao lưu; sổ `transcribe_usage` và trường mới bản cũ bỏ qua. Bản cũ lại đòi key người dùng.

## Kết quả (2026-10-16 11:53)

- Key server ghi bằng stdin vào `OPENAI_KEY_PATH` (600, chủ user service); `.env.production` thêm `OPENAI_KEY_PATH`.
- Sao lưu `/root/onescribe-backup-20261016-115313/` (binary, dist, dump `transcripts` + `users`); hàng đợi = 0.
- Deploy commit `a90a7e8`: sha256 binary khớp (`0b4d4576…`), service active, `/api/health` OK, journal "chép lời bật (key server)", FE `index-8BNOtOKg.js`, `NRestarts=0`.
- `admin set-plan … plus` cho email trong `TRANSCRIBE_EMAILS`: 1/1 thành công.
- `admin usage`: quỹ free 0/3333 phút, tổng 0/10000 phút.
- Chưa: chép một bản thật trên prod (cần người dùng bấm, app iPhone đang cài còn bản cũ — xem review #7), journal 30 phút.
