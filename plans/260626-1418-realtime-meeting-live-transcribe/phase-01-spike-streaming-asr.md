---
phase: 1
title: "Spike Streaming ASR (RISK GATE)"
status: in-progress
priority: P1
effort: "3-5d"
dependencies: []
---

# Phase 1: Spike Streaming ASR — RISK GATE

> **Tiến độ (2026-06-26):** Harness spike HOÀN THÀNH + verified (`stream_asr_spike.py` + `spike_lib/`),
> đã qua code-review (3 lỗi đo lường High đã sửa). **G3 (RAM) + G4 (VAD) ĐẠT bằng số thật**
> (small+medium peak ~2.1GB, webrtcvad build OK Py3.14). **CÒN:** G1/G2 (RTF/latency cần mẫu
> speech tiếng Việt thật) + G5 (chất lượng định tính) + G6 (mini-spike chân audio WebView2 — cần app).
> Chi tiết & cách chạy: [`reports/spike-streaming-asr-results.md`](./reports/spike-streaming-asr-results.md).
> **Gate CHƯA đóng** → chưa cấp ngân sách Phase 2+.

## Overview

Chứng minh **máy host transcribe được nhiều luồng audio đồng thời theo kiểu streaming** (latency caption ~2–4s) trên cấu hình máy yếu nhất của khách hàng. Đây là **gate sống/chết**: nếu không đạt, toàn bộ mô hình "host-transcribes-all" phải xem lại trước khi đổ công vào media/server/web.

Đây là **spike** — code throwaway/đo đạc, KHÔNG phải code production. Mục tiêu là con số, không phải tính năng hoàn chỉnh.

## Requirements

- **Functional:** sidecar nhận audio dạng **chunk PCM streaming** (không phải file hoàn chỉnh) → phát caption **partial** (tạm, cập nhật liên tục) + **final** (chốt câu) kèm timestamp, theo từng nguồn (track id).
- **Non-functional:**
  - Latency từ lúc nói xong 1 câu → caption final xuất hiện: **≤ ~4s** (mục tiêu ~2–3s).
  - Real-time factor mỗi luồng đang nói **< 1.0** trên máy host tối thiểu.
  - Chịu được **≥ 2 người nói chồng** mà không tụt latency thảm họa (nhờ VAD active-speaker gating).

<!-- Updated: Validation Session 1 — chốt spec host + quy mô -->
- **Máy host tối thiểu (đã chốt validation):** **laptop văn phòng — i5 ~4 nhân, 8–16GB RAM, KHÔNG GPU**. Đây là cấu hình "xấu nhất thực tế"; đạt ở đây = đạt khắp nơi. Đo song song trên máy dev để có baseline.
- **Quy mô MVP (đã chốt):** **≤ 8 người/phòng** → tối đa ~8 track, nhưng ASR đồng thời chỉ chạy cho track đang nói (active-speaker). Trần ASR đồng thời cần đo: kỳ vọng 1–2 đủ cho hội thoại tự nhiên.

## Architecture

```
[Giả lập N nguồn audio PCM 16kHz mono int16 LE]   (spike: file WAV nhiều người / mic + loopback)
        │ chunk ~0.5-1s
        ▼
[VAD per-track: webrtcvad HOẶC faster-whisper vad_filter] ──► chỉ track ĐANG nói đẩy vào ASR (gating)
        │                                                       (KHÔNG silero-vad — kéo PyTorch ~2GB)
        ▼
[LocalAgreement: whisper_online.py copy-vào-repo HOẶC tự implement] ── backend = WhisperModel PhoWhisper-small CPU int8 (warm, KHÔNG BatchedInferencePipeline)
        │  partial + final
        ▼
[stdout JSON: {type:"caption", track, kind:"partial|final", text, t0, t1}]
```

- **Streaming policy:** Whisper huấn luyện cửa sổ 30s, không streaming gốc → dùng **LocalAgreement** (copy `whisper_online.py` của UFAL vào repo — KHÔNG có trên PyPI) hoặc tự cắt câu bằng **VAD** (webrtcvad/built-in) rồi transcribe từng utterance khi VAD báo hết câu. So sánh 2 cách, chọn cái đạt latency. Xem mục **Red Team Hardening** bên dưới.
- **Model:** PhoWhisper **small** ct2 int8 — nạp bằng **`WhisperModel` trần** (KHÔNG `BatchedInferencePipeline` như `main.py:80`) cho live; KHÔNG dùng medium/large cho live (chậm).
- **Gating:** chỉ chạy ASR cho track có speech → tải hiệu dụng ~số-người-đang-nói, không phải tổng số người.

## Related Code Files

- Create: `sidecar/transcriber/stream_asr_spike.py` — script spike độc lập (LocalAgreement qua `whisper_online.py` copy + VAD webrtcvad/built-in), đọc PCM từ stdin/nhiều WAV, in caption + đo latency/RTF.
- Read for context: `sidecar/transcriber/main.py` (protocol JSON-stdio, cách nạp model warm, `get_model`), `src-tauri/src/transcribe.rs` (vòng đời sidecar).
- Create: `plans/260626-1418-realtime-meeting-live-transcribe/reports/spike-streaming-asr-results.md` — báo cáo số đo (latency, RTF, CPU, RAM theo số track).

## Implementation Steps

