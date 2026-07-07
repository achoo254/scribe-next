---
phase: 1
title: "Benchmark ASR Gate"
status: done
effort: "S"
priority: P1
dependencies: []
---

# Phase 1: Benchmark ASR Gate

## Overview

Đo RTF/RAM PhoWhisper small (stream) + medium (batch) **trên chính box .65** (4 vCPU Broadwell — yếu hơn máy dev đã spike) trước khi commit toàn bộ plan. Thay ước lượng bằng số thật. Đây là GO/NO-GO gate.

## Requirements

- Functional: số RTF + RAM peak cho (a) batch medium @2 threads, (b) stream small vad-cut @2 threads ×1–2 track.
- Non-functional: KHÔNG làm degrade ~15 service đang chạy trong lúc đo (theo dõi load/RAM trước–trong–sau).

## Architecture

Tái dùng harness spike Phase 1 plan cũ: `sidecar/transcriber/stream_asr_spike.py` (stream — CÓ `--cpu-threads`) + `sidecar/transcriber/main.py` (batch protocol JSON stdio). Chạy trực tiếp trên .65 qua SSH, model tải về `/opt/onescribe/models/`.

> **[RT-1] Cảnh báo đo lường — GATE ĐÚNG CẤU HÌNH:** `main.py:67` hardcode `cpu_threads=os.cpu_count()` (=4 trên box) và **không có argparse** (`main.py:142` chỉ check `--stream`). Nếu đo batch bằng main.py hiện tại → chạy **4 threads** trong khi production bị `CPUQuota=200%` (2 core) → RTF đo LẠC QUAN, GO/NO-GO SAI. `stream_server.py:56`/`stream_session.py:29` cũng vậy. **Bắt buộc trước khi đo:** hoặc (a) đo dưới `taskset -c 0,1` + `CPUQuota` cgroup mô phỏng production, hoặc (b) làm luôn patch nhỏ `cpu_threads` từ env (đây là việc Phase 3 cần → làm sớm ở đây). Ghi rõ phương án đã dùng trong report.

## Related Code Files

- Đọc/tái dùng: `sidecar/transcriber/stream_asr_spike.py` (đã có `--cpu-threads`), `main.py` (batch — **không có** flag threads), `model_paths.py` (small: `quocphu/PhoWhisper-ct2-FasterWhisper`, medium cùng repo), `requirements.txt`.
- Có thể sửa nhỏ (nếu chọn phương án b): thêm đọc `ONESCRIBE_CPU_THREADS` env vào `main.py:65-68` — patch tối thiểu, không đổi protocol.
- Tạo: `plans/260707-1325-onescribe-web-pivot/reports/phase-01-benchmark-box65-results.md` — số liệu + kết luận GO/NO-GO.
- Tham khảo mẫu đo + bài học: `plans/260626-1418-realtime-meeting-live-transcribe/reports/spike-streaming-asr-results.md` (LocalAgreement NO-GO, vad-cut là chuẩn; `latency_valid` flag).

## Implementation Steps

