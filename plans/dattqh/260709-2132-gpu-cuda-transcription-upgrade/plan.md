---
title: "Bật GPU CUDA transcript + model turbo + glossary DevOps"
description: "Bật CUDA cho faster-whisper sidecar (RTX 5070 Ti Blackwell), thêm model whisper-large-v3-turbo, xác nhận glossary DevOps mặc định"
status: completed
priority: P2
effort: 10h
branch: main
tags: [gpu, cuda, transcription, sidecar, whisper, faster-whisper]
created: 2026-07-09
---

# GPU CUDA Transcription Upgrade — Overview

> **Đóng plan (2026-10-19):** phase 01–07 đã làm và kiểm trên RTX 5070 Ti (2026-07-09). App desktop (Tauri + sidecar) đã bỏ
> ngày 2026-10-16 và chép lời nay chạy trên server bằng ElevenLabs, nên bước QA GUI còn lại không còn áp dụng.

Research: `plans/dattqh/reports/gpu-transcription-model-upgrade-260709-2124-rtx5070ti-report.md`

**Phát hiện quan trọng lúc lập plan:** Quyết định #5 (glossary DevOps mặc định) **ĐÃ triển khai** ở commit `5e05433` (2026-07-08) — `settings.rs:34-35,58-66,68-86` đã có `default_glossary()` y hệt `EXAMPLE_PROMPT` cũ. Phase 05 chỉ còn verify + dọn code chết, không phải implement mới.

