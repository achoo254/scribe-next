# Research: Model ASR tiếng Việt nhanh trên CPU (ưu tiên tốc độ)

> Thực hiện: 2026-06-15 · Bối cảnh: Scribe Mode C đang dùng `kiendt/PhoWhisper-large-ct2` (int8 CPU) — chất lượng tốt nhưng chậm trên CPU (large ~1.55B params). User muốn nhanh hơn, chấp nhận giảm chút chất lượng.

## Executive Summary

**KHÔNG cần đổi kiến trúc.** Stack hiện tại (faster-whisper + CTranslate2 + PhoWhisper) đã là lựa chọn CPU tối ưu — research xác nhận faster-whisper int8 + batching **nhanh hơn cả whisper.cpp** trên CPU (nhờ Intel oneMKL/oneDNN + AVX512). Chỉ cần 2 đòn bẩy, **giữ nguyên code Rust/sidecar protocol**:

1. **Đổi model nhỏ hơn** — `PhoWhisper-small` (~244M, nhanh ~6×) hoặc `PhoWhisper-medium` (~769M, nhanh ~2×) thay cho large. Cùng họ VinAI, cùng format CT2 int8, chỉ đổi `MODEL_REPO`.
2. **Bật batched inference** — `BatchedInferencePipeline` của faster-whisper cho thêm ~2× trên CPU. Code hiện dùng `model.transcribe()` tuần tự (chưa batch).

Kết hợp 2 đòn bẩy: 11.6 phút audio có thể từ **~10–20 phút (large hiện tại)** xuống **~1–3 phút**.

## Key Findings

### 1. Họ PhoWhisper (VinAI) — 5 size, cùng fine-tune 844h tiếng Việt

| Model | Params (~) | Tốc độ vs large | Chất lượng tiếng Việt |
|-------|-----------|-----------------|------------------------|
| tiny | 39M | ~40× | Yếu (ngang wav2vec2-base) |
| base | 74M | ~20× | Khá (ngang wav2vec2-base-vi-vlsp) |
| **small** | **244M** | **~6×** | **Tốt — vượt mọi baseline wav2vec2** |
| **medium** | **769M** | **~2×** | **Rất tốt, sát large** |
| large (hiện tại) | 1550M | 1× | SOTA |

Theo paper PhoWhisper (ICLR 2024 Tiny Paper): small/medium/large đều **vượt** các baseline wav2vec2; tiny/base chỉ ngang wav2vec2. → **small là ngưỡng "nhanh mà vẫn chất lượng tốt"**.

### 2. CT2 availability (không cần tự convert nếu dùng sẵn)

- `quocphu/PhoWhisper-ct2-FasterWhisper` — bản CT2 cho faster-whisper (HF).
- `dongxiat/ggml-PhoWhisper-medium` — bản GGML cho whisper.cpp (KHÔNG cần, ta giữ faster-whisper).
- Hoặc tự convert chuẩn: `ct2-transformers-converter --model vinai/PhoWhisper-small --quantization int8 --output_dir ...` (vài phút, 1 lần).

### 3. CPU benchmark (anchor số liệu)

