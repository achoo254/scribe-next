# Research Report: Nâng cấp model transcript tận dụng GPU RTX 5070 Ti (OneScribe)

> Thời điểm research: 2026-07-09 21:24 (Asia/Saigon) · Gemini: tắt → dùng WebSearch

## Executive Summary

Máy có **RTX 5070 Ti 16GB VRAM** (Blackwell, sm_120) + Ryzen 9 9900X + 61GB RAM — thừa sức chạy model transcript lớn nhất trên GPU. **Hiện app đang bỏ không GPU hoàn toàn**: sidecar `faster-whisper` (CTranslate2) chỉ chạy **CPU int8**, PhoWhisper small/medium/large.

**Kết luận thẳng:** thứ bạn thiếu KHÔNG phải "model xịn hơn" — mà là **bật GPU cho model đang có**. `PhoWhisper-large` (đã có sẵn trong app, CER tiếng Việt tốt nhất) hiện quá chậm trên CPU nên mặc định để medium. Bật CUDA → large chạy nhanh gấp ~4× CPU, thành model mặc định khả thi. Đây là "win" lớn nhất, chi phí thấp nhất.

**Cảnh báo kỹ thuật quan trọng:** RTX 50-series (Blackwell sm_120) có **bug int8 với CTranslate2/cuBLAS** — crash `CUBLAS_STATUS_NOT_SUPPORTED` nếu để int8 mặc định. **Bắt buộc dùng `compute_type=float16` trên GPU này** (16GB VRAM thừa sức). Đây là điều kiện tiên quyết, không phải tùy chọn.

Model "xịn hơn thật sự" (parakeet Vietnamese của NVIDIA) tồn tại nhưng đổi runtime (NeMo, không phải CTranslate2) → đại phẫu kiến trúc, YAGNI ở giai đoạn này.

---

## Machine (verified qua nvidia-smi + WMI)

| Thành phần | Giá trị | Ghi chú |
|---|---|---|
| GPU | RTX 5070 Ti, **16303 MiB** | Blackwell, **compute 12.0 / sm_120**, driver 591.86 |
| CPU | Ryzen 9 9900X | 12C/24T, Zen 5 |
| RAM | 61.6 GB | |
| CUDA Toolkit | **chưa cài** (`nvcc` not found) | chỉ có driver — đủ để chạy, KHÔNG đủ để build CTranslate2 từ nguồn |

> Lưu ý: `Win32_VideoController` báo VRAM 4GB là sai (giới hạn WMI với >4GB). `nvidia-smi` báo đúng 16GB.

## Hiện trạng transcript (từ README + sidecar)

- Runtime: **faster-whisper (CTranslate2), CPU int8**, PyInstaller onefile.
- Model: PhoWhisper **small/medium/large** (mặc định medium). small/medium từ `quocphu/PhoWhisper-ct2-FasterWhisper`, large từ `kiendt/PhoWhisper-large-ct2`.
- macOS Apple Silicon có thêm Whisper Large-v3 (MLX/Metal). **Windows/NVIDIA: 0% GPU.**

---

## Key Findings

### 1. GPU nhanh hơn CPU rõ rệt — và model đang có chạy được ngay trên GPU
- faster-whisper GPU nhanh **~4× CPU**; `large-v3 int8` trên RTX 4070 đạt **~12× realtime, ~2.5GB VRAM**. 5070 Ti mạnh hơn 4070.
- **Model PhoWhisper ct2 đã tải KHÔNG cần tải lại** — CTranslate2 chọn CPU/GPU lúc load (`device="cuda"`), tự upcast precision. Chỉ cần đổi device + có thư viện CUDA.
- `large` float16 cần ~5GB VRAM → 16GB thừa sức (có thể bật cả batching).

