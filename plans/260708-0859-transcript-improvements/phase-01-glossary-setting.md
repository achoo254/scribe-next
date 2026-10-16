# Phase 01 — Glossary cấu hình được

> Tier **S** · Phụ thuộc: — · Fix gap G1 (glossary hardcode IT/DevOps trong `main.py:29`).

## Bối cảnh

`DEFAULT_PROMPT` (`sidecar/transcriber/main.py:29`) cứng domain DevOps, dùng cho MỌI file (cả ct2 `initial_prompt` lẫn mlx). Ghi âm ngoài domain → bias sai tên riêng. Mục tiêu: user chỉnh glossary trong app; để trống → không truyền prompt.

## Yêu cầu

- Glossary lưu qua settings (persist), truyền xuống sidecar per-job.
- Rỗng/không set → sidecar chạy **không** `initial_prompt` (không ép DevOps).
- 1 nguồn glossary dùng chung Mode C (batch) và Mode A (auto-transcript).
- Tương thích settings cũ (serde default).

## Files sửa

- `src-tauri/src/settings.rs` — thêm field `glossary: Option<String>` (`#[serde(default)]`) vào `AppSettings` + `Default`.
- `src-tauri/src/transcribe.rs` — trong `transcribe()`: load `settings::load(app)`, lấy `glossary`, thêm vào JSON gửi sidecar khi non-empty: `"prompt": <glossary>`. (Đọc ở Rust = DRY, không phải nhét qua mọi lời `invoke`.)
- `sidecar/transcriber/main.py` — `transcribe()` nhận `prompt = req.get("prompt")`; truyền xuống `_transcribe_ct2`/`_transcribe_mlx`. `initial_prompt=prompt` chỉ khi truthy; bỏ hằng `DEFAULT_PROMPT` (hoặc giữ làm placeholder gợi ý ở UI, KHÔNG dùng làm mặc định ngầm).
- `src/views/TranscriptView.vue` — thêm ô "Thuật ngữ / tên riêng" (UiTextarea, collapsible) bind vào glossary; load từ `get_settings`, lưu qua `set_settings` (debounce hoặc lưu khi blur).

## Các bước

1. `settings.rs`: thêm field + default `None`.
2. `transcribe.rs::transcribe`: sau khi có `dir`, `let glossary = settings::load(app).ok().and_then(|s| s.glossary).filter(|g| !g.trim().is_empty());` → khi build JSON mỗi file, chèn `prompt` nếu `Some`.
3. `main.py`: đổi chữ ký `transcribe(..., prompt=None)`; hàm ct2/mlx nhận `initial_prompt=prompt`; xoá dùng `DEFAULT_PROMPT` làm default. Giữ 1 comment ví dụ glossary mẫu để UI show placeholder.
4. UI: section "Thuật ngữ" (mặc định thu gọn) + placeholder gợi ý (VictoriaLogs, Grafana…). Lưu vào settings.

## Test / validate

- `cargo build` + `vue-tsc --noEmit` sạch.
- Transcribe 1 file có tên riêng đặc thù (vd tên người/sản phẩm) với glossary chứa tên đó → xuất đúng chính tả; xoá glossary → chạy bình thường (không lỗi thiếu prompt).
- Đổi glossary ở Mode C → auto-transcript Mode A (đoạn ghi kế) cũng dùng glossary mới (chung settings).

## Rủi ro / rollback

- Rủi ro thấp. Nếu prompt quá dài (Whisper giới hạn ~224 token context) → cắt/cảnh báo ở UI. Rollback: field serde-default → gỡ chỉ cần bỏ đọc `prompt` ở `main.py` (fallback về không prompt).
