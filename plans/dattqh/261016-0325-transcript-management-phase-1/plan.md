---
title: "Quản lý transcript — giai đoạn 1 (xem và sửa)"
description: "Mở tab Transcript: mỗi phiên ghi là một transcript có tiêu đề, đọc kèm audio (chạm câu để tua), sửa câu, đổi tên, xoá."
status: in-progress
priority: P1
effort: "L (4 phase: M, S, M, S)"
branch: main
tags: [backend, frontend, ios, api, database]
blockedBy: []
blocks: []
created: 2026-10-16
---

# Quản lý transcript — giai đoạn 1 (xem và sửa)

> **Rà trạng thái (2026-10-19):** phase 1–3 xong, đã deploy prod 2026-10-16 11:07 (phase 4); `transcript_docs` và tab Transcript
> đang chạy và là nền cho biên bản, dấu mốc, dịch. **Còn lại, nên giữ in-progress:** QA trên iPhone thật (mục 5–6 phase 4: chạm câu tua đúng đoạn,
> tự sang đoạn kế trong WKWebView, sửa/chép lại/đổi tên/xoá, xoá cache khi đăng xuất); xoá bản sao lưu `/root/onescribe-backup-20261016-110644/` sau 2026-10-30 (hỏi trước).

## Overview

Mở tab Transcript (đang khoá "Sắp có"): mỗi **phiên ghi** là một transcript có tiêu đề, đọc kèm audio (chạm câu để tua), sửa câu, đổi tên, xoá. Thiết kế cho màn hình iPhone; web dùng chung mã. Chỉ người dùng đang được phép chép lời (`canTranscribe`) thấy tab.

Quyết định Jev (2026-10-16, xem `../reports/brainstorm-261016-0809-scribe-target-architecture-refactor.md`): một transcript cho mỗi phiên ghi (0,89) · xoá cùng audio như hiện nay (0,98) · làm ngay cho nhóm email được phép (0,83) · thiết kế cho iPhone, web dùng chung mã (0,81).

## Thiết kế chính

**Chép lời vẫn chạy theo từng file; transcript phiên là lớp phía trên.** Hàng đợi, key OpenAI trong RAM, `runId` và việc xoá theo file đều gắn `transcripts._id = fileId` (`server/scribe-be/src/models/transcript.rs:21-177`, `transcribe/mod.rs:56-93`), và đoạn iPhone tới server dần từng phút. Vì vậy:

- Collection `transcripts` (kết quả máy, mỗi file một doc) **giữ nguyên**.
- Collection mới **`transcript_docs`**: một doc cho mỗi phiên, giữ phần người dùng tạo ra (tiêu đề, câu đã sửa). Tạo lười khi người dùng đổi tên hoặc sửa lần đầu.
- **Khoá phiên** (`key`): tên file dạng `<sessionId>#<index>` (iPhone, `web/ios/App/App/Recorder/UploadQueue.swift:8`) → `sessionId`; bản ghi trình duyệt (một file) → id file. Cùng regex với web (`web/src/features/files/recording-groups.ts:110`).
- Danh sách và chi tiết transcript **tính lúc đọc** từ `files` + `transcripts` + `transcript_docs` (không lưu trùng).

```
files (recording)        transcripts (máy, theo file)      transcript_docs (người dùng, theo phiên)
 <sid>#0 ─┐               _id=file0  sentences[]  ─┐        { userId, key="s:<sid>", title?,
 <sid>#1 ─┼─ key=s:<sid> ─ _id=file1  sentences[]  ─┼──────    edits: { file1: {runId, texts} } }
 <sid>#2 ─┘               _id=file2  (running)    ─┘
                                   │
            GET /api/transcript-docs/s:<sid> → các đoạn theo index, offset cộng dồn,
            câu = edits[file].texts nếu runId khớp lượt chép hiện tại, ngược lại câu máy
```

## Phases

| # | Phase | Tier | Status | Phụ thuộc |
|---|-------|------|--------|-----------|
| 1 | [Backend: transcript theo phiên + API](./phase-01-backend-transcript-docs.md) | M | Done | — |
| 2 | [Dữ liệu cũ: kiểm tra chỉ đọc trước deploy](./phase-02-legacy-data-audit.md) | S | Done | 1 |
| 3 | [Web/iPhone: tab Transcript, đọc, sửa, đổi tên, xoá](./phase-03-transcript-tab-ui.md) | M | Done | 1 |
| 4 | [QA iPhone, deploy prod, docs](./phase-04-qa-deploy-docs.md) | S | Đã deploy 16/10; chờ QA iPhone thật | 2, 3 |

## Non-goals