### 2. [BLOCKER] Blackwell sm_120 + CTranslate2: bug int8, phải float16
- RTX 50-series **crash `CUBLAS_STATUS_NOT_SUPPORTED`** khi để int8 mặc định — int8 tensor core Blackwell cần padding mà CTranslate2 bản cũ chưa có.
- **Workaround xác nhận: đặt `compute_type="float16"` → chạy bình thường.** (Tránh `int8` và `int8_float16` trên GPU này.)
- Yêu cầu: **CTranslate2 build CUDA 12 + cuDNN 9 + cuBLAS**. Bản CTranslate2 mới chỉ hỗ trợ CUDA 12/cuDNN 9. Blackwell cần CUDA 12.8-class libs.
- Có báo cáo 5070 Ti chậm hơn ~10% so với 4070 Ti Super trên faster-whisper (issue #1287, chưa có root cause chốt) — vẫn nhanh gấp bội CPU, chỉ là chưa khai thác hết Blackwell. Không phải blocker.

### 3. Phân phối thư viện CUDA — TẢI ON-DEMAND, không bundle
Sidecar hiện là PyInstaller **CPU-only**. Bật GPU cần cuBLAS + cuDNN 9. **Quan trọng: đây là thư viện, KHÔNG phải model — và tải on-demand được y như model, KHÔNG cần nhúng vào installer.**

Kích thước thật (verified PyPI, Windows win_amd64):
- `nvidia-cudnn-cu12` (cuDNN **9.24.0**): **737 MB**
- `nvidia-cublas-cu12` (12.9): **553 MB**
- **Tổng ~1.3 GB**, tải 1 lần vào app-data.

**Điểm then chốt (verified): faster-whisper/CTranslate2 KHÔNG phụ thuộc PyTorch** → tránh được cục torch nhiều GB. Chỉ cần cuBLAS + cuDNN.

Hướng khuyến nghị **(a) On-demand download** (đúng pattern model đang có):
- Installer giữ nguyên gọn nhẹ (CPU-only).
- Máy không NVIDIA (macOS, laptop khác) → không tải gì thêm, chạy CPU như cũ.
- Máy có GPU NVIDIA, lần đầu bật GPU → tải ~1.3GB CUDA runtime (2 wheel PyPI là .zip → giải nén lấy `.dll` → thêm vào DLL search path). **Không cần tự host** (PyPI làm CDN).
- Tối ưu về sau: cuDNN 737MB chủ yếu là `cudnn_engines_precompiled` (~600MB) — có thể chỉ lấy subset DLL thực dùng để giảm còn vài trăm MB. Làm sau (KISS).

Hướng thay thế:
- **(b) 2 bản installer** (CPU nhỏ / GPU lớn) — user chọn; đơn giản code nhưng 2 artifact + bắt user chọn.
- **(c) Purfview `whisper-standalone-win`** — libs đóng sẵn, vẫn phải host/tải.
- **(d) Bundle vào installer** — phình ~2–3GB. ❌ User đã loại (không ổn).
- Bắt buộc giữ **fallback CPU** cho máy không có NVIDIA.

### 4. "Model xịn hơn" — có gì đáng đổi?

| Model | Điểm mạnh | Runtime | Đáng làm? |
|---|---|---|---|
| **PhoWhisper-large** (đã có) | **CER tiếng Việt thuần tốt nhất** nhóm large | CTranslate2 (đang dùng) | ✅ Ưu tiên 1 — chỉ cần bật GPU |
| **Whisper large-v3-turbo** | Code-switch Việt–Anh tốt hơn PhoWhisper; **~8× nhanh**, 809M params, sai lệch 1–2% so large-v3 | CTranslate2 (thêm dễ) | ✅ Ưu tiên 2 — thêm option cho nội dung lẫn tiếng Anh |
| **Whisper large-v3** | Code-switch Việt–Anh tốt nhất | CTranslate2 | 🟡 Có nếu cần độ chính xác code-switch tối đa |
| **nvidia/parakeet-ctc-0.6b-Vietnamese** | Train 2000h+ tiếng Việt, throughput cao | **NeMo (KHÁC runtime)** | 🚫 YAGNI — đổi runtime, đại phẫu |
| NVIDIA Canary-1B-v2 / Parakeet-TDT-0.6b-v3 | Top leaderboard đa ngữ | NeMo | 🚫 **Chỉ 25 tiếng châu Âu, KHÔNG có tiếng Việt** |

---

## Recommendation (ưu tiên theo ROI)

**Ưu tiên 1 — Bật CUDA cho faster-whisper, mặc định PhoWhisper-large float16 (khi có GPU NVIDIA):**
1. Build/ship CTranslate2 CUDA (cuDNN 9 + cuBLAS) hoặc dùng Purfview standalone libs.
2. Runtime detect GPU NVIDIA → `device="cuda"`, **`compute_type="float16"`** (KHÔNG int8 — bug Blackwell). Không có GPU → fallback CPU int8 như hiện tại.
3. Khi có GPU, nâng model mặc định medium → **large** (giờ đã đủ nhanh).
4. → Vừa "model xịn hơn" (large, CER Việt tốt nhất) vừa nhanh hơn nhiều. Không cần tải model mới.

**Ưu tiên 2 — Thêm option `whisper-large-v3-turbo`** cho nội dung Việt–Anh lẫn lộn (kỹ thuật, DevOps). Cùng runtime CTranslate2, tải nhẹ, chạy GPU rất nhanh.

**Không làm (giai đoạn này):** chuyển sang NVIDIA Parakeet/Canary (NeMo) — đổi toàn bộ runtime, vi phạm KISS/YAGNI trừ khi PhoWhisper-large không đạt yêu cầu chính xác.

---

## Resources & References

- [faster-whisper crashes on RTX 50-series (cuBLAS NOT_SUPPORTED) — SubtitleEdit #10180](https://github.com/SubtitleEdit/subtitleedit/issues/10180)
- [WhisperX không chạy Blackwell sm_120 — whisperX #1211](https://github.com/m-bain/whisperX/issues/1211)
- [faster-whisper chậm trên 5070 Ti hơn 4070 Ti Super — #1287](https://github.com/SYSTRAN/faster-whisper/issues/1287)
- [CUDA compatibility với CTranslate2 — faster-whisper #1086](https://github.com/SYSTRAN/faster-whisper/issues/1086)
- [SYSTRAN/faster-whisper (README, yêu cầu CUDA12/cuDNN9)](https://github.com/SYSTRAN/faster-whisper)
- [faster-whisper GPU cloud production guide 2026 — Spheron](https://www.spheron.network/blog/faster-whisper-gpu-cloud-production-deployment-guide/)
- [Whisper Large V3 Turbo: nhanh 6–8× — Medium](https://medium.com/@bnjmn_marie/whisper-large-v3-turbo-as-good-as-large-v2-but-6x-faster-97f0803fa933)
- [nvidia/parakeet-ctc-0.6b-Vietnamese — Hugging Face](https://huggingface.co/nvidia/parakeet-ctc-0.6b-Vietnamese)
- [Canary-1B-v2 & Parakeet-TDT-0.6B-v3 (arXiv 2509.14128) — chỉ 25 tiếng châu Âu](https://arxiv.org/html/2509.14128v2)
- [Best open-source STT 2026 benchmarks — Northflank](https://northflank.com/blog/best-open-source-speech-to-text-stt-model-in-2026-benchmarks)

## Unresolved Questions

1. ~~Bundle vs nhẹ hơn~~ → **ĐÃ CHỐT: tải on-demand ~1.3GB CUDA libs (chỉ máy NVIDIA), installer giữ gọn.** Còn lại: xác nhận nguồn tải = PyPI trực tiếp hay mirror iNET.
2. Có cần code-switch Việt–Anh nhiều không? Nếu có → thêm large-v3-turbo lên ưu tiên cao hơn.
3. Root cause 5070 Ti chậm hơn 4070 Ti Super (issue #1287) chưa chốt — cần benchmark thực tế trên máy sau khi bật GPU để xác nhận tốc độ thực.
