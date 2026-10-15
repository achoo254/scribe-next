---
title: "Viết lại web OneScribe bằng React theo design system của localman-next"
description: "Bỏ Vue + Vuetify + @hobui/viui; dựng lại web bằng React 19 + Vite 7 + Tailwind 4 với tokens.css giống hệt localman-next, giữ nguyên backend và tính năng."
status: in-progress
priority: P1
effort: L
branch: main
tags: [frontend, react, tailwind, design-system, web]
blockedBy: []
blocks: [project:260728-1609-onescribe-ios-app-ghi-nen-khoa-man-hinh]
created: 2026-10-15
---

# Viết lại web OneScribe bằng React

## Bối cảnh và quyết định của chủ sở hữu (2026-10-15)

- Không dùng ViUI cho app này. Dùng design system **giống hệt `localman-next`**: Radix Colors v3 (violet/slate/plum), `tokens.css` là nơi duy nhất chứa mã màu, sáng/tối theo hệ điều hành.
- Viết lại bằng **React** như `localman-next`, không giữ Vue.
- Làm **trước** plan iOS (`260728-1609-onescribe-ios-app-ghi-nen-khoa-man-hinh`): Phase 1 iOS sửa trực tiếp các file Vue, làm iOS trước thì phải làm lại.

## Outcome

`scribe.inetdev.io.vn` chạy bản React có đủ tính năng của bản Vue hiện tại (đăng nhập Google, Ghi âm, Cắt file, Lịch sử ghi + phát lại, quota, xoá), giao diện theo token của `localman-next`, sẵn sàng bọc bằng Capacitor.

## Ràng buộc

- Backend `server/onescribe-be` và nginx **không đổi**. Mọi đường dẫn `/api/*` và hợp đồng request/response giữ nguyên (danh sách ở Phase 2).
- Chỉ `tokens.css` được chứa mã màu; `check:tokens` chặn phần còn lại, như `localman-next`.
- Không phụ thuộc gói riêng tư (`@hobui/*`): `npm ci` phải chạy được trên máy sạch không cần đăng nhập npm.
- Viết đè thẳng trong `web/` (chủ sở hữu chọn). Giữa chừng `web/` có thể không build được; prod (.65) vẫn chạy dist Vue cũ vì deploy làm tay, chỉ deploy ở Phase 4.

## Non-goals

- App desktop Tauri (`src/`) vẫn Vue + ViUI, không đụng.
- Đa ngôn ngữ: chỉ tiếng Việt, không thêm thư viện i18n.
- Tính năng mới (Transcript web, realtime): thuộc plan iOS và plan khác.
- Chuyển CI sang GitHub Actions: GitLab đã archive, deploy prod vẫn làm tay theo `docs/deploy-web-production.md`.

## Phases

| # | Phase | Tier | Trạng thái |
|---|---|---|---|
| 1 | [Khung dự án, token và primitive UI](./phase-01-khung-du-an-token-primitive.md) | M | Done |
| 2 | [Lớp API, auth và logic nghiệp vụ](./phase-02-lop-api-auth-logic.md) | M | Done |
| 3 | [Màn hình và parity tính năng](./phase-03-man-hinh-parity.md) | L | Code xong, chờ QA parity trên backend thật |
| 4 | [Thay thế bản Vue, deploy prod, cập nhật plan iOS](./phase-04-thay-the-deploy.md) | S | Pending |

Thứ tự: **1 → 2 → 3 → 4**.

## Success Criteria (toàn plan)

- [ ] Mọi tính năng trong bảng parity của Phase 3 chạy trên Chrome desktop và Safari iOS (trình duyệt) ở dev local; sau deploy, smoke test trên prod.
- [ ] `npm ci && npm run build` chạy trên máy không có `~/.npmrc`.
- [ ] `npm run lint`, `typecheck`, `test`, `check:tokens` đều EXIT 0.
- [ ] Không còn `vue`, `vuetify`, `@hobui/*` trong `web/package.json`.
- [ ] `git diff --stat server/ deploy/` trống.
- [ ] Plan iOS đã cập nhật tên file React và lớp API mới.

## Rủi ro chính

| Rủi ro | Mức | Giảm thiểu |
|---|---|---|
| Recorder (`use-recorder.ts`, 421 dòng: trộn Web Audio, MediaRecorder timeslice 1s, upload theo offset, wake lock, xử lý gián đoạn) chuyển sai hành vi | Cao | Chuyển thành module thuần TS tách khỏi React, giữ nguyên thuật toán; test đơn vị cho tính offset/retry; QA ghi thật ≥ 10 phút trước khi thay |
| Upload theo phần (`use-upload.ts`, resume theo `/api/upload/status`) sai offset | Trung bình | Module thuần + test đơn vị với fetch giả lập phản hồi status |
| Thiếu tính năng khi thay bản Vue | Trung bình | Bảng parity ở Phase 3 là cổng của Phase 4 |
| Viết đè trong `web/`: giữa chừng không deploy khẩn cấp được bản Vue từ `main` | Đã chấp nhận | SHA cuối của bản Vue: `4e50ea837423c2fe94c43840856bcccf67b9b215` (`web/` đổi lần cuối ở `f852ca8`). Khẩn cấp: `git worktree add /tmp/os-vue 4e50ea8 && cd /tmp/os-vue/web && npm ci && npm run build` (cần `~/.npmrc` cho `@hobui/*`; bản Vue build bằng `--mode staging` nên tạm chép `web/.env` thành `web/.env.staging`) |
| `web/.env` (Firebase config công khai, dùng chung dev local và build prod) không có trong git | Thấp | Ghi rõ trong deploy doc: máy build phải có file này; build báo lỗi rõ nếu thiếu biến |

## Môi trường (chủ sở hữu chốt 2026-10-15)

Chỉ còn **dev local** và **prod** (`.65`, `scribe.inetdev.io.vn`); khái niệm staging đã bỏ. Hệ quả cho plan: bảng parity kiểm ở dev local; Phase 4 là deploy prod, cần chủ sở hữu đồng ý thời điểm; không dựng môi trường xem trước trên `.65` (môi trường tunnel `onescribe-react.tunnel.hoandat.com` dựng thử cùng ngày đã gỡ, key tunelo đã thu hồi).

## Validation Log

### Session 1 — 2026-10-15

| # | Câu hỏi | Chủ sở hữu chọn |
|---|---|---|
| V1 | Dựng bản React ở đâu | Viết đè thẳng trong `web/` (không dùng `web-next/`) |
| V2 | Tab Transcript | Giữ nhãn "Sắp có" |
| V3 | Sáng/tối | Theo hệ điều hành, có nút chuyển |
| V4 | Font | Inter + JetBrains Mono tự host |