Tìm kiếm, thư mục/nhãn, chia sẻ link, xuất docx (giai đoạn 2–3); tóm tắt AI; nhận diện người nói; giữ transcript lâu hơn audio; mở chép lời cho người ngoài danh sách; đổi đăng nhập hay tính phút.

## Success Criteria

- [ ] Người dùng được phép thấy tab Transcript; người khác không thấy.
- [ ] Phiên iPhone 49 đoạn hiện là **một** transcript, câu đúng thứ tự, mốc thời gian cộng dồn đúng (so với độ dài từng đoạn).
- [ ] Chạm một câu → audio phát đúng đoạn, đúng giây (lệch ≤ 1 s); hết đoạn tự sang đoạn sau.
- [ ] Sửa câu → lưu → tải lại vẫn thấy bản sửa; xuất txt/srt dùng bản sửa.
- [ ] Đổi tên → danh sách transcript và màn hình đọc hiện tên mới; xoá tên → về tên mặc định.
- [ ] Xoá transcript → audio còn nguyên; xoá audio (tay, hết 14 ngày, hoặc cắt bản ghi — chấp nhận, không cảnh báo theo Jev) → transcript và `transcript_docs` cũng mất (sweep mồ côi đảm bảo trong ≤ 1 giờ).
- [ ] Chép lại một đoạn đã sửa (nút trên từng đoạn) → có xác nhận, chỉ đoạn đó về câu máy mới; chép lại cả phiên → xác nhận nêu đúng số đoạn/câu đã sửa sẽ mất.
- [ ] Phần sửa không bao giờ áp lên lượt chép khác (`runId`), kể cả khi lưu và chép lại chen nhau.
- [ ] Đoạn thiếu hoặc chưa chép hiện rõ trong màn hình đọc; có nút chép các đoạn còn lại.
- [ ] Đăng xuất xoá cache audio trên iPhone; `audit_events` không chứa tiêu đề hay câu.
- [ ] `cargo clippy -D warnings`, `cargo test --workspace`, `npm run lint/typecheck/check:tokens/test` sạch; CI GitHub xanh.

## Rủi ro chính

| Rủi ro | Giảm thiểu |
|---|---|
| Phần sửa lệch câu khi chép lại | Phần sửa gắn `runId`; PUT ghi có điều kiện; đọc/xuất chỉ áp khi `runId` và số câu khớp |
| `transcript_docs` sót lại sau khi audio bị xoá | `delete_doc(&FileDoc)` dọn ngay; sweep mồ côi mỗi giờ là cơ chế đảm bảo |
| Trình phát không đổi đoạn kèm tua được | Sửa `media-player.tsx`, `useMediaSource`, gộp tải trùng trong `NativeFilesPlugin.swift` (Phase 3) |
| Poll nặng khi chép phiên dài | Danh sách dùng projection; reader poll `/status` nhẹ, chỉ khi tab mở |
| Deploy làm hỏng lượt chép đang chạy | Chờ hàng đợi rỗng trước restart (Phase 4) |
| Thay đổi prod | Sao lưu `transcripts` ngay trước deploy, có hạn xoá; người dùng đồng ý trước |

## Red Team Review

### Session — 2026-10-16
**Findings:** 28 từ 3 reviewer (Security Adversary, Failure Mode Analyst, Assumption Destroyer), gộp còn 19; 19 accepted, 0 rejected (đều có trích dẫn file:line).
**Severity breakdown (sau gộp):** 1 Critical, 8 High, 10 Medium

