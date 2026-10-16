---
phase: 3
title: "Màn hình và parity tính năng"
status: in-progress
priority: P1
effort: L
dependencies: [2]
---

# Phase 3: Màn hình và parity tính năng

## Overview

Dựng lại các màn hình bằng primitive của Phase 1 và logic của Phase 2. Bố cục giữ như bản Vue (shell 3 tab, khớp desktop), giao diện theo token của `localman-next`.

## Bảng chuyển đổi

| Vue (`web/src-vue-legacy/`, số dòng) | React (`web/src/`) |
|---|---|
| `views/LoginView.vue` (89) | `pages/login-page.tsx` |
| `views/HomeView.vue` (73) + `components/AppTopBar.vue` (59) | `layout/app-shell.tsx` + `layout/app-top-bar.tsx` (tabs Ghi âm / Cắt file / Transcript "Sắp có") |
| `views/RecordView.vue` (319) | `pages/record/record-page.tsx` + các khối con |
| `views/SplitView.vue` (354) | `pages/split/split-page.tsx` + các khối con |
| `components/RecordingHistory.vue` (336) | `pages/record/recording-history.tsx` |
| `components/MediaPlayer.vue` (225) | `components/media-player.tsx` |
| `components/QuotaBar.vue` (47) | `components/quota-bar.tsx` (dùng `progress-bar`) |
| `components/BrandMark.vue` (42) | `components/brand/wordmark.tsx` (Phase 1) |

Route giữ nguyên: `/` (cần đăng nhập), `/login`, mọi đường khác về `/`.

## Bảng parity (cổng của Phase 4)

| Tính năng | Kiểm |
|---|---|
| Đăng nhập Google, đăng xuất, guard `/` | Chrome + Safari iOS |
| Ghi mic; ghi mic + màn hình + tiếng hệ thống (Chrome desktop); đồng hồ, meter âm lượng, dừng | Ghi thật ≥ 10 phút, tải về phát được |
| Cắt bản ghi thành nhiều bản ghi | Có trong Lịch sử ghi |
| Cắt file: chọn file, upload có tiến độ và resume, tạo job phút/MB/chia đều, tải zip | File ≥ 100 MB, ngắt mạng giữa chừng rồi tiếp |
| Lịch sử ghi: danh sách, phát, tua, tải, tải theo khoảng ngày, xoá | Dữ liệu tạo từ bản Vue |
| Quản lý file, xoá nhiều, quota cập nhật sau khi xoá | QuotaBar giảm |
| Thông báo lỗi tiếng Việt như bản Vue (mic bị chặn, hết quota, mất mạng) | Gây lỗi có chủ đích |

## Yêu cầu UI

- Mọi khối cấp trang, nút, input có `data-id` (tối đa 3 bậc), như quy ước `localman-next`.
- Trạng thái tải theo mục "Loading states" của `localman-next/docs/design-system.md` (skeleton lần đầu, `Button loading`).
- Vùng chạm ≥ 44px trên màn cảm ứng; không cuộn ngang ở 375px.
- WCAG AA: `check:tokens` kiểm cặp màu; focus ring nhìn thấy được.

## Success Criteria

- [ ] Mọi dòng trong bảng parity đạt ở dev local (backend thật chạy local).
- [ ] `lint`, `typecheck`, `test`, `check:tokens`, `build` EXIT 0.

## Môi trường kiểm chứng lab (dựng rồi gỡ 2026-10-15)

Đã dựng thử `scribe-react.tunnel.hoandat.com` trên .65 (pm2 + tunelo client + key riêng) rồi gỡ cùng ngày khi chủ sở hữu chốt chỉ còn dev local và prod: pm2 đã xoá, `/opt/scribe-react-lab` đã xoá, key `scribe-react-lab` đã `revoked`. Bài học còn giá trị: `create-access-token.mjs` của tunelo tạo key thiếu `tenantId` nên server từ chối (`TUNELO_AUTH_001`) cho tới khi gán `tenantId: "primary"`.

## Kết quả kiểm parity ở dev local (2026-10-15, Chrome desktop, BE local + Mongo dev 176)

| Tính năng | Kết quả |
|---|---|
| Đăng nhập Google, guard `/` | Đạt (chủ sở hữu đăng nhập; `/` khi chưa đăng nhập → `/login`) |
| Ghi mic ≥ 10 phút | Đạt: 11:07, 10,3 MB webm/opus, ffprobe 667 s, giải mã tại 10:00; chuyển tab trong lúc ghi không ngắt |
| Cắt bản ghi thành nhiều bản ghi | Đạt: 0,1 phút → 6 phần thay bản gốc trong Lịch sử ghi |
| Cắt file: upload 3 MB và 123 MB, phút/MB/số phần, tải zip | Đạt; **lỗi BE có sẵn**: "Số phần = 2" ra thêm đoạn thứ 3 hỏng 880 byte (`core-split` dùng `-segment_time` = thời lượng/K; sửa bằng `-segment_times` K−1 mốc) |
| Lịch sử ghi: phát, tua, tải đơn, tải theo ngày, tải tất cả | Đạt (tải đơn: header attachment + tên UTF-8 đúng; lượt tải tự động bị Chrome chặn vì tải nhiều file liên tiếp — không phải lỗi app) |
| Quản lý file, xoá, quota giảm | Đạt: xoá upload 123 MB → quota 0,24 → 0,12 GB |
| Chưa kiểm | Ghi kèm màn hình/tiếng hệ thống (cần chọn màn hình thủ công), ngắt mạng giữa chừng rồi tải tiếp (đã có test đơn vị), Safari iOS, đăng xuất |