1. Dựng venv spike (tái dùng `sidecar/transcriber/.venv`): copy `whisper_online.py` (UFAL, không có trên PyPI) HOẶC tự implement LocalAgreement; thêm `webrtcvad` (KHÔNG silero-vad/PyTorch), `soundfile`/`numpy`. Pin version + hash.
2. Chuẩn bị mẫu test tiếng Việt: 1 file 1 người nói liên tục; 1 kịch bản 2–3 người nói chồng (ghép từ mic/loopback hoặc nhiều WAV); đa dạng tốc độ nói.
3. Viết `stream_asr_spike.py`: nạp PhoWhisper-small warm (`WhisperModel` int8), feed chunk PCM ~0.5–1s, VAD gate (webrtcvad/built-in), LocalAgreement → in `{caption, track, kind, text, t0, t1}` + log mốc thời gian để tính latency.
4. Đo trên **2 cấu hình**: (a) máy dev hiện tại; (b) máy yếu nhất giả định (giới hạn cpu_threads để mô phỏng). Ghi RTF, latency p50/p95, CPU%, RAM theo 1/2/3 track nói đồng thời.
5. Thử biến thể: chunk size (0.5s vs 1s), beam_size=1, batch on/off, VAD aggressiveness → tìm điểm latency/độ chính xác chấp nhận được.
6. **Xác định spec host tối thiểu** + ngưỡng degradation (quá tải → bỏ qua track nào / hạ tần suất). Viết báo cáo kết luận GO/NO-GO.

## Success Criteria

- [ ] Caption final xuất hiện ≤ 4s sau khi nói xong câu, ở kịch bản 2 người nói chồng, trên máy host tối thiểu.
- [ ] RTF mỗi luồng đang nói < 1.0 trên máy tối thiểu (có headroom cho 2 luồng đồng thời).
- [ ] Chất lượng caption small "đủ đọc" tiếng Việt kỹ thuật (không cần chuẩn như medium — đó là việc của batch sau họp).
- [ ] Báo cáo `spike-streaming-asr-results.md` có số liệu cụ thể + **kết luận GO/NO-GO** + spec host tối thiểu.
- [ ] (Nếu NO-GO) ghi rõ phương án thay thế: hạ kỳ vọng (chỉ 1 người host nói), hoặc server-side ASR có GPU, hoặc giảm số track transcribe đồng thời.

## Risk Assessment

- **RTF ≥ 1 trên CPU yếu** → caption trễ dồn vô hạn. Giảm thiểu: model small, beam=1, VAD gate mạnh, giới hạn số track ASR đồng thời (vd tối đa 2 active speaker).
- **whisper_streaming chưa từng chạy với PhoWhisper** → có thể cần chỉnh backend faster-whisper + initial_prompt (glossary đã có trong `main.py`). Giảm thiểu: fallback sang VAD-cut-utterance đơn giản nếu LocalAgreement khó tích hợp.
- **Audio chồng tiếng (cross-talk)** làm ASR sai — chấp nhận ở MVP (track riêng từng người giảm bớt vì mỗi track 1 giọng).
- **Đây là spike** → không hoàn thiện, không tối ưu sớm (KISS). Chỉ đo để quyết định.

## Red Team Hardening (Session 1)

Các fix BẮT BUỘC đưa vào spike (red-team 4/4 reviewer hội tụ — không bỏ qua):

- **[C1] Dùng `WhisperModel` trần (KHÔNG `BatchedInferencePipeline`) cho streaming.** `main.py:80` hiện wrap BatchedInferencePipeline (chỉ hợp batch file). whisper_streaming/LocalAgreement cần `WhisperModel` trực tiếp. Đo RTF với CHÍNH `device="cpu", compute_type="int8"` — KHÔNG qua default cuda/float16 của whisper_streaming (nếu không RTF đo sai, GO/NO-GO sai). **Đo RAM khi giữ ĐỒNG THỜI** model small (streaming) + medium (batch Mode C) → kiểm máy 8GB có OOM không.
- **[C3] `whisper_streaming` KHÔNG có trên PyPI.** Copy thẳng `whisper_online.py` (single file) từ UFAL repo vào `sidecar/transcriber/`, HOẶC tự implement LocalAgreement (đơn giản). Update `transcriber.spec`/`build_pyinstaller.ps1` để `--add-data` file này. KHÔNG `pip install whisper-streaming`.
- **[C4] KHÔNG dùng silero-vad** (kéo PyTorch ~2GB, phình binary + xung đột ctranslate2). Dùng **`webrtcvad`** (pure C, ~50KB) hoặc **`vad_filter=True`** sẵn có của faster-whisper (`main.py:95`). Pin version + hash trong requirements.
- **[C5] MỞ RỘNG spike để test CHÂN audio thật trong WebView2** (đây là unknown khó nhất, không để tới Phase 3): từ `getUserMedia`/remote track → `AudioWorklet` (kiểm `MediaStreamTrackProcessor` có bật trong WebView2 không; `SharedArrayBuffer` cần COOP/COEP mà `tauri://` set không được) → `invoke()` Rust → sidecar. Đo throughput + latency round-trip thật. Nếu chân này không đạt, latency ASR (Python) không cứu được.
- **[stream_pcm schema]** Chốt rõ: PCM **16kHz mono int16 little-endian**, chunk ~0.5–1s, framing (length-prefix), truyền binary (Tauri ArrayBuffer) — KHÔNG JSON-array (32KB → ~96KB JSON/chunk/track × 8 track = bão hòa IPC).
- **[C2 preview]** Thiết kế streaming chạy **tách process/thread riêng** (không nhét vào vòng `for line in sys.stdin` đơn luồng của `main.py` — sẽ bị batch Mode C chặn). Validate mô hình process tách trong spike.

### Tiêu chí GO bổ sung
- [ ] RTF < 1 đo bằng WhisperModel CPU int8 (không phải BatchedInferencePipeline/cuda).
- [ ] RAM small+medium đồng thời KHÔNG OOM trên 8GB.
- [ ] Chân AudioWorklet→Rust→sidecar trong WebView2 chạy được + latency đo được.
- [ ] VAD chọn xong (webrtcvad / faster-whisper built-in), không kéo PyTorch.
