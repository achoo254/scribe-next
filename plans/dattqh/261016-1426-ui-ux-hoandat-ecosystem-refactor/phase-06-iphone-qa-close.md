---
phase: 6
title: "QA iPhone + đóng plan"
status: pending
priority: P1
effort: "S"
dependencies: [5]
---

# Phase 6: QA iPhone + đóng plan

## Tasks
- Cài app qua cáp (`IOS_DEVICE=<udid> pnpm run ios:device`), người dùng bấm, tôi đối chiếu log/DB: đăng nhập, ghi khi khoá màn hình, có cuộc gọi đến (nếu có máy thứ hai), tải lên, chép lời, mở transcript, đăng xuất từ menu, theme sáng/tối, safe area (notch, home indicator).
- Docs: `docs/codebase-summary.md` (layout, routes, providers, i18n), `docs/ios-app.md` (giao diện mới), `README.md` (`test:e2e`), skill `scribe-ops-prod` nếu lệnh build/verify đổi.
- Đóng plan; journal.

## Verification
- Bằng chứng log + DB cho từng mục QA; mọi cổng kiểm tra đạt trên commit cuối đã deploy.
