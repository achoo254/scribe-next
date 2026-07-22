---
phase: 2
title: "Settings + frontend composable — rolling debounce + incremental + Mode A/C wiring"
status: pending
effort: "M"
dependencies: [1]
---

# Phase 2 — Settings + frontend composable

Logic debounce/incremental sống ở **frontend** (đã có event per-segment ở `use-record-transcribe.ts`); Rust chỉ lo spawn. Composable mới `use-summary.ts` gom text đoạn → debounce ~60s → `invoke("summarize", ...)`; nghe `summary_msg` → cập nhật state hiển thị.

## Context

- Đọc tham khảo: `src/composables/use-record-transcribe.ts` (pattern `listen("transcribe_msg")`, state ref, `enqueue`, dedup, `onUnmounted` cleanup), `src/composables/settings-store.ts` (đọc/ghi settings frontend), `src/composables/use-transcribe.ts` (Mode C batch → text `done`).
- `src-tauri/src/settings.rs` — thêm field (serde default → tương thích ngược).

## Requirements

- Mode A rolling: gom `text` từ event `done` của từng đoạn (nguồn: cùng luồng `use-record-transcribe`), sau mỗi ~`interval` giây (mặc định 60) mà có text mới → gọi `summarize(kind="rolling", prev=<tóm tắt hiện tại>, text=<đoạn tích luỹ chưa tóm>)`; nhận `done` → cập nhật `rollingSummary`.
- Mode A final: khi dừng ghi → gọi `summarize(kind="final", text=<toàn transcript phiên>)` → `finalMinutes`.
- Mode C: nút "Tạo biên bản" → `summarize(kind="modec", text=<nội dung transcript file>)`.
- Toggle "Tự tóm tắt (Claude Code)" persist trong settings; nếu `claude_status` không ready → disabled.

## Files to create / modify

- **Modify `src-tauri/src/settings.rs`** — thêm vào `AppSettings` (đều `#[serde(default...)]`):
  - `summarize_enabled: bool` (default false — bật là hành động chủ động, giống `use_gpu`).
  - `summary_rolling_interval_s: u32` (default 60).
  - (Để sau, KHÔNG làm MVP: `summary_prompt: Option<String>` — template sửa được.)
  - Cập nhật `Default for AppSettings` + default fn tương ứng.
- **Create `src/composables/use-summary.ts`**:
  - State: `enabled`, `claudeReady` ({installed, loggedIn}), `rollingSummary`, `finalMinutes`, `status` ("idle"|"summarizing"|"error"), `error`.
  - `checkClaude()` → `invoke("claude_status")` → set `claudeReady`.
  - `listen("summary_msg")` → cập nhật theo `kind`+`type` (start/done/error; delta nếu có).
  - `feedSegment(text)` — tích luỹ buffer đoạn chưa tóm (Mode A); debounce timer ~`interval`s → nếu buffer khác rỗng và không đang summarizing → `invoke("summarize", {kind:"rolling", prev:rollingSummary, text:buffer})`, clear buffer.
  - `finalize(fullText)` — `invoke("summarize", {kind:"final", text:fullText})`.
  - `summarizeFile(text)` — Mode C `invoke("summarize", {kind:"modec", text})`.
  - `reset()` + `onUnmounted` clear timer + unlisten (khớp pattern `use-record-transcribe`).

## Implementation steps

1. Thêm field settings + default (Rust) → `cargo check`.
2. `use-summary.ts` state + `checkClaude` + `listen("summary_msg")`.
3. Debounce rolling (timer + buffer, guard single in-flight khớp Rust cancel/kill cũ).
4. `finalize` + `summarizeFile`.
5. Cleanup timer/listener trong `onUnmounted`.

## Tests / validation

- `pnpm vue-tsc` / build FE xanh.
- Thủ công (chờ Phase 3 có UI để bấm) — ở phase này verify bằng log: gọi thủ công `feedSegment`/`finalize` từ console, xem `summary_msg` cập nhật state.

## Risks / Rollback

- Rolling bắn khi rolling trước chưa xong → guard `status==="summarizing"` bỏ qua/để chờ tick sau + Rust kill in-flight cũ (Phase 1).
- Buffer lớn dần nếu tóm chậm → chấp nhận (incremental gộp), hoặc cap độ dài buffer đẩy 1 lần.
- Rollback: composable + field settings độc lập; chưa render UI → gỡ không ảnh hưởng luồng ghi/transcribe.
