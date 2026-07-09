# Phase 07 — Verify GPU CUDA transcript trên máy thật (RTX 5070 Ti)

> Thời điểm: 2026-07-09 22:23 (Asia/Saigon) · Máy: RTX 5070 Ti 16GB (Blackwell sm_120), Ryzen 9 9900X, driver 591.86
> Verify qua ĐÚNG code path sidecar (`gpu_device.load_whisper_model`), không reimplement.

## Executive Summary

Feature bật GPU CUDA cho transcript **hoạt động trên máy thật**. PhoWhisper-large chạy `cuda/float16` KHÔNG crash CUBLAS trên Blackwell — rủi ro kỹ thuật lớn nhất (đã cảnh báo từ research) được gỡ. Tốc độ warm: **GPU 73× realtime vs CPU 5× → nhanh ~14.5×**.

Phase 07 tìm ra **1 bug thật** không lộ ở compile/review: `os.add_dll_directory` không đủ để CTranslate2 tìm cublas/cudnn DLL — đã fix bằng prepend PATH + verify lại.

## Môi trường sẵn có (không phải cài mới)
- Sidecar venv: `ctranslate2 4.8.0` + `faster-whisper 1.2.1` (build CUDA-enabled), `get_cuda_device_count()=1`.
- Model `PhoWhisper-large-ct2` đã tải sẵn trong app-data.

## Kết quả verify

### 1. Downloader CUDA runtime (Phase 01) — PASS
- Chạy `python cuda_runtime.py <cuda_dir>` thật: tải 2 wheel PyPI (~1.29GB), verify sha256, giải nén.
- Kết quả: **13 DLL** (`cublas64_12`, `cublasLt64_12` 668MB, `cudnn*64_9` family, `nvblas64_12`) + `_cuda_ready.marker`, **0 file `.part/.whl` sót** (cleanup đúng).
- Disk sau giải nén: **~1.8GB** (lớn hơn 1.29GB tải nén — `cublasLt` 668MB + `cudnn_engines_precompiled` 547MB chiếm phần lớn). ⚠️ Con số disk footprint thực tế ~1.8GB, không phải 1.3GB (1.3GB là dung lượng tải).
- File `.part` ghi ĐÚNG trong `cuda_dir` → validate cơ chế poll `dir_size` của Rust (Phase 04).

### 2. Bug tìm ra: add_dll_directory KHÔNG đủ cho CTranslate2 — FIXED
- Triệu chứng: `RuntimeError: Library cublas64_12.dll is not found or cannot be loaded` lúc `model.encode()`, dù DLL có trong `cuda_dir` + `add_dll_directory` đã gọi.
- Chẩn đoán (empirical):
  - `ctypes.WinDLL(<full_path>/cublas64_12.dll)` → OK (deps đủ, cudart KHÔNG cần — static-link, đúng như khảo sát).
  - `ctypes.WinDLL("cublas64_12.dll")` sau `add_dll_directory` → OK.
  - Nhưng CTranslate2 vẫn fail → **CTranslate2 nạp cublas/cudnn theo TÊN qua LoadLibrary, KHÔNG tôn trọng `add_dll_directory`** (chỉ tra PATH + thư mục chuẩn).
- Fix: `gpu_device._prepare_dll_dir` prepend `cuda_dir` vào `os.environ["PATH"]` (giữ `add_dll_directory` — belt & suspenders). Verified lại → PASS.
- **Bài học**: đây là bug chỉ lộ khi chạy thật; compile/typecheck/review không bắt được. Xác nhận giá trị của Phase 07 verify trên hardware.

### 3. GPU float16 trên Blackwell — PASS (không crash CUBLAS)
- `WhisperModel(device="cuda", compute_type="float16")` + `transcribe()` chạy trọn vẹn, không `CUBLAS_STATUS_NOT_SUPPORTED`.
- Xác nhận quyết định "float16 bắt buộc trên Blackwell" là đúng và đủ.

### 4. Benchmark tốc độ (warm run, bỏ cold-start/cuDNN autotune)

| Clip | Device | Warm run | Realtime factor |
|---|---|---|---|
| 60s (tone+pink noise) | GPU cuda/float16 | **0.82s** | **73×** |
| 60s | CPU int8 (24 thread) | 11.95s | 5× |
| | **Speedup** | | **~14.5×** |

- Lần chạy GPU ĐẦU (cold): 12s clip 6.13s vs CPU 4.57s — **GPU chậm hơn** do cold-start + cuDNN JIT autotune + audio quá ngắn. Đây là lý do phải đo warm run + clip đủ dài. Con số warm (73×) mới phản ánh dùng thật.
- Lưu ý: clip test là tone+noise (không phải speech) → decode ngắn, GPU còn nhanh hơn nữa trên speech thật (decode autoregressive dài, GPU lợi thế lớn hơn).

## Known limitations (chấp nhận / để sau)
1. GPU init lỗi được bắt ở lúc CONSTRUCT model (try/except trong `gpu_device`), nhưng cublas nạp LAZY lúc `encode` (inference) → nếu DLL lỗi lúc inference, file đó báo error (không tự fallback CPU giữa chừng). Với marker + PATH fix, kịch bản này rất khó xảy ra thực tế → không over-engineer (YAGNI). Ghi nhận.
2. Cold-start lần transcribe GPU đầu tiên/session chậm (~cuDNN autotune vài giây). Không chặn; steady-state rất nhanh.

## Tối ưu dung lượng (áp dụng sau verify — user chọn)

Đo qua `EnumProcessModules` khi GPU transcribe (encoder conv + decoder attn + word_timestamps + beam=5, 2 clip): CTranslate2 4.8 **CHỈ nạp `cublas64_12` + `cublasLt64_12`**, KHÔNG nạp cuDNN nào. Verify dứt khoát: dời toàn bộ cuDNN ra khỏi `cuda_dir` → GPU transcribe (kể cả luồng sidecar production `word_timestamps=True`) vẫn chạy 73× realtime, info "Đang dùng CUDA (float16)".

→ Bỏ `nvidia-cudnn-cu12` khỏi `CUDA_PACKAGES` (`cuda_runtime.py`) + `CUDA_TOTAL_BYTES` (`cuda.rs`) = 553.2MB.

| | Trước | Sau | Giảm |
|---|---|---|---|
| Tải PyPI | 1.29GB | **553MB** | -57% |
| Đĩa sau giải nén | ~1.8GB | **~736MB** | -59% |

`cublasLt64_12.dll` (668MB) là dep bắt buộc của cublas → không giảm thêm được (sàn ~736MB). Coupling: nếu nâng ct2 lên bản cần cuDNN → thêm lại package (đã ghi comment cả 2 file). Bản cài cũ (đã tải 2 package) giữ nguyên marker → không tải lại, cuDNN thừa nằm im (vô hại); muốn thu hồi thì xóa `cuda_dir`.

## Unresolved Questions
1. Luồng UI end-to-end (Tauri GUI: toggle GPU → nút tải runtime → transcribe) CHƯA chạy qua GUI thật — mới verify tầng sidecar code path. Cần user QA thủ công `npm run tauri:dev`.
2. Root cause 5070 Ti chậm hơn 4070 Ti Super (issue #1287 từ research) — benchmark này cho thấy GPU rất nhanh (73×), không thấy vấn đề; issue đó có thể đã fix ở ct2 4.8.0 hoặc chỉ ảnh hưởng workload khác.
