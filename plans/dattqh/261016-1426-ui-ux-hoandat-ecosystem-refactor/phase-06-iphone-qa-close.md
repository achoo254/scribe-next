---
phase: 6
title: "QA iPhone + đóng plan"
status: completed
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

## Kết quả (16/10/2026)
- Web: hoàn thành. Prod phục vụ `assets/index-BodpZ39F.js` (commit 84456a1), `/api/health` ok, `/api/me` 401 khi chưa đăng nhập. Cổng kiểm tra trên commit đó: lint sạch, typecheck ok, check:tokens ok, vitest 48 tệp / 242 test, build + check:dist ok, Playwright e2e 54 đạt / 4 bỏ qua.
- iPhone: QA tay **hoãn** theo yêu cầu của chủ sản phẩm (không cắm máy nữa). Đã kiểm trên iPhone trong Phase 3: một phiên ghi 3 đoạn (60 s, 60 s, 48 s) tải lên và chép lời đủ. Chưa kiểm: đăng xuất từ account menu, theme sáng/tối, safe area trên bản cuối. Lần cắm máy sau: `IOS_DEVICE=<udid> pnpm run ios:device` rồi chạy lại danh sách ở trên.
- Docs đã cập nhật ở commit 0cb1788.