faster-whisper **small int8**, 13 phút audio, i7-12700K, 8 threads ([saytowords](https://www.saytowords.com/blogs/Whisper-V3-Benchmarks/)):
- int8 beam5: **1m42s**
- int8 + `batch_size=8`: **51s** ← batching cho ~2×

→ small int8 batched ≈ **15× realtime**. Large nặng gấp ~6× → khớp với trải nghiệm "large chậm" của user.

### 4. whisper.cpp vs faster-whisper trên CPU → giữ faster-whisper

- whisper.cpp small FP32: 2m05s vs faster-whisper FP32 2m37s (cpp nhỉnh chút ở FP32).
- NHƯNG faster-whisper int8+batch = **51s** → nhanh nhất. Lý do: CTranslate2 dùng Intel oneMKL + AVX512 cho matmul.
- Kết luận: **không đổi sang whisper.cpp** — vô ích + tốn công viết lại sidecar C++. ([promptquorum](https://www.promptquorum.com/power-local-llm/local-whisper-stt-comparison-2026), [builderai](https://builderai.tools/blog/whisper-cpp-vs-faster-whisper-speed-and-accuracy))

### 5. Phương án ngoài Whisper (KHÔNG khuyến nghị)

- **wav2vec2 tiếng Việt** (`nguyenvulebinh/wav2vec2-base-vietnamese-250h`, WER 6.15% VIVOS): nhanh nhưng **không có dấu câu, không timestamp sẵn cho .srt** (phải tự align CTC), và phải viết lại pipeline → phá KISS.
- **Vosk**: nhẹ nhất, streaming CPU, nhưng độ chính xác thấp hơn transformer + model tiếng Việt yếu.
→ Cả hai đòi đổi kiến trúc + mất tính năng .srt. Bỏ qua.

## Recommendation (KISS, theo đúng stack hiện có)

**Ưu tiên tốc độ như user yêu cầu:**

- **Mặc định: `PhoWhisper-small` (int8) + `BatchedInferencePipeline`.** Nhanh ~6× model + ~2× batching, chất lượng vẫn "tốt" (vượt wav2vec2). Hợp nhất với "tốc độ ưu tiên, chất lượng giảm chút OK".
- **Nếu thấy small sai nhiều thuật ngữ kỹ thuật** (audio DevOps: VictoriaLogs/Grafana...) → nâng lên `medium` (vẫn nhanh ~2× large).
- **Lý tưởng (đúng kế hoạch "v2" trong plan): model picker** small/medium/large để user tự chọn theo từng file. Nhưng tốn thêm UI + quản lý nhiều model — để sau.

### Thay đổi code cần thiết (nhỏ, trong sidecar Python — KHÔNG đụng Rust)

1. `main.py`: đổi `MODEL_REPO = "vinai/PhoWhisper-small"` (hoặc bản CT2 small) + convert int8 nếu cần.
2. `main.py`: bọc `BatchedInferencePipeline(model)` + `batched_model.transcribe(file, batch_size=8, beam_size=1, language="vi", vad_filter=True, ...)`.
3. Lưu ý: model size đổi → đường dẫn lưu model + dung lượng tải lần đầu giảm mạnh (small ~0.5GB vs large ~3GB) → bonus: tải nhanh hơn nhiều.

### Đánh đổi rõ ràng

| | large (hiện tại) | small + batch (đề xuất) |
|---|---|---|
| Tốc độ 11.6′ audio | ~10–20′ | ~1′ |
| Chất lượng | SOTA | Tốt (sai hơn ở thuật ngữ/tên riêng) |
| Tải model lần đầu | ~3GB | ~0.5GB |
| Đổi kiến trúc | — | Không (chỉ sửa sidecar) |

## Unresolved Questions

1. Bản CT2 `quocphu/PhoWhisper-ct2-FasterWhisper` có sẵn size small/medium hay chỉ large? → nếu thiếu, tự convert từ `vinai/PhoWhisper-small` bằng `ct2-transformers-converter` (1 lệnh).
2. Chốt 1 model mặc định (small — speed-first) hay làm picker đa-model (đúng "v2")? Cần user quyết.
3. Có cần giữ glossary `initial_prompt` (DevOps terms) khi đổi small không? → nên giữ, giúp small bớt sai tên riêng.

## Sources

- [PhoWhisper paper (arXiv 2406.02555)](https://arxiv.org/abs/2406.02555) · [VinAI](https://research.vinai.io/publication/phowhisper-automatic-speech-recognition-for-vietnamese/) · [vinai/PhoWhisper-small/medium/large (HF)](https://huggingface.co/vinai/PhoWhisper-medium)
- [quocphu/PhoWhisper-ct2-FasterWhisper (HF)](https://huggingface.co/quocphu/PhoWhisper-ct2-FasterWhisper) · [dongxiat/ggml-PhoWhisper-medium (HF)](https://huggingface.co/dongxiat/ggml-PhoWhisper-medium)
- [Whisper V3 CPU benchmarks (saytowords)](https://www.saytowords.com/blogs/Whisper-V3-Benchmarks/) · [whisper.cpp vs faster-whisper 2026 (promptquorum)](https://www.promptquorum.com/power-local-llm/local-whisper-stt-comparison-2026)
- [faster-whisper (PyPI)](https://pypi.org/project/faster-whisper/) · [wav2vec2-base-vietnamese-250h](https://huggingface.co/nguyenvulebinh/wav2vec2-base-vietnamese-250h)