**Phát hiện bổ sung (không có trong research report):** khảo sát 5 GitHub issue của faster-whisper trên Windows (SYSTRAN/faster-whisper #535, #1276, #1230, #1279, #575) chỉ báo thiếu `cublas64_12.dll`/`cublasLt64_12.dll`/`cudnn_*64_9.dll` — KHÔNG báo thiếu `cudart64_12.dll` → CTranslate2 build Windows tĩnh hoá (static-link) CUDA runtime. Giữ đúng 2 wheel (cublas+cudnn) như research; có fallback thêm `nvidia-cuda-runtime-cu12` (~3.6MB, đã verify tồn tại trên PyPI win_amd64) nếu Phase 07 phát hiện thiếu.

Repo model turbo đã chốt: `deepdml/faster-whisper-large-v3-turbo-ct2` (flat, `model.bin` 1,617,884,929 bytes, MIT) — KHÔNG dùng `mobiuslabsgmbh/faster-whisper-large-v3-turbo` (đã rename → `dropbox-dash/...`, ownership không rõ ràng).

## Phases

| # | Phase | Status | Deps | Effort | Files chính |
|---|-------|--------|------|--------|--------------|
| 01 | [CUDA runtime downloader](phase-01-cuda-runtime-downloader.md) | ✅ done | — | 2h | `sidecar/transcriber/cuda_runtime.py` (mới) |
| 02 | [GPU device selection sidecar](phase-02-gpu-device-selection-sidecar.md) | ✅ done | 01 | 1.5h | `sidecar/transcriber/gpu_device.py` (mới), `main.py` |
| 03 | [Model whisper-large-v3-turbo](phase-03-model-whisper-large-v3-turbo.md) | ✅ done | — | 1h | `model_paths.py`, `asr-models.ts`, `transcribe.rs` |
| 04 | [Rust CUDA plumbing](phase-04-rust-cuda-plumbing.md) | ✅ done | 01,02,03 | 2.5h | `cuda.rs` (mới), `settings.rs`, `transcribe.rs`, `commands.rs`, `lib.rs` |
| 05 | [Glossary DevOps — verify](phase-05-glossary-devops-verify.md) | ✅ done | — | 0.25h | `main.py` (đã dọn dead `EXAMPLE_PROMPT`) |
| 06 | [UI toggle GPU](phase-06-ui-gpu-toggle.md) | ✅ done | 03,04 | 1.5h | `TranscriptView.vue`, `use-gpu.ts` (mới) |
| 07 | [Test & verify trên máy thật](phase-07-test-verify.md) | ✅ done | 01-06 | 1.5h | verify GPU float16 trên RTX 5070 Ti |

**Đã hoàn tất (2026-07-09):** implement Phase 01-07; cargo check + vue-tsc + py_compile xanh; code-review (1 Critical + 1 High + 1 Medium + Low) đã fix.

**Phase 07 verify TRÊN MÁY THẬT (RTX 5070 Ti):**
- Downloader Phase 01 chạy thật: tải + giải nén DLL + marker, không rác `.part/.whl`.
- **Tối ưu dung lượng (user chọn)**: đo `EnumProcessModules` → ct2 4.8 chỉ nạp cublas + cublasLt, KHÔNG dùng cuDNN. Bỏ `nvidia-cudnn-cu12` → **tải 553MB (từ 1.29GB), đĩa ~736MB (từ ~1.8GB), giảm ~57%**. Verify: dời cuDNN ra ngoài, GPU vẫn chạy 73× realtime (kể cả luồng sidecar production word_timestamps).
- **Bug tìm ra + fix**: `os.add_dll_directory` KHÔNG đủ — CTranslate2 nạp cublas/cudnn theo TÊN qua LoadLibrary, chỉ tra PATH. Fix: `gpu_device._prepare_dll_dir` prepend `cuda_dir` vào `os.environ["PATH"]` (verified). Không có fix này → "cublas64_12.dll cannot be loaded" lúc encode.
- **Kết quả**: PhoWhisper-large `cuda/float16` chạy KHÔNG crash CUBLAS trên Blackwell. Warm run 60s audio: **GPU 0.82s (73× realtime) vs CPU int8 11.95s (5×) → nhanh x14.5**.
- Report: `plans/dattqh/reports/gpu-phase07-realmachine-verify-260709-2223-rtx5070ti-report.md`.

**Còn lại (QA thủ công của user):** chạy app GUI thật (npm run tauri:dev) → bật toggle GPU trong UI → tải runtime qua nút → transcribe file thật để xác nhận luồng UI→Rust→sidecar end-to-end (core đã verify qua sidecar code path).

**Thứ tự thực thi:** 01 → 02 → 03 → 04 → 06 → 07 (tuần tự, Phase 03 đặt trước 04 để tránh 2 phase cùng sửa `transcribe.rs` lệch base). Phase 05 làm bất kỳ lúc nào (độc lập, không đụng file ai).

## Dependency Graph

```
01 (cuda downloader) ──► 02 (device select) ──┐
                                                ├──► 04 (rust plumbing) ──► 06 (UI) ──► 07 (verify)
03 (turbo model, độc lập) ─────────────────────┘
05 (glossary verify, độc lập) ─────────────────────────────────────────────────────────► 07
```

## File Ownership (không phase nào chạy song song thật — vẫn khai để tránh nhầm base khi review diff)

- Phase 01: CREATE `sidecar/transcriber/cuda_runtime.py`; touch `main.py` (chỉ thêm dispatch cmd `ensure_cuda`)
- Phase 02: CREATE `sidecar/transcriber/gpu_device.py`; touch `main.py` (`get_model`)
- Phase 03: touch `model_paths.py`, `asr-models.ts`, `transcribe.rs` (`ALL_SIZES`/`total_bytes`)
- Phase 04: CREATE `src-tauri/src/cuda.rs`; touch `settings.rs`, `transcribe.rs`, `commands.rs`, `lib.rs`
- Phase 05: touch `main.py` (xoá `EXAMPLE_PROMPT` chết, KHÔNG đụng logic)
- Phase 06: CREATE `src/composables/use-gpu.ts`; touch `TranscriptView.vue`

## Top Risks (chi tiết từng phase xem file phase-XX)

| Risk | L×I | Mitigation |
|---|---|---|
| Blackwell int8 crash CUBLAS_STATUS_NOT_SUPPORTED | Cao×Cao | BẮT BUỘC `compute_type="float16"` trên GPU (Phase 02), không bao giờ int8/int8_float16 |
| GPU load fail giữa chừng (driver/DLL lỗi) làm transcribe hỏng | TB×Cao | try/except fallback CPU int8 trong `gpu_device.py`, KHÔNG crash cả file (Phase 02) |
| Model cache (`_model`/`_model_size`) không phân biệt device → dùng nhầm model đã nạp sai device | TB×TB | thêm `_model_device` vào điều kiện reload (Phase 02) |
| Thiếu `cudart64_12.dll` ngoài dự kiến | Thấp×TB | fallback tải thêm `nvidia-cuda-runtime-cu12` (Phase 07 nếu gặp) |

## Rollback

Mọi phase chỉ ADD code mới hoặc thêm field `#[serde(default)]`/optional — không đổi hành vi CPU path hiện tại. Rollback = revert commit của phase đó; máy không NVIDIA / user không bật toggle GPU không thấy khác biệt gì (đường CPU int8 giữ nguyên 100%).

## Resolved Decisions (user chốt 2026-07-09)

- **Model mặc định khi CÓ GPU = `whisper-large-v3-turbo`** (nội dung Việt-Anh code-switch DevOps → Whisper xử lý code-switch tốt hơn PhoWhisper thuần). Khi bật GPU lần đầu & user chưa chọn model thủ công → default sang turbo. Máy CPU giữ default medium như cũ. Ảnh hưởng: Phase 04 (settings default theo device) + Phase 06 (UI).

## Unresolved Questions

1. Root cause 5070 Ti chậm hơn 4070 Ti Super (issue #1287) — chưa chốt, chỉ benchmark thực tế ở Phase 07 mới có số liệu áp dụng cho máy này.
