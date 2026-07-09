# Phase 03 — Thêm model whisper-large-v3-turbo

## Context Links
- Overview: [plan.md](plan.md)
- Depends on: không phụ thuộc code Phase 01/02 (chỉ đặt sau trong thứ tự để tránh đụng `transcribe.rs` cùng lúc với Phase 04)
- Research: mục 4 (bảng so sánh model) trong research report.
- Verify HF (thực hiện lúc lập plan): `deepdml/faster-whisper-large-v3-turbo-ct2` — siblings: `config.json, model.bin (1,617,884,929 bytes), tokenizer.json, vocabulary.json` — flat structure (không subfolder), MIT license. Loại `mobiuslabsgmbh/faster-whisper-large-v3-turbo` vì đã rename → `dropbox-dash/faster-whisper-large-v3-turbo` (ownership không rõ, rủi ro biến mất).

## Overview
- Priority: P2
- Status: pending
- Thêm 1 entry model mới vào catalog `MODELS` (sidecar) + `BASE_OPTIONS` (frontend) — chạy chung backend `ct2` như small/medium/large hiện có, KHÔNG cần thay đổi runtime/dependency nào. Chạy được cả CPU (int8, chậm hơn nhưng vẫn dùng được) và GPU (nhanh, sau Phase 02) — KHÔNG gate theo `cuda_supported` (khác `large-v3-mlx` vốn cần backend riêng chỉ có trên macOS).

## Key Insights
- Turbo dùng backend `ct2` giống PhoWhisper — không thêm vào `MLX_SIZES`, không cần gating platform. Xuất hiện cho MỌI OS (Windows/macOS/Linux) giống small/medium/large.
- `subdir=""` giống `large` (repo `kiendt/PhoWhisper-large-ct2`) → không có nguy cơ MAX_PATH Windows (path ngắn, không subfolder repo dài).
- `model.bin` = 1,617,884,929 bytes (~1.618GB) — dùng làm cơ sở ước lượng `total_bytes` cho progress bar tải (Phase 04 touch `transcribe.rs`, nhưng field này khai ở đây vì gắn với catalog model, xem Related Code Files).
- Turbo là model **tiếng Anh gốc đa ngôn ngữ** (không train riêng tiếng Việt như PhoWhisper) — khác PhoWhisper về đối tượng dùng: hợp với nội dung Việt-Anh lẫn (DevOps/kỹ thuật) theo đúng lý do user chọn, KHÔNG thay thế PhoWhisper cho nội dung tiếng Việt thuần (CER kém hơn PhoWhisper theo research). UI label phải truyền tải đúng điều này để người dùng chọn đúng use-case.

## Requirements
### Functional
- `model_paths.MODELS` thêm key `"large-v3-turbo"` → `("deepdml/faster-whisper-large-v3-turbo-ct2", "")`.
- `asr-models.ts` `ModelSize` type + `BASE_OPTIONS` thêm entry turbo (hiện luôn, không gate platform).
- `SIZE_HINT` thêm `"large-v3-turbo": "~1.6GB"`.
- `transcribe.rs`: `ALL_SIZES` thêm `"large-v3-turbo"`; `total_bytes()` thêm case turbo (~1,622,000,000.0); `model_dir()` không cần case riêng (rơi vào nhánh `other => other`, leaf = `"large-v3-turbo"`, đủ ngắn).

### Non-functional
- KHÔNG đổi hành vi 3 model PhoWhisper hiện có — chỉ thêm entry mới, mọi dict/array/match hiện tại giữ nguyên các case cũ.

## Architecture
Không có kiến trúc mới — tái dùng 100% pipeline ct2 hiện có (`get_model`/`_transcribe_ct2`/`ensure_model`/`resolve_model_path` đều generic theo `size`, không hardcode danh sách size ở logic, chỉ ở data — xem `model_paths.MODELS`, `transcribe.rs::ALL_SIZES`, `asr-models.ts::BASE_OPTIONS`).

## Related Code Files
- **Modify:**
  - `sidecar/transcriber/model_paths.py` (dict `MODELS` dòng 12-17)
  - `src/composables/asr-models.ts` (`ModelSize` dòng 8, `BASE_OPTIONS` dòng 12-16, `SIZE_HINT` dòng 24-29)
  - `src-tauri/src/transcribe.rs` (`total_bytes()` dòng 14-20, `ALL_SIZES` dòng 120)

