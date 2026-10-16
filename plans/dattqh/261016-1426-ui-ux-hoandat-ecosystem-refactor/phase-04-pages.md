---
phase: 4
title: "Dựng lại các trang theo pattern Localman"
status: pending
priority: P1
effort: "L"
dependencies: [3]
---

# Phase 4: Dựng lại các trang

## Pattern áp dụng (nguồn `localman-next/apps/web` + `docs/design-system.md`)
- Tiêu đề trang + mô tả ngắn + hành động chính; card bo 24 (`Card`), dùng `Card` thay class inline (4 chỗ); empty state (`empty-state.tsx`); loading bằng `use-delayed-flag` + skeleton; toast cho kết quả; lỗi inline trong form.
- Bảng/danh sách: hàng có hành động bằng `dropdown-menu` thay dãy nút khi chật (iPhone).

## Trang
- **Đăng nhập** (`pages/login-page.tsx`): theo trang đăng nhập hoandat-auth (card giữa, nền grid như `hoandat-auth/static/css/app.css` 13–23 nếu tái dùng được bằng token), một nút "Đăng nhập bằng hoandat", lỗi từ `login-error.ts`.
- **Ghi âm** (`pages/record/*`): card ghi (nguồn, nút ghi lớn, đồng hồ), lịch sử ghi dạng danh sách có menu hành động.
- **Cắt file** (`pages/split/*`): bước chọn tệp → tiêu chí → kết quả; `select` cho tiêu chí.
- **Transcript** (`pages/transcript/*`): danh sách + trang chi tiết `/transcripts/:key`; gộp hai widget quota (`components/quota-bar.tsx`, `pages/transcript/transcription-quota.tsx`) thành một nếu cùng dữ liệu (đọc cả hai trước); chỉ đổi khung/khoảng cách của `transcript-reader.tsx`, không tách file (non-goal).
- Chữ mono chỉ cho wordmark/badge/tab theo Localman; tiêu đề trang dùng Inter nếu Localman làm vậy (so ảnh).

## Verification
- Playwright ảnh chụp từng trang light/dark, desktop + iPhone, đặt cạnh ảnh Localman tương ứng trong báo cáo `plans/dattqh/reports/`; axe.
- vitest các trang chạm tới; deploy web prod sau mỗi nhóm trang (đăng nhập + ghi; cắt; transcript) nếu muốn chia nhỏ.
