# Phase 0 Spike — `claude -p` headless feasibility — KẾT QUẢ

**Ngày:** 2026-07-14 · **Máy:** Windows 11, dev · **Verdict: ✅ GO**

## Môi trường

- `claude` v**2.1.209** (Claude Code), PATH: `/c/Users/hoandat-pc/.local/bin/claude`.
- `ANTHROPIC_API_KEY` **UNSET** → `claude -p` chạy được ⇒ **dùng OAuth/login (gói sub), không phải API key**. ✓ (mục tiêu chính)

## Kết quả 5 câu hỏi GATE

| # | Câu hỏi | Kết quả |
|---|---------|---------|
| 1 | Headless chạy? | ✓ `claude -p` non-interactive, exit 0, không mở TUI. |
| 2 | Auth = sub? | ✓ API key UNSET vẫn chạy. `total_cost_usd` chỉ là **ước tính tương đương**, không phải phí thật khi dùng sub (nhưng token tính vào **rate limit** sub). |
| 3 | Detect trạng thái? | ✓ installed = `which claude`/`where claude`. logged-in = tồn tại `~/.claude/.credentials.json` (máy này CÓ, 509B) — fast-path; validate bằng probe nhẹ khi dùng lần đầu (credential có thể ở keychain/hết hạn). |
| 4 | Latency? | opus (default) **25s**; **haiku 4.5 ~8s** (api 5.6s, ttft 4.9s). Đều < 60s → hợp rolling. |
| 5 | Cờ đúng? | ✓ xem dưới. |

## Cờ chuẩn (đã verify)

- `-p, --print` — non-interactive, in ra rồi thoát.
- `--output-format text|json|stream-json` — **json** cho parse structured.
- `--model <id>` — `claude-haiku-4-5` (rolling, nhanh) / để default (biên bản cuối, chất lượng).
- `--disallowed-tools "Bash Edit Write Read Glob Grep WebFetch WebSearch"` — chặn tool cho tác vụ tóm tắt thuần text.
- Input: pipe qua **stdin** (`cat seg.txt | claude -p "<instruction>"`) — stdin gộp vào input, hoạt động tốt.
- ⚠️ **KHÔNG dùng `--bare`**: ép auth chỉ API key (bỏ OAuth) → mất sub.

### Lệnh mẫu Phase 1 spawn (rolling)

```
cat <transcript> | claude -p "<prompt VN>" \
  --model claude-haiku-4-5 --output-format json \
  --disallowed-tools "Bash Edit Write Read Glob Grep WebFetch WebSearch"
```

### JSON shape (parse ở Phase 1)

```json
{ "type":"result", "subtype":"success", "is_error":false,
  "result":"<markdown tóm tắt>", "stop_reason":"end_turn",
  "session_id":"...", "total_cost_usd":0.08, "usage":{...} }
```
→ Rust/FE đọc: `subtype=="success" && !is_error` → lấy `result`; else `error`.

## Phát hiện quan trọng (ảnh hưởng thiết kế)

1. **Overhead ~38k token/call** (`cache_creation_input_tokens: 38660`) — là context harness Claude Code (CLAUDE.md global + tool defs + MCP inet-viui/mongodb/firebase + skills catalog) nạp mỗi lần. Transcript thật chỉ 9 input token; phần còn lại là overhead.
   - **Hệ quả:** rolling 60 lần/giờ × 38k = ~2.3M token vào rate limit sub → có thể chạm trần.
   - **Giảm thiểu:** (a) **haiku cho rolling**; (b) **đừng rolling quá dày** — 60s là tối thiểu, cân nhắc theo lượng text mới thay vì timer cứng; (c) chạy từ **cwd sạch** (không CLAUDE.md project); (d) `--disable-slash-commands` + `--settings` tối giản để bớt skills/MCP khỏi context; (e) **tái dùng session** (`--resume <session_id>`) cho rolling → 38k thành cache_read (~0.1×) thay vì tạo mới mỗi lần, đồng thời session tự nhớ tóm tắt trước (incremental tự nhiên).
2. **Model lever:** haiku 4.5 (rolling, 8s) vs default/opus (biên bản cuối, 25s, chất lượng cao hơn).
3. **Chất lượng output:** biên bản tiếng Việt đúng cấu trúc (ý chính + việc cần làm + deadline + gán người) ngay lần đầu — không cần fine-tune prompt nhiều.

## Cập nhật quyết định cho Phase 1+

- Rolling: `--model claude-haiku-4-5`; biên bản cuối/Mode C: default model.
- Detect: file `~/.credentials.json` (fast) + probe validate lần đầu.
- Parse JSON `.result`/`.subtype`/`.is_error`.
- **Cân nhắc session-reuse** (`--resume`) cho rolling để cắt overhead — đánh giá ở Phase 1 (đổi thiết kế: giữ 1 session/phiên ghi thay vì stateless + prev_summary thủ công). Nếu phức tạp → giữ stateless MVP, tối ưu sau.

## Unresolved

- Cách giảm 38k overhead khi vẫn giữ OAuth (thử `--disable-slash-commands` + `--settings` rỗng + cwd sạch — đo lại token). Làm đầu Phase 1.
- Session-reuse `--resume` giữ auth sub + cache hoạt động xuyên call? (verify Phase 1).
- Latency thật khi spawn từ OneScribe (cwd runtime, không project CLAUDE.md) — có thể < 25s do bớt context.
