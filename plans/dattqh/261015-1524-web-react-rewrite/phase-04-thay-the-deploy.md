---
phase: 4
title: "Thay thế bản Vue, deploy prod, cập nhật plan iOS"
status: done
priority: P1
effort: S
dependencies: [3]
---

# Phase 4: Thay thế bản Vue, deploy prod, cập nhật plan iOS

## Implementation steps

1. Xoá `web/src-vue-legacy/` sau khi bảng parity đạt.
2. Cập nhật `docs/deploy-web-production.md` (đã đổi tên và chuyển sang mô hình dev local + prod ngày 2026-10-15): mô tả FE là React, build bằng `npm ci && npm run build` trong `web/`, máy build phải có `web/.env` (không có trong git), bỏ mục CI GitLab (project đã archive) hoặc ghi rõ là ngừng dùng.
3. Được chủ sở hữu đồng ý thời điểm, build và deploy prod `.65` theo doc: chỉ thay thư mục dist mà nginx phục vụ, không đụng backend hay box dùng chung khác.
4. Smoke test trên `https://$DEPLOY_DOMAIN`: đăng nhập, phát một bản ghi cũ, cắt một file nhỏ.
5. Cập nhật plan iOS `260728-1609-scribe-ios-app-ghi-nen-khoa-man-hinh`: thay tên file Vue bằng React (`use-auth.ts` → `auth/*`, `MediaPlayer.vue` → `media-player.tsx`, `RecordView.vue` → `record-page.tsx`, `use-download.ts` → `lib/download.ts` đã có…), ghi lớp `apiFetch` + `API_BASE` đã giải quyết đường dẫn tương đối.
6. Cập nhật `README.md`, `docs/codebase-summary.md`, `docs/project-roadmap.md` (mục Web).

## Rollback

Build lại dist từ SHA cuối của bản Vue (ghi ở `plan.md`) và deploy. Backend không đổi nên không cần rollback phía server.

## Success Criteria

- [ ] `web/package.json` không còn `vue`, `vuetify`, `@hobui/*`.
- [ ] Prod chạy bản React; smoke test bước 4 đạt.
- [ ] `git diff --stat server/ deploy/` trống.
- [ ] Plan iOS và docs đã cập nhật, link kiểm tra được.

## Kết quả deploy (2026-10-15 21:14)

- Jev quyết deploy (0,59) theo yêu cầu "làm nốt việc còn lại" của chủ sở hữu. Deploy tay (repo ở GitHub, không có CI): `web/dist` (commit `bc14254`) lên `$DEPLOY_WEB_ROOT/dist`, kèm backend cùng commit (có chép lời và bản sửa cắt "Số phần").
- Sao lưu trước khi thay: bản Vue + binary cũ ở `/root/<app>-backup-20261015-211433/` trên `.65`. Quay lại: `rsync -a --delete <thư mục sao lưu>/dist/ $DEPLOY_WEB_ROOT/dist/`.
- Kiểm: mọi asset trong `index.html` trả 200; `/record`, `/cut` trả SPA; `index.html` trên prod khớp bản build; bundle có cấu hình Firebase; trang đăng nhập hiện đúng trong Chrome, console không lỗi; backend `active`, 0 lần khởi động lại.
- Chưa kiểm: đăng nhập và thao tác thật trên prod (cần tài khoản của chủ sở hữu).
