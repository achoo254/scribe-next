---
phase: 3
title: "UI panes + fallback + lưu biên bản + verify"
status: pending
effort: "M"
dependencies: [2]
---

# Phase 3 — UI + fallback + persist + verify

Ghép `use-summary` vào UI: pane "Tóm tắt trực tiếp" ở Mode A (rolling + biên bản cuối), section "Tạo biên bản" ở Mode C; xử lý fallback khi chưa có Claude Code; lưu biên bản ra đĩa; verify end-to-end.

## Context

- Đọc tham khảo: `src/views/RecordView.vue` (Mode A — nơi wire `use-record-transcribe`, toggle auto-transcript, `SegmentTranscriptList`), `src/views/TranscriptView.vue` (Mode C), `src/components/AsrModelControls.vue` + `SegmentTranscriptList.vue` (pattern component + design system), `src/design-system/ui/` (chỉ dùng `Ui*`, token-only, WCAG — theo README design system).

## Requirements

- Mode A: toggle "Tự tóm tắt (Claude Code)" cạnh toggle auto-transcript; khi bật → pane "Tóm tắt trực tiếp" hiện `rollingSummary` cập nhật lúc ghi + `finalMinutes` khi dừng. Nút Sao chép / Lưu.
- Mode C: khu vực "Tạo biên bản" cho transcript đang xem → hiện `finalMinutes`.
- Fallback: `claudeReady` không sẵn sàng → toggle disabled + `UiAlert` hướng dẫn ("Cần cài & đăng nhập Claude Code để dùng tính năng này") kèm link tài liệu; KHÔNG crash/treo.
- Persist: lưu biên bản `summary.md` cạnh recording/transcript (dùng lệnh ghi file có sẵn hoặc thêm command nhỏ).
- UI theo design system: chỉ `Ui*`, token, không hardcode màu/spacing.

## Files to modify / create

- **Modify `src/views/RecordView.vue`** — wire `use-summary`: gọi `checkClaude()` khi mount; `feedSegment(text)` khi 1 đoạn `done` (hook vào chỗ `use-record-transcribe` báo done); `finalize(fullText)` khi dừng ghi; render pane.
- **Modify `src/views/TranscriptView.vue`** — nút "Tạo biên bản" → `summarizeFile(text)`; render kết quả.
- **Create `src/components/SummaryPanel.vue`** — component hiển thị tóm tắt (trạng thái summarizing/done/error, nội dung markdown-ish, nút Sao chép/Lưu), dùng chung Mode A + C (DRY). `Ui*` only.
- **Persist**: nếu chưa có command ghi text ra path → thêm `#[tauri::command] save_summary(path, content)` (hoặc tái dùng plugin-fs). Lưu `summary.md`.
- (Nếu cần) **Modify `src-tauri/src/commands.rs` / `lib.rs`** cho `save_summary`.

## Implementation steps

1. `SummaryPanel.vue` (design system) — props: status/summary/error + emit copy/save.
2. RecordView: toggle + pane + wire feedSegment/finalize; fallback alert theo `claudeReady`.
3. TranscriptView: nút + panel Mode C.
4. Lưu `summary.md` (command hoặc plugin-fs).
5. Fallback state: disabled toggle + alert khi thiếu Claude Code.

## Tests / validation (verify end-to-end)

- `pnpm ci:all` / `pnpm vue-tsc` + `cargo check` + `npm run tauri build` (hoặc `tauri dev`) xanh.
- **Máy CÓ Claude Code + login**: ghi Mode A giọng tiếng Việt ~2–3 phút → pane rolling cập nhật trong lúc ghi; dừng → biên bản đầy đủ; **không gõ prompt tay**. Mode C: chọn transcript → biên bản. Lưu `summary.md` OK. Xác nhận không phát sinh phí API (dùng sub).
- **Máy KHÔNG có / chưa login Claude Code**: toggle disabled + alert hướng dẫn; app không crash; luồng ghi/transcribe cũ vẫn chạy bình thường.
- Delegate `code-reviewer` sau khi code xong (theo operating model).

## Risks / Rollback

- QA giọng thật không automate được → user (Reviewer/QA) test thủ công UI/UX; Agent cung cấp bản build.
- Ghi `summary.md` sai quyền/đường dẫn → dùng thư mục cạnh recording (đã có quyền ghi), validate path.
- Rollback: toggle mặc định off → tính năng ẩn; gỡ = xóa panel + unwire + field settings. Không đụng luồng transcribe.

## Docs cần cập nhật (sau khi xong)

- `docs/codebase-summary.md`: thêm mục "Tự tóm tắt qua Claude Code (desktop, local, dùng sub)".
- `docs/project-roadmap.md`: thêm entry tính năng + trạng thái.