## Implementation Steps
1. `model_paths.py` dòng 12-17: thêm `"large-v3-turbo": ("deepdml/faster-whisper-large-v3-turbo-ct2", ""),` vào dict `MODELS` (giữ nguyên comment docstring đầu file, bổ sung 1 dòng giải thích turbo = ct2, không phải MLX).
2. `asr-models.ts` dòng 8: `export type ModelSize = "small" | "medium" | "large" | "large-v3-turbo" | "large-v3-mlx";`
3. `asr-models.ts` dòng 12-16 (`BASE_OPTIONS`): thêm
   ```ts
   { title: "Whisper Large-v3-turbo — nhanh, hợp nội dung Việt-Anh lẫn (kỹ thuật/DevOps)", value: "large-v3-turbo" },
   ```
4. `asr-models.ts` dòng 24-29 (`SIZE_HINT`): thêm `"large-v3-turbo": "~1.6GB",`.
5. `transcribe.rs` dòng 14-20 (`total_bytes`): thêm case
   ```rust
   "large-v3-turbo" => 1_622_000_000.0,
   ```
   (đặt trước nhánh `_ => ... // large`, giữ đúng thứ tự match).
6. `transcribe.rs` dòng 120 (`ALL_SIZES`): đổi `[&str; 4]` → `[&str; 5]`, thêm `"large-v3-turbo"`.
7. `npm run build` (vue-tsc --noEmit) để bắt lỗi type nếu có nơi khác switch/match trên `ModelSize` chưa cover case mới (TS union type sẽ báo lỗi tại compile nếu có).
8. `cargo check` (hoặc `npm run tauri build --debug` nhanh hơn khi dev) để bắt lỗi Rust nếu match nào khác thiếu arm.

## Todo List
- [ ] `model_paths.py`: thêm entry `large-v3-turbo`
- [ ] `asr-models.ts`: type + BASE_OPTIONS + SIZE_HINT
- [ ] `transcribe.rs`: `total_bytes` + `ALL_SIZES`
- [ ] `npm run build` pass (không lỗi TS)
- [ ] `cargo check` (trong `src-tauri/`) pass
- [ ] Test tải model turbo qua UI Mode C (Phase 06 sẽ hiện UI, nhưng flow tải qua `ensure_model` đã generic — test được ngay bằng cách chọn size trong dropdown hiện có trước khi Phase 06 xong)

## Success Criteria
- Dropdown model (Mode A + C) hiện thêm option turbo, tải về đúng repo `deepdml/faster-whisper-large-v3-turbo-ct2`, `model_status`/`model_path`/`list_models`/`delete_model` hoạt động đúng cho size mới (không cần sửa gì thêm — các hàm này đã generic theo `ALL_SIZES`).
- Transcribe bằng turbo trên CPU int8 chạy được (không crash) — verify trước khi có GPU.
- `npm run build` + `cargo check` xanh, không lỗi type do thiếu match arm.

## Risk Assessment
| Risk | Likelihood × Impact | Mitigation |
|---|---|---|
| Repo HF đổi cấu trúc/xoá | Thấp × TB | đã verify snapshot hiện tại; nếu gãy, đổi 1 dòng trong `MODELS` dict sang repo khác cùng format |
| Turbo model tiếng Việt kém hơn PhoWhisper gây hiểu lầm "model tệ" | TB × Thấp | label UI ghi rõ use-case ("hợp nội dung Việt-Anh lẫn"), không quảng cáo là thay thế PhoWhisper |

## Security Considerations
- Không đổi — tải qua `huggingface_hub.snapshot_download` giống các model khác, cùng threat model đã chấp nhận.

## Next Steps
- Phase 04 không cần đụng lại các file này (đã xong ở đây) — chỉ cần biết `ALL_SIZES` đã có 5 entry khi viết code CUDA plumbing để tránh conflict base.
- Phase 06 UI tự động hiện option turbo qua `sizeOptions` (không cần code UI riêng cho turbo).
