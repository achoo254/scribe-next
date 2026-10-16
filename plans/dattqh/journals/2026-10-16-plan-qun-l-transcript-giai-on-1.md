---
title: Plan quản lý transcript giai đoạn 1
date: 2026-10-16
summary: "Transcript theo phiên là lớp trên kết quả chép theo file; red-team 19 phát hiện, Jev validate sẵn sàng 0,82"
---

# Plan quản lý transcript giai đoạn 1

## What happened
- Lập plan `plans/dattqh/261016-0325-transcript-management-phase-1/` (4 phase: backend M, kiểm dữ liệu cũ S, tab Transcript M, QA + deploy S).
- Hàng đợi chép lời, key OpenAI trong RAM, `runId` và việc xoá đều gắn `transcripts._id = fileId`; đoạn iPhone tới dần từng phút. Vì vậy transcript phiên là collection mới `transcript_docs` (tiêu đề + câu đã sửa) đặt trên kết quả theo file, tính lúc đọc.
- Red-team 3 reviewer: 28 phát hiện, gộp còn 19 (1 Critical, 8 High, 10 Medium), chấp nhận cả 19. Nặng nhất: `AudioPlayer` không export và không đổi đoạn kèm tua được; phần sửa chưa gắn `runId`; `delete_doc(id)` không tính được khoá phiên; "Chép lại" chép cả phiên; poll kéo toàn bộ câu.

## Decision
- Phần sửa lưu `{runId, texts}`, chỉ áp khi `runId` và số câu khớp; sweep mồ côi mỗi giờ; khoá `s:`/`f:`; route mới body 1 MiB, index tạo đồng bộ.
- Jev: cắt bản ghi mất transcript không cảnh báo (0,83); chép lại từng đoạn + cả phiên (1,00); không phát nền khi khoá màn hình (1,00); đọc/sửa/xoá dữ liệu của mình chỉ cần đăng nhập (1,00); giữ Phase 2 chỉ đọc (0,99); plan sẵn sàng 0,82.

## Next steps
- `/ak:cook` Phase 1 khi người dùng đồng ý; Phase 2 chạy audit chỉ đọc trước deploy.

> Historical work record — not durable authority. Prefer docs/specs/ADRs for current decisions.
