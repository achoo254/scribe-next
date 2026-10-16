---
title: "Scribe — Tự tóm tắt/biên bản qua Claude Code headless (hướng B)"
description: "Scribe làm driver gọi `claude -p` ngầm (dùng gói sub Claude Code trên máy) để tóm tắt transcript: rolling realtime lúc ghi (Mode A) + biên bản đầy đủ khi dừng; Mode C tóm tắt file có sẵn. Fallback: chỉ báo khi chưa cài/chưa login."
status: draft
priority: P2
branch: "main"
tags: [claude-code, headless, summary, tauri, desktop, mode-a, mode-c]
created: "2026-07-14T21:33:00+07:00"
createdBy: "ck:ask → plan"
---

# Scribe — Tự tóm tắt/biên bản qua Claude Code headless

> **Định vị (chốt với user 2026-07-14):** tính năng **cá nhân/nội bộ desktop**, KHÔNG phải feature ship kèm sản phẩm. Scribe = driver; **Claude Code trên máy** = engine ẩn; dùng **login/gói sub sẵn có** của user (không API key, không tốn token). Giống mô hình pencil.dev ở chỗ *piggyback lên Claude Code đã cài*, nhưng ngược hướng: Scribe điều khiển Claude Code (hướng B), không phải Claude Code điều khiển Scribe (hướng A/MCP).

## Overview

Thêm 1 backend tóm tắt **`claude -p` (Claude Code print/headless mode)** vào Scribe desktop:
- **Mode A (Ghi):** trong lúc ghi, cứ ~60s gom text đoạn mới → cập nhật **bản tóm tắt rolling** (incremental: tóm tắt cũ + đoạn mới). Bấm **Dừng** → sinh **biên bản đầy đủ** 1 lần. Hiển thị ở pane "Tóm tắt trực tiếp".
- **Mode C (Transcript):** nút "Tạo biên bản" cho transcript file đã có (1 lần gọi).
- **Fallback:** detect `claude` chưa cài / chưa `/login` → disable toggle + hiện hướng dẫn (KHÔNG tự fallback Ollama/API ở giai đoạn này).

Kiến trúc **mirror pattern sidecar transcribe** đã có (`Transcriber` trong `src-tauri/src/transcribe.rs`): spawn process qua `std::process::Command`, reader thread đọc stdout → `app.emit(...)`, frontend `listen(...)`. Tách interface `Summarizer` (Rust) để sau này cắm thêm backend (Ollama/Messages API BYOK) mà không sửa luồng — nhưng giai đoạn này CHỈ impl `claude -p`.

## Quyết định đã chốt (nguồn chân lý)

| # | Quyết định |
|---|-----------|
| 1 | Backend tóm tắt = **`claude -p` headless**, dùng login Claude Code trên máy (subscription, không API key). |
| 2 | Trigger = **rolling ~60s lúc ghi (Mode A) + biên bản đầy đủ khi dừng**; Mode C = on-demand 1 lần. |
| 3 | Rolling = **incremental** (input: bản tóm tắt trước + đoạn transcript mới) → giữ context nhỏ, giảm latency + tiêu thụ gói sub. |
| 4 | Debounce ở **frontend composable** (đã có event `done` per-segment ở `use-record-transcribe.ts`); Rust chỉ lo spawn + stream stdout. |
| 5 | Fallback khi thiếu Claude Code = **chỉ báo + hướng dẫn**, disable toggle. KHÔNG auto Ollama/API (YAGNI). |
| 6 | Phạm vi = **cả Mode A + Mode C**. |
| 7 | Prompt/biên bản template = **hard-code tiếng Việt trong app** (setting sửa được để sau; MVP fixed). |
| 8 | Interface `Summarizer` tách sẵn (pluggable) nhưng CHỈ 1 impl `ClaudeCodeSummarizer` lúc này. |

## Phase 0 GATE — kết quả (2026-07-14): ✅ GO

