---
phase: 8
title: "QA iPhone thật + docs"
status: completed
priority: P2
effort: "S"
dependencies: [7]
---

# Phase 8: QA iPhone thật + docs

## Tasks

1. QA iPhone (người dùng bấm, tôi đối chiếu log và DB): đăng nhập IdP, ghi khoá màn hình, chép lời không hỏi key, quota giảm đúng, hết quota bị chặn có số, đăng xuất ở `auth.hoandat.com` làm app mất phiên, mở lại đăng nhập được.
2. Gộp QA còn nợ của transcript GĐ1 (tự phát đoạn kế trong WKWebView, mạng chậm).
3. Docs: `docs/deploy-web-production.md` (tên mới, OIDC, key, CLI, đổi DB), `docs/codebase-summary.md` (auth, sessions, usage, plan), `docs/ios-app.md`, `docs/project-roadmap.md`, `README.md`; skill `scribe-ops-prod` (domain, service, db, lệnh mới); xoá dòng "tên prod cũ" trong memory khi xong.
4. Kết thúc plan theo `plan-state-files-first`.

## Kết quả (2026-10-16 14:2x)

- iPhone: đăng nhập IdP (sau khi sửa CSP `form-action` ở hoandat-auth `52f5b11`), ghi, tải lên, chép lời bằng key server, phát lại; sổ phút 16/16 giây (gói plus, không chạm quỹ free).
- Back-channel logout: lần đầu 13:53 timeout (đường server → Cloudflare → server chậm tới ~5 s, oidc-provider chờ mặc định 2,5 s). Sửa hoandat-auth `4c2766c` (`fetch` của provider chờ 10 s, giữ dispatcher chống SSRF). Thử lại 14:2x: `/session/end/confirm` 1,7 s → Scribe `POST /api/auth/backchannel-logout` 200 → phiên web bị xoá, `/api/me` 401, `logout_jtis` 1. Phiên iPhone giữ nguyên (thiết bị không gắn phiên IdP, theo thiết kế).
- Docs: `4f0f095` (deploy guide, codebase summary, ios-app, roadmap, README, skill); dọn tồn đọng `3225368`.
- Còn ngoài phase: báo nhà cung cấp hosting về đường tới Cloudflare chậm và IPv6 không có địa chỉ (người dùng gửi).
