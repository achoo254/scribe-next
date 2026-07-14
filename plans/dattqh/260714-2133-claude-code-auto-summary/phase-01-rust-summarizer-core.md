---
phase: 1
title: "Rust summarizer core — module + detect + commands + events"
status: pending
effort: "M"
dependencies: [0]
---

# Phase 1 — Rust summarizer core

Mirror pattern sidecar transcribe (`src-tauri/src/transcribe.rs`): spawn process qua `std::process::Command`, reader thread đọc stdout → `app.emit(...)`. Nhưng KHÔNG persistent như model giữ ấm — mỗi lần tóm tắt là 1 lần spawn `claude -p` (single-shot), vì không có model để giữ ấm.

## Context

- Đọc tham khảo: `src-tauri/src/transcribe.rs` (pattern spawn/reader-thread/emit, `resolve()` tìm binary, `CREATE_NO_WINDOW`, kill sạch), `src-tauri/src/commands.rs` (khai báo `#[tauri::command]`), `src-tauri/src/lib.rs` (`invoke_handler![...]` đăng ký command + `manage(...)` state), `src-tauri/src/settings.rs` (`load()`).
- Cờ + lệnh chuẩn: lấy từ `reports/phase-00-spike-claude-p-headless-results.md`.

## Requirements

- Functional: detect trạng thái Claude Code (installed / logged-in / ready); nhận (transcript_text, prev_summary?, kind) → spawn `claude -p` → trả tóm tắt qua event; hủy được (kill child) khi dừng/đổi phiên.
- Non-functional: non-blocking (spawn + reader thread như transcriber); Windows ẩn cửa sổ (`CREATE_NO_WINDOW`); không chặn UI; 1 tóm tắt đang chạy tại 1 thời điểm (single in-flight — hủy cái cũ nếu có cái mới rolling).

## Files to create / modify

- **Create `src-tauri/src/summarizer.rs`**:
  - Interface tách sẵn: `trait Summarizer { fn summarize(...) }` + struct `ClaudeCodeSummarizer` (impl duy nhất hiện tại). Giữ đơn giản — 1 struct `Summarizer` với state `Mutex<Option<Child>>` cho in-flight cũng đủ nếu tách trait làm phức tạp; ưu tiên KISS, chỉ tách trait nếu thấy rõ đường mở rộng.
  - `pub fn claude_status(app) -> ClaudeStatus { installed: bool, logged_in: bool }` — dùng `resolve_claude()` (tìm `claude` trên PATH; Windows `where`, unix `command -v`) + call detect theo Phase 0.
  - `resolve_claude()` — tìm binary `claude` (PATH; cân nhắc vị trí cài mặc định Windows/macOS). Trả `Err` nếu không thấy.
  - `pub fn summarize(app, text, prev_summary, kind)` — build prompt template (tiếng Việt, hard-code; rolling incremental vs biên bản đầy đủ tùy `kind`), spawn `claude -p <cờ chuẩn>`, ghi `text`(+`prev_summary`) qua stdin + đóng EOF, reader thread đọc stdout → `app.emit("summary_msg", {type, ...})`; kill in-flight cũ trước khi spawn mới.
  - `pub fn cancel(&self)` — kill child in-flight (dừng ghi / đổi phiên).
  - Emit event schema: `{type: "start"|"delta"|"done"|"error", kind: "rolling"|"final"|"modec", text?, message?}` (delta chỉ khi Phase 0 xác nhận stream được; nếu không → chỉ start/done).
- **Modify `src-tauri/src/commands.rs`**: thêm `#[tauri::command]`:
  - `claude_status() -> ClaudeStatus`
  - `summarize(text: String, prev: Option<String>, kind: String) -> Result<(), String>`
  - `cancel_summary()`
- **Modify `src-tauri/src/lib.rs`**: `.manage(Summarizer::default())`, thêm 3 command vào `invoke_handler![...]`, `mod summarizer;`.

## Implementation steps

1. `mod summarizer;` + `manage` state + đăng ký command (khớp pattern `Transcriber`).
2. `resolve_claude()` + `claude_status()` theo cách detect chốt ở Phase 0.
3. Prompt template tiếng Việt: (a) rolling incremental — "Cập nhật bản tóm tắt hiện có với nội dung mới, giữ gọn gạch đầu dòng"; (b) final/modec — "Viết biên bản cuộc họp đầy đủ: mục đích, các ý chính, quyết định, việc cần làm".
4. `summarize()` spawn + stdin + reader thread emit; single in-flight (kill cũ).
5. `cancel_summary()`.
6. `cargo check` + `cargo clippy` sạch.

## Tests / validation

- `cargo test` (nếu tách được pure fn: build prompt, parse status) + `cargo check`.
- Thủ công: invoke `claude_status` (máy có/không claude) → đúng; invoke `summarize` với text mẫu → nhận `summary_msg` done kèm text (kiểm bằng log tạm/console).

## Risks / Rollback

- `claude` không trên PATH khi app chạy từ shortcut (env PATH khác terminal) → `resolve_claude()` phải thử cả vị trí cài mặc định, không chỉ PATH. Ghi rõ trong report nếu gặp.
- In-flight race (rolling bắn khi cái cũ chưa xong) → single in-flight + kill cũ; test bằng bắn 2 lần liên tiếp.
- Rollback: module độc lập, chưa wire UI → xóa `mod summarizer` + 3 command là gỡ sạch.
