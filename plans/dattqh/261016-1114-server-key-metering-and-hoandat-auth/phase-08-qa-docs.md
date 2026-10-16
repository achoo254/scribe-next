---
phase: 8
title: "QA iPhone thật + docs"
status: pending
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
