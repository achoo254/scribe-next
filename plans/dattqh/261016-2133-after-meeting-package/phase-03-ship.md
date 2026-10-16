---
phase: 3
title: "Cổng kiểm tra, deploy, docs"
status: pending
effort: "M"
---

# Phase 3: Ship

- Review code (code-reviewer) cho cả backend và web, rồi sửa các phát hiện.
- Chạy toàn bộ cổng một lần trên commit cuối.
- Deploy prod theo skill `scribe-ops-prod`:
  - điều kiện: hàng đợi 0, `recording_sessions` 0, `upload_sessions` 0;
  - sao lưu binary và dist;
  - FE giữ `assets/` cũ.
- Kiểm sau deploy: health, NRestarts, journal không có ERROR; một lượt tóm tắt, liên kết và gửi thật trên prod.
- Docs: `docs/codebase-summary.md` (route, collection mới, task long-poll), `docs/deploy-web-production.md` (biến build mới), `docs/project-roadmap.md`; journal.
- Đợt sau khi có iPhone: đánh dấu từ màn hình khoá hoặc AirPods (Now Playing remote command).
