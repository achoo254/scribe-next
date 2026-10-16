---
title: Plan viết lại web Scribe bằng React theo design system localman-next
date: 2026-10-15
summary: Lập plan 261015-1524-web-react-rewrite; chặn plan iOS cho tới khi web chạy bản React
---

# Plan viết lại web Scribe bằng React theo design system localman-next

## What happened
- Khi bắt đầu Phase 1 plan iOS, `npm install` trong `web/` lỗi 404 ở `@hobui/ui@1.0.16`: gói private trên npmjs, cả token trong `~/.npmrc` lẫn token trong sổ iCloud đều trả 401. Web không build được trên máy sạch.
- Chủ sở hữu quyết bỏ design system cho app này, dùng design system giống hệt `localman-next` (Radix Colors v3 violet/slate/plum, `tokens.css` là nơi duy nhất chứa mã màu, `check:tokens`), và viết lại web bằng React như `localman-next`.
- Web Vue hiện chỉ 2.622 dòng; phần nặng nhất là `use-recorder.ts` (421 dòng), `SplitView.vue` (354), `RecordingHistory.vue` (336).

## Decision
- Plan `plans/dattqh/261015-1524-web-react-rewrite/`, mức L, 4 phase: khung + token + primitive; lớp API/auth/logic thuần TS; màn hình + bảng parity; thay thế, deploy staging, cập nhật plan iOS.
- Validate: viết đè thẳng trong `web/`, mã Vue chuyển sang `web/src-vue-legacy/` làm bản đọc rồi xoá; giữ tab Transcript "Sắp có"; sáng/tối theo OS có nút chuyển; Inter + JetBrains Mono tự host.
- Lớp `apiFetch` với `API_BASE` giải quyết luôn việc 25 đường dẫn `/api/` tương đối trỏ về `capacitor://localhost` trong app iOS.
- Plan iOS `260728-1609` đặt `blockedBy` plan này, vì Phase 1 iOS sửa đúng các file Vue sắp bị thay.

## Next steps
- Gate 0 iOS (app Capacitor rỗng) vẫn chờ chủ sở hữu tin cậy chứng chỉ Personal Team trên iPhone.
- Bắt đầu Phase 1 của plan React bằng `/ak:cook`.

> Historical work record — not durable authority. Prefer docs/specs/ADRs for current decisions.