1. SSH .65: kiểm `python3 --version` (cần ≥3.9 cho faster-whisper; sidecar pin theo 3.14 dev — nếu box chỉ có 3.9–3.11 vẫn OK, ghi version thật vào report). Kiểm dung lượng `/opt` (cần ~2.5GB model).
2. Đưa `sidecar/transcriber/` lên `/opt/onescribe/bench/` (rsync). Cài deps: `pip install --target` + `PYTHONPATH` (pattern đã dùng trên mac, tránh venv). KHÔNG cài silero/torch.
3. Tải model small + medium CT2 về `/opt/onescribe/models/` (dùng `ensure_model` của main.py hoặc `huggingface_hub`).
4. Chuẩn bị mẫu: dùng lại mẫu TTS đa giọng từ spike cũ + (nếu có) 1 file họp tiếng Việt thật ≥10'. Upload lên box.
5. **Đo batch medium ĐÚNG 2 core**: `taskset -c 0,1 nice -n 10` chạy main.py (hoặc env `ONESCRIBE_CPU_THREADS=2` nếu đã patch); đo wall-time → RTF; RAM peak (`/usr/bin/time -v`). Lặp file 10'+ và 1 file 60' thật (RTF có thể khác theo độ dài).
6. **Đo stream small ĐÚNG 2 core**: `stream_asr_spike.py --vad-cut --cpu-threads 2 --realtime`, 1 track rồi 2 track; ghi `rtf_mean`, `aggregate_rtf_wall`, `latency_valid`.
7. **[RT-2] Đo RAM PEAK batch+live ĐỒNG THỜI** (kịch bản two-pass thật, không đo riêng lẻ): chạy stream small + batch medium cùng lúc, ghi RSS tổng đỉnh. Đây là số quyết định `MemoryMax`/`MemorySwapMax` và có cần arbiter cấm chạy đồng thời hay không. Spike cũ đo peak ~2.9–3.2GB → kỳ vọng OOM nếu MemoryMax=2500M.
8. Suốt lúc đo: terminal thứ 2 theo dõi `vmstat 5` + `free -h` + swap delta + spot-check response service chính (nginx vhost đang chạy trên box). Ghi degrade nếu có.
9. Viết report + kết luận GO/NO-GO. NO-GO → phương án: resize VM (khuyến nghị mạnh) / box khác / hạ live 1 stream / bỏ meeting khỏi MVP / batch-only.

## Success Criteria

> **KẾT QUẢ ĐO 2026-07-07** — chi tiết: [reports/phase-01-benchmark-box65-results.md](./reports/phase-01-benchmark-box65-results.md).

- [x] Batch medium RTF ≤ ~3.0 đo đúng 2 core — **ĐẠT: 0.485 (bs=8) / 0.542 (bs=1)**, small 0.222. ✅ GO batch.
- [x] Stream small `rtf_mean`/call < 1.0 @2 threads — **KHÔNG ĐẠT: 4.71 (max_utt=5) → 1.71 (max_utt=15)**; `rtf_wall` tốt nhất 1.49; `latency_valid: false` mọi config. ❌ **NO-GO live.** Nguyên nhân CPU (Broadwell không-VNNI + encoder 30s/utterance), KHÔNG phải swap (đối chứng clean-swap xác nhận).
- [x] RAM peak → chốt `MemoryMax` + arbiter — **ĐO: batch medium bs=1 = 2.19GB, bs=8 = 4.27GB (gấp 2.5× giả định).** Production dùng **6.76GB resident, chỉ 920MB free** → 8GB không đủ deploy sạch; `MemorySwapMax=0` bất khả thi trên 8GB.
- [x] Service khác không degrade — **CÓ degrade khi batch bs=8**: swap đầy 2GB, avail↓280M, pages prod bị evict (hồi được). bs=1 không swap thêm.
- [x] Report có số + GO/NO-GO + spec MemoryMax + resize — **DONE.**

## KẾT QUẢ GATE

- **Batch (Phase 2): GO** (batch_size=1, cần RAM headroom).
- **Live (Phase 3) + Meeting (Phase 4): NO-GO trên .65** — giới hạn CPU streaming, resize KHÔNG cứu.
- **8GB không đủ** → resize 12–16GB (batch sạch) hoặc box CPU mới (nếu giữ live).
- ⇒ **STOP: chờ user quyết scope/infra** (xem Unresolved trong report).

## Risk Assessment

- Python trên box quá cũ (<3.9) → dùng pyenv/conda-forge standalone build; ghi vào report vì ảnh hưởng deploy Phase 2.
- Broadwell không có AVX-512/VNNI → int8 chậm hơn kỳ vọng — chính là lý do phải đo; số xấu = dữ liệu NO-GO hợp lệ, không phải thất bại.
- Đo lúc box đang bận đột biến → chạy 2 lần ở 2 thời điểm khác nhau, lấy số xấu hơn.
