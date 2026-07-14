---
phase: 3
title: "Frontend toolkit UI"
status: pending
priority: P2
dependencies: [2]
---

# Phase 3: Frontend toolkit UI

## Overview
UI cho convert/extract/trim: gọi job qua composable, poll tiến độ, tải output đơn file. Gắn hành động vào lịch sử ghi + tab Cắt file.

## Requirements

<!-- Updated: Validation Session 1 — format audio {MP3,M4A,WAV} (bỏ MP4); trim → MP3. -->

- Functional: từ 1 bản ghi (lịch sử) hoặc file đã upload → chọn "Xuất **MP3/M4A/WAV**", "Tách audio" (input có video), "Trim (in/out)" → job chạy → tải file kết quả.
- Non-functional: bám design tokens/ViUI (dark theme); không phá `useSplitter` split hiện có; tái dùng poll pattern. `type` gửi lên = convert/extract/trim (khớp BE Phase 2).

## Architecture
- Tổng quát hoá job composable (DRY): `use-splitter.ts` hiện chỉ split. Chọn 1 trong 2:
  - (a) Mở rộng `useSplitter.createJob(inputFileId, type, params)` — thêm `type` (convert/extract/trim) + params (format/startS/endS), giữ chữ ký split cũ qua default. HOẶC
  - (b) `use-media-job.ts` mới bọc chung POST /api/jobs + poll (dùng lại logic poll của useSplitter). Khuyến nghị (a) nếu poll giống hệt (tránh trùng logic poll) — extract logic poll ra hàm dùng chung.
- Kết quả op đơn file: đọc `result.segments[0].fileId` → tải `/api/files/:id?download=1` (đã có ở use-recordings/use-managed-files — cân nhắc 1 helper `downloadFile(id)` dùng chung, tránh lặp 3 chỗ).
- UI:
  - `RecordingHistory.vue`: mỗi bản ghi thêm nút/menu "Xuất…" (chọn format) + "Tách audio" (nếu hasVideo) + "Trim…" (mở modal nhập in/out mm:ss). Modal dùng `v-dialog` nền solid (đã có pattern os-confirm-modal).
  - `SplitView.vue`: với file đã upload, thêm khu "Chuyển định dạng / Trim" song song "Cắt".
- Trim modal: 2 input thời gian (giây hoặc mm:ss) + validate start<end ≤ duration; output cố định **MP3** (không cho chọn format ở Wave 1); hiển thị tiến độ + link tải khi done.

## Related Code Files
- Modify: `web/src/composables/use-splitter.ts` (op/params + tách poll dùng chung) HOẶC Create `web/src/composables/use-media-job.ts`
- Modify: `web/src/components/RecordingHistory.vue` (nút Xuất/Tách/Trim + modal)
- Modify: `web/src/views/SplitView.vue` (convert/trim cho file upload)
- Cân nhắc Create: `web/src/composables/download-file.ts` (helper `downloadFile(id)` dùng chung — gom 3 chỗ lặp)

## Implementation Steps
1. Tổng quát job composable (op/params) + tách poll dùng chung; giữ split cũ nguyên.
2. Helper `downloadFile(id)` dùng chung (khử lặp use-recordings/use-managed-files/mới).
3. UI hành động Xuất/Tách/Trim + Trim modal (validate in/out).
4. Nối tiến độ + tải kết quả; refresh usage + manage list sau khi tạo output.
5. Gate: `vue-tsc --noEmit` + `vite build` EXIT 0; QA thủ công trên Chrome (không có test harness FE — roadmap P3).

## Success Criteria
- [ ] Từ bản ghi webm → "Xuất MP3" → tải được .mp3 phát đúng.
- [ ] Screen recording → "Tách audio" → .mp3 không video.
- [ ] "Trim" in/out → file đúng đoạn; validate chặn start≥end / vượt duration.
- [ ] Không lặp logic poll (dùng chung với split) + không lặp downloadFile 3 chỗ.
- [ ] `vue-tsc` + `vite build` EXIT 0.

## Risk Assessment
- **Không có test harness FE** (vitest chưa setup — roadmap P3) → gate = typecheck/build + QA thủ công. Không claim "đã test" nếu chưa chạy thật trên Chrome.
- **Cache deploy**: như các lần trước, cần hard-refresh tới khi nginx `no-cache` được cài lên .65.
- **UX nhiều nút**: tránh làm rối hàng action bản ghi → gom "Xuất…" vào 1 menu/nút thay vì nhiều nút rời.