Verified trên máy (claude v2.1.209, API key UNSET → dùng sub). Chi tiết: [report](./reports/phase-00-spike-claude-p-headless-results.md). Deltas cho Phase 1+:
- Rolling dùng `--model claude-haiku-4-5` (~8s); biên bản cuối/Mode C dùng model default (~25s, chất lượng cao hơn).
- Spawn: `cat <txt> | claude -p "<prompt VN>" --output-format json --disallowed-tools "Bash Edit Write Read Glob Grep WebFetch WebSearch"`; parse `.result` / `.subtype` / `.is_error`. **KHÔNG dùng `--bare`** (mất OAuth sub).
- Detect: installed = `which/where claude`; logged-in = tồn tại `~/.claude/.credentials.json` (fast) + probe validate lần đầu.
- ⚠️ **Overhead ~38k token/call** (context harness). → rolling đừng bắn quá dày; cân nhắc `--resume` (session-reuse → cache_read + incremental tự nhiên) + cwd sạch + `--disable-slash-commands`; đo lại đầu Phase 1.

## Phases

| Phase | Name | Status | Gate |
|-------|------|--------|------|
| 0 | [Spike — `claude -p` headless feasibility](./phase-00-spike-claude-p-headless.md) | ✅ **DONE — GO** (2026-07-14, [report](./reports/phase-00-spike-claude-p-headless-results.md)) | **FEASIBILITY GATE — PASSED** |
| 1 | [Rust summarizer core (module + detect + commands + events)](./phase-01-rust-summarizer-core.md) | Pending | — |
| 2 | [Settings + frontend composable (rolling debounce + incremental + Mode A/C wiring)](./phase-02-settings-composable-wiring.md) | Pending | — |
| 3 | [UI panes + fallback + lưu biên bản + verify](./phase-03-ui-fallback-persist-verify.md) | Pending | — |

## Thứ tự & phụ thuộc

- **Phase 0 (GATE) trước nhất** — validate rẻ rằng `claude -p` chạy headless được trên máy, dùng đúng login sub (không đòi login tương tác), latency chấp nhận cho rolling ~60s, và có cách detect installed/logged-in. NO-GO → xem lại backend (chuyển Messages API/Ollama) trước khi đổ công.
- Phase 1 phụ thuộc Phase 0 GO.
- Phase 2 phụ thuộc Phase 1 (commands + events sẵn).
- Phase 3 phụ thuộc Phase 2.

## Acceptance criteria (toàn plan)

- [ ] Máy có Claude Code + đã login: bật toggle "Tự tóm tắt", ghi Mode A → pane "Tóm tắt trực tiếp" cập nhật rolling trong lúc ghi; bấm Dừng → biên bản đầy đủ; **không gõ prompt tay lần nào**.
- [ ] Mode C: chọn transcript đã có → "Tạo biên bản" → ra biên bản.
- [ ] Máy chưa cài / chưa login Claude Code → toggle disabled + thông báo hướng dẫn rõ ràng; app KHÔNG crash, KHÔNG treo.
- [ ] Tóm tắt dùng gói sub (không cần nhập API key); xác nhận không phát sinh phí API.
- [ ] Biên bản lưu ra đĩa (`summary.md`) cạnh recording/transcript.
- [ ] `pnpm ci:all` / build Tauri xanh; không phá luồng transcribe hiện có.

## Rủi ro lớn nhất

1. **`claude -p` auth/headless** (Phase 0) — nếu print-mode đòi login tương tác hoặc không dùng được sub trong subprocess → cả hướng phải xem lại. → Gate cứng Phase 0.
2. **Latency + rate limit gói sub** — headless nặng hơn API; rolling dày có thể chậm/chạm trần. → debounce ~60s + incremental + cho tắt rolling (chỉ biên bản cuối).
3. **Detect "đã login"** — có thể không có lệnh check sạch; phải suy từ exit code/stderr của 1 call thử. → xác định cách detect trong Phase 0.
4. **Cross-platform `claude` trên PATH** — Windows vs macOS; app spawn subprocess cần tìm đúng binary (giống `resolve()` của transcriber). → Phase 1.

## Out of scope (YAGNI)

- Auto fallback Ollama / Messages API (BYOK) — để backend interface sẵn, không impl.
- Ship tính năng cho end-user không có Claude Code — chỉ báo, không giải quyết.
- Cấu hình template biên bản trong Settings — MVP hard-code; field để sau.
- Web app (`web/` + `server/`) — tính năng này desktop-only (BE Rust musl không có SDK, khác luồng).
- Diarization / phân vai người nói.

## Unresolved (xác nhận trong Phase 0)

- Cờ chính xác của `claude -p` (output format, giới hạn tool, non-interactive) — verify bằng `claude --help` trên máy.
- Cách detect "đã login" đáng tin (exit code vs parse stderr).
