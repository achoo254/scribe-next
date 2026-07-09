# Phase 05 — Glossary DevOps mặc định (verify + dọn code chết)

## Context Links
- Overview: [plan.md](plan.md)
- Depends on: không (độc lập hoàn toàn, không đụng file phase nào khác)
- Commit đã triển khai: `5e05433a14d711815f58988348be695d09ac48aa` ("feat(transcript): glossary sửa được + export VTT/JSON + player/editor + quản lý model", 2026-07-08)

## Overview
- Priority: P3 (đã xong — chỉ verify + cleanup nhỏ)
- Status: **đã triển khai trước khi lập plan này** — phase này KHÔNG implement mới, chỉ (1) xác nhận đúng yêu cầu, (2) dọn 1 hằng số chết còn sót trong sidecar.

## Key Insights
- **Quyết định #5 của user ("wire glossary DevOps làm mặc định") đã được code từ trước** — `settings.rs:34-35` (`#[serde(default = "default_glossary")] pub glossary: Option<String>`) + `settings.rs:58-66` (hàm `default_glossary()` — nội dung **giống 100%** `EXAMPLE_PROMPT` cũ trong `main.py:39-43`) + `settings.rs:83` (`Default impl` gọi `default_glossary()`). Cả 2 đường (setting mới tạo lần đầu VÀ settings.json cũ thiếu field `glossary` do serde default) đều ra glossary DevOps.
- `main.py:37-43` còn `EXAMPLE_PROMPT` — comment tự ghi rõ "chỉ để tham khảo/placeholder UI. KHÔNG dùng làm mặc định ngầm" — đã grep xác nhận (`Grep pattern="EXAMPLE_PROMPT"`) hằng số này KHÔNG được reference ở đâu khác trong toàn repo → dead code, an toàn xoá.
- Không cần sửa gì ở Rust/frontend — `use-glossary.ts` (đọc/ghi qua `settings-store.ts` merge pattern) đã hoạt động đúng từ commit trên.

## Requirements
### Functional
- Verify: cài app mới (xoá `settings.json` hoặc test trên profile sạch) → mở Mode C → glossary textarea hiện sẵn text DevOps (không rỗng).
- Xoá `EXAMPLE_PROMPT` (main.py dòng 37-43) — dead code, YAGNI.

### Non-functional
- Không đổi hành vi transcribe — `EXAMPLE_PROMPT` chưa từng được dùng trong logic, xoá không ảnh hưởng runtime.

## Architecture
Không đổi — xem `settings.rs::load()`/`save()` (dòng 89-110) đã là nguồn chân lý cho glossary.

## Related Code Files
- **Modify:** `sidecar/transcriber/main.py` (xoá dòng 37-43 `EXAMPLE_PROMPT` + comment liên quan)

## Implementation Steps
1. Xoá block dòng 37-43 trong `main.py`:
   ```python
   # Glossary MẪU (domain IT/DevOps) — chỉ để tham khảo/placeholder UI. KHÔNG dùng làm mặc định ngầm;
   # prompt thật lấy từ request (app đọc settings.glossary). Rỗng → transcribe không kèm initial_prompt.
   EXAMPLE_PROMPT = (
       "Bài nói kỹ thuật tiếng Việt về hệ thống và DevOps. Thuật ngữ thường gặp: "
       "VictoriaLogs, VictoriaMetrics, Grafana, OpenTelemetry, observability, log, parse, "
       "JSON, query, cheatsheet, cluster, HA, Docker, server, port, IP, API, systemctl, rsyslog."
   )
   ```
2. Cập nhật docstring đầu file (dòng 18 `Glossary (initial_prompt) truyền từ app (settings)...`) nếu cần rút ngắn do không còn tham chiếu EXAMPLE_PROMPT.
3. Verify thủ công: xoá/backup `%APPDATA%/vn.inet.onescribe/settings.json` (hoặc dir tương ứng `app_data_dir()`), chạy app, mở Mode C → check glossary textarea.
4. Verify settings cũ (không có field `glossary`, tạo giả 1 file settings.json cũ thiếu field) → load → vẫn ra glossary DevOps (serde default hoạt động).

## Todo List
- [ ] Xoá `EXAMPLE_PROMPT` trong `main.py`
- [ ] Test fresh install (không settings.json) → glossary DevOps mặc định
- [ ] Test settings.json cũ thiếu field `glossary` → serde default áp dụng đúng
- [ ] `python -c "import ast; ast.parse(open('main.py').read())"` hoặc chạy sidecar thử để chắc không lỗi syntax sau khi xoá

## Success Criteria
- Glossary textarea (Mode C) không rỗng lần đầu mở app trên profile sạch — hiện đúng nội dung DevOps 100% khớp giá trị cũ.
- `main.py` không còn `EXAMPLE_PROMPT` (grep trả rỗng), sidecar chạy bình thường không lỗi import/syntax.

## Risk Assessment
| Risk | Likelihood × Impact | Mitigation |
|---|---|---|
| Xoá nhầm phá docstring/comment liên quan còn cần | Thấp × Thấp | chỉ xoá đúng block đã trích, review diff trước commit |

## Security Considerations
- Không áp dụng — thay đổi thuần dọn code, không đổi luồng dữ liệu.

## Next Steps
- Không phase nào phụ thuộc phase này.
