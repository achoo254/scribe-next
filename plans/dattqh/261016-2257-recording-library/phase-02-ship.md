---
phase: 2
title: "Review, deploy, docs"
status: pending
effort: "M"
---

# Phase 2
- Review (code-reviewer, chỉ đọc), sửa các phát hiện.
- Deploy theo skill `scribe-ops-prod`: hàng đợi và phiên đều 0, sao lưu, giữ `assets/`.
- Kiểm: health; log backfill; tìm kiếm và hỏi đáp thật trên prod với bản ghi hiện có.
- Docs: `docs/codebase-summary.md`, `docs/deploy-web-production.md` (biến build `ASK_*`), `docs/project-roadmap.md`; journal.

## Review 936e9fb (16/10/2026 23:35): chưa sửa, chưa deploy
- **H1.** Xoá transcript hoặc bản ghi chỉ dọn index trong task nền. Task lỗi hoặc tiến trình bị restart thì câu đã xoá vẫn tìm được và vẫn bị gửi cho `/api/ask`. Sửa: `delete_many` đồng bộ ở `transcribe/docs.rs:472` và `models/file.rs:244`; thêm quét câu mồ côi vào `retention::sweep_once`.
- **M1.** Thay câu của phiên không nguyên tử (`models/sentence.rs:101-126`). Sửa: dấu thế hệ `gen`, cộng đối soát định kỳ (phiên có transcript done mà không có câu nào).
- **M2.** Đoạn iPhone tới muộn làm lệch `at`. Sửa: gọi `search::index::schedule` sau `file::create` ở `recordings.rs:220` và `upload.rs:202`.
- **M3.** Giới hạn 2000 ứng viên lấy theo thứ tự khoá, nên bỏ sót phiên mới. Sửa: thêm `createdAt` vào doc câu, sort giảm dần trước `limit`, index `{userId, createdAt}`; đòi ít nhất 2 ký tự cho tiền tố.
- **M4.** `session_meta` đọc mọi bản ghi ở mỗi request. Sửa: chỉ đọc các khoá cần, có projection.
- **M5 (cần Jev).** Hỏi trên bản ghi dài (tới 120k ký tự, khoảng 535 giây mỗi câu) làm cạn trần tháng free 3600 giây. Hướng: hạ `FULL_CONTEXT_CHARS` cho free hoặc tách trần riêng cho hỏi đáp.
- **Low.**
  - L1: tạo text index đồng bộ ở `main.rs`.
  - L2: cờ backfill khi chỉ một phiên lỗi.
  - L3: gộp các task dựng lại khi retention xoá hàng loạt.
  - L4: `?t=` đổi khi không remount.
  - L5: highlight khi `keepPreviousData`.
