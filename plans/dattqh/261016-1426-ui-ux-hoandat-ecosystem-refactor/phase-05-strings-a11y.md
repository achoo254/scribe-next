---
phase: 5
title: "Từ điển tiếng Việt, rà chuỗi, a11y"
status: pending
priority: P2
effort: "M"
dependencies: [4]
---

# Phase 5: Từ điển, rà chuỗi, a11y

## Tasks
- Chuyển nốt chuỗi hiển thị còn viết cứng trong `web/src/` vào `i18n/messages/vi/*` (Phase 2–4 đã chuyển phần chạm tới). Thông báo lỗi từ backend giữ nguyên (đã là tiếng Việt).
- Kiểm tra tự động: test quét `src/**/*.tsx` tìm chuỗi có dấu tiếng Việt ngoài `i18n/messages` (như cách `check:tokens` quét literal), cho phép danh sách ngoại lệ có lý do.
- Thuật ngữ thống nhất: nhãn "Transcript" (tiếng Anh) — chọn từ Việt cho nhãn tab/tiêu đề (quyết qua Jev), giữ thống nhất toàn app.
- a11y: axe không lỗi serious/critical ở mọi trang; điều hướng bàn phím top bar/menu; vùng chạm ≥ 44 px trên iPhone (`pointer-coarse:`).

## Verification
- vitest (dictionaries + quét chuỗi), Playwright + axe, lint/typecheck/check:tokens; deploy web prod.
