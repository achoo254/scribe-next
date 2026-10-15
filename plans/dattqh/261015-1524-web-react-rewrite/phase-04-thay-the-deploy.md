---
phase: 4
title: "Thay thế bản Vue, deploy prod, cập nhật plan iOS"
status: pending
priority: P1
effort: S
dependencies: [3]
---

# Phase 4: Thay thế bản Vue, deploy prod, cập nhật plan iOS

## Implementation steps

1. Xoá `web/src-vue-legacy/` sau khi bảng parity đạt.
2. Cập nhật `docs/deploy-web-production.md` (đã đổi tên và chuyển sang mô hình dev local + prod ngày 2026-10-15): mô tả FE là React, build bằng `npm ci && npm run build` trong `web/`, máy build phải có `web/.env` (không có trong git), bỏ mục CI GitLab (project đã archive) hoặc ghi rõ là ngừng dùng.
3. Được chủ sở hữu đồng ý thời điểm, build và deploy prod `.65` theo doc: chỉ thay thư mục dist mà nginx phục vụ, không đụng backend hay box dùng chung khác.
4. Smoke test trên `https://scribe.inetdev.io.vn`: đăng nhập, phát một bản ghi cũ, cắt một file nhỏ.
5. Cập nhật plan iOS `260728-1609-onescribe-ios-app-ghi-nen-khoa-man-hinh`: thay tên file Vue bằng React (`use-auth.ts` → `auth/*`, `MediaPlayer.vue` → `media-player.tsx`, `RecordView.vue` → `record-page.tsx`, `use-download.ts` → `lib/download.ts` đã có…), ghi lớp `apiFetch` + `API_BASE` đã giải quyết đường dẫn tương đối.
6. Cập nhật `README.md`, `docs/codebase-summary.md`, `docs/project-roadmap.md` (mục Web).

## Rollback

Build lại dist từ SHA cuối của bản Vue (ghi ở `plan.md`) và deploy. Backend không đổi nên không cần rollback phía server.

## Success Criteria

- [ ] `web/package.json` không còn `vue`, `vuetify`, `@hobui/*`.
- [ ] Prod chạy bản React; smoke test bước 4 đạt.
- [ ] `git diff --stat server/ deploy/` trống.
- [ ] Plan iOS và docs đã cập nhật, link kiểm tra được.
