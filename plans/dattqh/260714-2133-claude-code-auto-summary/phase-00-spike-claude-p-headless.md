---
phase: 0
title: "Spike — `claude -p` headless feasibility (FEASIBILITY GATE)"
status: pending
effort: "S (1 buổi)"
gate: true
---

# Phase 0 — Spike: `claude -p` headless feasibility

**GATE cứng, làm trước nhất.** Validate rẻ (không viết code app) rằng Scribe có thể driver Claude Code headless. NO-GO ở đây → dừng, xem lại backend (Messages API BYOK / Ollama) trước khi đổ công Phase 1+.

## Mục tiêu (câu hỏi cần trả lời)

1. **Headless chạy được?** `claude -p "<prompt>"` (đọc transcript qua stdin) trả tóm tắt ra stdout, non-interactive, không mở TUI?
2. **Auth = subscription?** Dùng đúng login Claude Code sẵn có (không đòi API key, không login tương tác giữa chừng)? Xác nhận không phát sinh phí API.
3. **Detect được trạng thái?** Có cách phân biệt: (a) `claude` chưa cài (không trên PATH), (b) đã cài nhưng chưa `/login`, (c) sẵn sàng — qua exit code / stderr / lệnh version?
4. **Latency chấp nhận?** Đo thời gian 1 lần tóm tắt đoạn ~500–1000 từ → có hợp rolling ~60s không? (headless spin-up + model latency)
5. **Cờ đúng?** Xác định cờ chính xác: output format (`--output-format text|json`), giới hạn tool (tắt Read/Bash/… vì tóm tắt không cần), single-shot.

## Việc làm (thủ công, ngoài code app)

- `claude --help` (và `claude -p --help` nếu có) → ghi lại cờ thật: print mode, output-format, allowed/disallowed tools, max-turns/single-shot.
- Chạy thử trên máy (Windows dev):
  - `echo "<transcript mẫu tiếng Việt>" | claude -p "Tóm tắt cuộc họp này, gạch đầu dòng ý chính, tiếng Việt"` → xem stdout.
  - Thử `--output-format json` → parse được `result`/`text` không.
  - Thử tắt tool: `--allowedTools ""` hoặc cờ tương đương → vẫn tóm tắt được (không đụng filesystem).
  - Đo thời gian (`time ...`) 3 lần: đoạn ngắn (~300 từ), vừa (~800 từ), incremental (tóm tắt cũ + đoạn mới).
- Detect trạng thái:
  - `where claude` (Windows) / `command -v claude` → installed?
  - 1 call thử tối thiểu → exit code + stderr khi CHƯA login (mô phỏng bằng máy/logout nếu an toàn, hoặc đọc doc) → ghi lại cách nhận biết.
- Xác nhận auth: kiểm tra không có `ANTHROPIC_API_KEY` trong env mà vẫn chạy (→ dùng profile/login sub). Kiểm tra dashboard/billing không phát sinh token API.

## Deliverable

- Report `reports/phase-00-spike-claude-p-headless-results.md`: GO/NO-GO + bảng cờ thật + số latency + cách detect installed/logged-in + snippet lệnh chuẩn để Phase 1 spawn.

## Success / Gate criteria

- [ ] GO: `claude -p` chạy headless, dùng login sub, latency đoạn vừa ≤ ~15s (chấp nhận cho rolling 60s), có cách detect 3 trạng thái.
- [ ] Chốt được: `program` + `args` chuẩn để spawn (vd `claude -p --output-format json --allowedTools ""`), cách feed stdin, cách đọc kết quả.
- [ ] NO-GO nếu: đòi login tương tác trong subprocess, hoặc chỉ chạy với API key (không dùng sub), hoặc latency quá cao cho cả biên bản cuối.

## Risks / Rollback

- Rủi ro: cờ/ý nghĩa `-p` khác giả định → chính vì vậy Phase 0 tồn tại; ghi cờ THẬT, không đoán.
- NO-GO không mất gì (chưa viết code app) → chuyển plan sang backend Messages API (BYOK) hoặc Ollama, giữ nguyên interface `Summarizer` dự kiến ở Phase 1.