| # | Finding | Severity | Disposition | Applied To |
|---|---------|----------|-------------|------------|
| 1 | Trình phát không đổi đoạn kèm tua, không có onEnded/onTimeUpdate (`media-player.tsx:116,162-164`, `download.ts:57-65`) | Critical | Accept | Phase 3 |
| 2 | Phần sửa không gắn `runId`, đua với chép lại (`transcript.rs:59-80`, `transcribe/mod.rs:137-145`) | High | Accept | Phase 1, 3 |
| 3 | `delete_doc(id)` không tính được khoá phiên; retention bỏ qua lỗi → `transcript_docs` mồ côi (`file.rs:227-234`, `retention.rs:64-66`) | High | Accept | Phase 1 |
| 4 | Chép lại cả phiên làm mất mọi phần sửa, không có chép lại từng đoạn (`transcript-panel.tsx:69-82`) | High | Accept | Phase 3, Success Criteria |
| 5 | Poll 2 s kéo toàn bộ câu; danh sách không projection (`transcript.rs:145-174`, `rate_limit.rs:19-23`) | High | Accept | Phase 1, 3 |
| 6 | Tải trước trùng lượt + `removeItem` đua; cache iPhone không bao giờ dọn (`NativeFilesPlugin.swift:27-47,113-120`) | High | Accept | Phase 3 |
| 7 | Rollback/sao lưu: không dump ngay trước deploy, lệnh restore sai (`docs/deploy-web-production.md`) | High | Accept | Phase 4 |
| 8 | Restart lúc deploy giết lượt chép đang chạy (`transcribe/mod.rs:274-279`) | Medium | Accept | Phase 4 |
| 9 | `key`/`fileId` chưa validate trước field path/upsert; body limit thực là 8 MiB (`routes.rs:32-35`) | Medium | Accept | Phase 1 |
| 10 | Index unique có thể chưa tạo khi upsert đua (`main.rs:55-70`) | Medium | Accept | Phase 1 |
| 11 | Export chế độ `ids` không áp sửa; gom phiên hai cách khác nhau (`transcribe/mod.rs:248-269`) | Medium | Accept | Phase 1 |
| 12 | Quyền không nhất quán giữa route cũ và mới (`transcribe/mod.rs:180-210`) | Medium | Accept | Phase 1 |
| 13 | Audit 180 ngày có thể chứa nội dung transcript (`audit.rs:15-16,54-60`) | Medium | Accept | Phase 1 |
| 14 | Offset sai khi thiếu đoạn / `durationS=0` (`transcribe/mod.rs:221-227`, `recordings.rs:207-209`) | Medium | Accept | Phase 1, 3 |
| 15 | Thiếu trạng thái "chưa chép" cho đoạn tới muộn (`transcript-api.ts:3`, `UploadQueue.swift:6-8`) | Medium | Accept | Phase 1, 3 |
| 16 | Tên mặc định nằm ở `session-row.tsx:23-32`, không có ngày | Medium | Accept | Phase 3 |
| 17 | Phase 2 dựa trên giả định yếu và có thao tác xoá prod (`transcript.rs:20-39,84-93`) | Medium | Accept | Phase 2 |
| 18 | Bản sao lưu prod không có hạn xoá | Medium | Accept | Phase 4 |
| 19 | Cắt bản ghi cũng xoá transcript và phần sửa (`jobs.rs:263-277`) | Medium | Accept (ghi vào tiêu chí; Jev: không cảnh báo) | Success Criteria |

## Validation Log

### Session 1 — 2026-10-16 (Jev, `jev-latest`)
State: toàn bộ `plan.md` + 4 phase sau red-team; câu hỏi chỉ nêu lựa chọn, không kèm khuyến nghị. Lưu ý: plan đã viết sẵn một số lựa chọn (chép lại từng đoạn, không phát nền, quyền theo chủ dữ liệu) nên các đáp án 1,00 có thể nghiêng theo nội dung plan.

| Câu hỏi | Jev | Quyết định | Áp vào |
|---|---|---|---|
| Cắt file trên bản ghi có transcript | chấp nhận mất, không cảnh báo 0,83 | Không đổi UI Cắt file; ghi trong tiêu chí | Success Criteria |
| Phạm vi chép lại | từng đoạn + cả phiên có xác nhận 1,00 | Giữ như Phase 3 | Phase 3 |
| Phát khi khoá màn hình | không trong GĐ1 1,00 | Audio trong WebView | Phase 3 |
| Quyền đọc/sửa/xoá dữ liệu của mình | chỉ cần đăng nhập + đúng chủ 1,00 | Như Phase 1 | Phase 1 |
| Phase 2 kiểm tra chỉ đọc | giữ 0,99 | Như Phase 2 | Phase 2 |
| Plan sẵn sàng triển khai | 0,82 | Đủ để bắt đầu Phase 1 | — |

### Verification Results
- Claims checked: 3 reviewer kiểm các đường dẫn/dòng plan trích (Fact Checker + Contract Verifier, tier Standard)
- Sai đã sửa trong plan: `AudioPlayer` không export (`media-player.tsx:116`), tên mặc định ở `session-row.tsx:23-32` chứ không ở `recording-groups.ts`, body limit thực 8 MiB (`routes.rs:32-35`), `delete_doc` chỉ nhận `id` (`file.rs:227`)
- Failed còn lại: 0

### Whole-Plan Consistency Sweep
- Đã rà `plan.md` + 4 phase: khoá phiên dùng tiền tố `s:`/`f:` thống nhất; phần sửa `{runId, texts}` thống nhất ở data model, API, export, rollback; Phase 2 chỉ đọc ở cả phase và bảng phase; xử lý Cắt file theo Jev (không cảnh báo) ghi trong Success Criteria.
- Mâu thuẫn còn lại: không.
