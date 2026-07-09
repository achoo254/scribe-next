# Phase 02 — GPU Device Selection trong sidecar (main.py get_model)

## Context Links
- Overview: [plan.md](plan.md)
- Depends on: [phase-01-cuda-runtime-downloader.md](phase-01-cuda-runtime-downloader.md) (cần `cuda_dir` convention + marker)
- Research: mục 2 (Blackwell int8 bug), mục 1 (float16 ~5GB VRAM đủ) trong research report.

## Overview
- Priority: P1 (blocker chính — không có bước này thì tải CUDA runtime xong cũng vô nghĩa)
- Status: pending
- Sửa `get_model()` trong `main.py` (dòng 71-85 hiện tại) để chọn `device="cuda"` + `compute_type="float16"` khi được yêu cầu VÀ CUDA runtime đã sẵn sàng; fallback CPU int8 mọi trường hợp khác (giữ nguyên hành vi cũ 100% khi không bật GPU).

## Key Insights
- **BẮT BUỘC `compute_type="float16"` trên GPU** — Blackwell sm_120 crash `CUBLAS_STATUS_NOT_SUPPORTED` với int8/int8_float16 (research report mục 2, verified qua SubtitleEdit #10180 + faster-whisper #1086). KHÔNG cho phép người dùng chọn compute_type khác trên GPU ở bước này (YAGNI — không expose setting này ra UI).
- `os.add_dll_directory(cuda_dir)` PHẢI gọi TRƯỚC lần `import ctranslate2`/`faster_whisper` ĐẦU TIÊN của process (Python cache module trong `sys.modules` — gọi lại `from faster_whisper import ...` các lần sau không re-trigger DLL search). `get_model()` hiện tại import lazy bên trong hàm (dòng 77) → đúng chỗ để chèn `add_dll_directory` ngay trước, nhưng phải guard "chỉ add 1 lần" vì hàm này có thể chạy lại khi đổi size.
- Cache hiện tại (`_model`, `_model_size` — dòng 30-31) CHỈ theo `size`, KHÔNG theo device. Nếu người dùng bật/tắt toggle GPU giữa 2 lần transcribe cùng size → cache cũ (device sai) bị dùng nhầm. PHẢI thêm `_model_device` vào điều kiện reload.
- GPU init có thể fail runtime (driver lỗi, DLL thiếu do tải dở/xoá tay) — KHÔNG để crash cả file transcribe; try/except quanh `WhisperModel(..., device="cuda", ...)`, fail → log info + fallback CPU int8 NGAY LẦN GỌI ĐÓ (không cache trạng thái "GPU hỏng" vĩnh viễn — cho phép thử lại lần sau, vì lỗi có thể tạm thời).
- `hasattr(os, "add_dll_directory")` — hàm này chỉ tồn tại Windows Python ≥3.8. Guard để không lỗi trên macOS/Linux (dù nhánh cuda chỉ kích hoạt khi `use_gpu=True` từ Rust, và Rust chỉ set `use_gpu=True` khi `cuda_supported()`=true, tức chỉ Windows+NVIDIA — vẫn guard cho chắc, tránh main.py crash nếu logic Rust có bug).

## Requirements
### Functional
- `get_model(model_dir, size, use_gpu=False, cuda_dir=None)` — mở rộng signature (giữ backward-compat: gọi không kèm 2 param mới vẫn chạy CPU như cũ).
- `transcribe()`/`_transcribe_ct2()` (main.py dòng 170-190, 238-248) forward `use_gpu`/`cuda_dir` từ request JSON xuống `get_model`.
- `main()` dispatch `cmd == "transcribe"` (dòng 305-309) đọc thêm `req.get("use_gpu", False)`, `req.get("cuda_dir")`.

### Non-functional
- Đường CPU (use_gpu=False hoặc thiếu field — request cũ từ Rust chưa update) phải chạy **y hệt** hành vi hiện tại — không đổi compute_type/device mặc định.
- File mới `gpu_device.py` < 200 dòng, tách khỏi `main.py` (đang 318 dòng, đã vượt ngưỡng modularize — không thêm logic mới vào đây ngoài vài dòng gọi hàm).

## Architecture
```
main.py::get_model(model_dir, size, use_gpu, cuda_dir)
   ├─ device = "cuda" if use_gpu else "cpu"
   ├─ nếu (_model is None) OR (_model_size != size) OR (_model_device != device):
   │     base, actual_device, actual_compute = gpu_device.load_whisper_model(
   │         resolve_model_path(model_dir, size), use_gpu, cuda_dir, cpu_threads=os.cpu_count() or 8)
   │     _model = BatchedInferencePipeline(model=base)
   │     _model_size, _model_device = size, actual_device
   │     send info: f"Đang dùng {actual_device.upper()} ({actual_compute}) — model {size}"
   └─ return _model

gpu_device.py::load_whisper_model(model_path, use_gpu, cuda_dir, cpu_threads)
   ├─ use_gpu và cuda_dir và cuda_dir chứa "_cuda_ready.marker"?
   │     ├─ True → _prepare_dll_dir(cuda_dir) (idempotent, hasattr guard)
   │     │         try: WhisperModel(model_path, device="cuda", compute_type="float16")
   │     │              → return (model, "cuda", "float16")
   │     │         except Exception → send info "GPU lỗi, fallback CPU" → rơi xuống nhánh CPU
   │     └─ False (chưa sẵn sàng) → send info "GPU chưa sẵn sàng (chưa tải CUDA runtime) — dùng CPU"
   └─ nhánh CPU (mặc định/fallback): WhisperModel(model_path, device="cpu", compute_type="int8", cpu_threads=cpu_threads)
                                       → return (model, "cpu", "int8")
```

## Related Code Files
- **Create:** `sidecar/transcriber/gpu_device.py`
- **Modify:** `sidecar/transcriber/main.py`:
  - `get_model()` dòng 71-85 (thêm param, đổi logic nạp model)
  - `_transcribe_ct2()` dòng 170-176 (forward use_gpu/cuda_dir)
  - `transcribe()` dòng 238-248 (forward tiếp từ tham số hàm)
  - `main()` dòng 305-309 (đọc field mới từ request)
  - Thêm `_model_device = None` cạnh dòng 31

## Implementation Steps
1. Tạo `gpu_device.py`: hằng `MARKER_NAME = "_cuda_ready.marker"` (trùng Phase 01 — import từ `cuda_runtime` để KHÔNG lặp hằng số, giữ DRY: `from cuda_runtime import MARKER_NAME`).
2. Hàm `_cuda_ready(cuda_dir) -> bool`: `cuda_dir and os.path.isfile(os.path.join(cuda_dir, MARKER_NAME))`.
3. Module-level flag `_dll_dir_added = False`; hàm `_prepare_dll_dir(cuda_dir)`: nếu chưa add và `hasattr(os, "add_dll_directory")` → `os.add_dll_directory(cuda_dir)`, set flag True.
4. Hàm `load_whisper_model(model_path, use_gpu, cuda_dir, cpu_threads, send_info)`:
   - `send_info` là callback (truyền `send` từ main.py vào, tránh gpu_device.py phải import main.py → tránh circular import).
   - Nếu `use_gpu` và `_cuda_ready(cuda_dir)`: `_prepare_dll_dir(cuda_dir)`; `from faster_whisper import WhisperModel`; try tạo `WhisperModel(model_path, device="cuda", compute_type="float16")` → return `(model, "cuda", "float16")`; except Exception as e → `send_info(f"GPU lỗi ({e}), dùng CPU")`, rơi tiếp xuống bước 5.
   - Nếu `use_gpu` nhưng KHÔNG `_cuda_ready` → `send_info("GPU chưa sẵn sàng (chưa tải CUDA runtime) — dùng CPU")`.
   - Bước 5 (CPU, mặc định/fallback): `from faster_whisper import WhisperModel`; `WhisperModel(model_path, device="cpu", compute_type="int8", cpu_threads=cpu_threads)` → return `(model, "cpu", "int8")`.
5. Sửa `main.py::get_model`:
   ```python
   _model_device = None  # cạnh dòng 31

   def get_model(model_dir, size, use_gpu=False, cuda_dir=None):
       global _model, _model_size, _model_device
       device_key = "cuda" if use_gpu else "cpu"
       if _model is None or _model_size != size or _model_device != device_key:
           from faster_whisper import BatchedInferencePipeline
           import gpu_device
           base, actual_device, actual_compute = gpu_device.load_whisper_model(
               resolve_model_path(model_dir, size), use_gpu, cuda_dir,
               os.cpu_count() or 8, send_info=lambda m: send({"type": "info", "message": m}),
           )
           _model = BatchedInferencePipeline(model=base)
           _model_size, _model_device = size, actual_device
           send({"type": "info", "message": f"Đang dùng {actual_device.upper()} ({actual_compute}) — model {size}"})
       return _model
   ```
6. Sửa `_transcribe_ct2(file, model_dir, size, prompt, use_gpu=False, cuda_dir=None)` → forward vào `get_model(model_dir, size, use_gpu, cuda_dir)`.
7. Sửa `transcribe(file, out_dir, outputs, model_dir, size, prompt=None, use_gpu=False, cuda_dir=None)` → forward tiếp xuống `_transcribe_ct2`/`_transcribe_mlx` (mlx branch bỏ qua use_gpu/cuda_dir — không áp dụng, giữ nguyên tham số nhưng không dùng).
8. Sửa `main()` dispatch `cmd == "transcribe"`: thêm `req.get("use_gpu", False), req.get("cuda_dir")` vào cuối lệnh gọi `transcribe(...)`.

## Todo List
- [ ] Tạo `gpu_device.py` với `_cuda_ready`, `_prepare_dll_dir`, `load_whisper_model`
- [ ] Sửa `get_model` + cache key theo device
- [ ] Forward `use_gpu`/`cuda_dir` qua `_transcribe_ct2` → `transcribe` → dispatch `main()`
- [ ] Test CPU path KHÔNG đổi hành vi (chạy transcribe cũ, không truyền use_gpu → vẫn int8/CPU)
- [ ] Test GPU path trên máy thật (sau Phase 01 xong, có marker) — xem info message đúng "cuda (float16)"
- [ ] Test fallback: xoá 1 DLL cuDNN giả lập lỗi → xác nhận fallback CPU, không crash

## Success Criteria
- Request `transcribe` KHÔNG có `use_gpu` (client cũ) → chạy CPU int8 y như trước khi có phase này (regression zero).
- Request `use_gpu=true` + CUDA runtime sẵn sàng → info message báo "cuda (float16)", transcribe ra kết quả đúng (so sánh text với bản CPU cùng file — sai lệch nhỏ do precision, không sai nghĩa).
- Tắt/bật toggle GPU giữa 2 lần transcribe cùng size → model được nạp lại đúng device (không dùng nhầm cache).
- GPU init lỗi giả lập → fallback CPU, không có exception nào thoát ra ngoài (`main()` không nhận `{"type":"error"}` cho case này, transcribe vẫn "done").

## Risk Assessment
| Risk | Likelihood × Impact | Mitigation |
|---|---|---|
| Blackwell crash nếu lỡ dùng int8/int8_float16 trên GPU | Thấp (đã hardcode float16, không expose setting) × Cao | Code hardcode `compute_type="float16"` khi `device="cuda"`, không tham số hoá |
| add_dll_directory gọi sai thứ tự (sau khi ctranslate2 đã import ở nơi khác) | Thấp × Cao | Toàn bộ import `faster_whisper`/`ctranslate2` trong sidecar CHỈ xảy ra lazy trong `get_model`/`gpu_device.load_whisper_model` — không có import top-level nào khác (đã grep xác nhận `main.py` không import faster_whisper ở module level) |
| Cache stale (device cũ) | TB × TB | `_model_device` thêm vào điều kiện reload (bước 5) |

## Security Considerations
- Không thay đổi bề mặt tấn công — model vẫn load từ path cục bộ do Rust chỉ định, không nhận input path từ nguồn không tin cậy (giữ nguyên threat model hiện tại của app desktop 1 người dùng).

## Next Steps
- Phase 04 (Rust) cần gửi đúng field `use_gpu` (bool) + `cuda_dir` (string path) trong JSON `transcribe` cmd, khớp field name Python đọc ở bước 8.
- Phase 07 verify thực tế tốc độ GPU vs CPU trên PhoWhisper-large + turbo.
