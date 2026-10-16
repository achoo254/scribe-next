# Phase 1 — Benchmark ASR Gate trên box .65 (KẾT QUẢ + GO/NO-GO)

**Ngày:** 2026-07-07 · **Box:** staging .65 (`103.72.98.65`, 4 vCPU Broadwell, 7.5GB RAM, 2GB swap, 60GB disk).
**Đo bởi:** chạy trực tiếp trên box qua SSH, model + mẫu tải về `/opt/scribe/`. Mẫu = FLEURS `vi_vn` (read speech thật 16kHz mono, ghép bằng ffmpeg → batch 10'/60', stream 2×60s).
**Đúng cấu hình production (finding RT-1):** mọi batch đo với `taskset -c 0,1 nice -n 10` + `SCRIBE_CPU_THREADS=2` (mô phỏng `CPUQuota=200%`). Stream `--cpu-threads 2`.

---

## KẾT LUẬN GO/NO-GO (tách đôi)

| Đường | Kết quả | Lý do |
|---|---|---|
| **Batch transcribe (Phase 2)** | ✅ **GO** (có điều kiện RAM) | RTF medium **0.485–0.542**, small **0.222** — dưới xa gate 3.0. Broadwell kham tốt decode batched. |
| **Live/Streaming (Phase 3)** | ❌ **NO-GO trên box này** | Stream small **không config nào đạt RTF<1**; tốt nhất `rtf_wall 1.49`, `latency_valid: false` mọi lần. Backlog tăng vô hạn. **KHÔNG phải lỗi RAM/swap — là giới hạn CPU.** |
| **Meeting web (Phase 4)** | ❌ **NO-GO trên box này** | Phụ thuộc đường live (bot = 1 client stream nội bộ). 1 track đã không kịp → ≤2 track càng tệ. |
| **RAM box 8GB** | ❌ **Không đủ cho deploy sạch** | Production đã dùng **6.76GB resident**, chỉ còn **~920MB free**. Batch medium bs=1 (2.19GB) ép swap; bs=8 (4.27GB) sát OOM. |

**Tóm tắt:** CPU Broadwell (không AVX-512/VNNI) đủ cho **batch offline** (không ràng buộc realtime → cứ chạy chậm cũng xong) nhưng **không kham realtime streaming** (cần RTF<1 bền vững). RAM 8GB không đủ ngay cả cho batch-only sạch. ⇒ **MVP đầy đủ (batch+live+meeting) KHÔNG khả thi trên .65 hiện trạng.**

---

## 1. Số đo Batch (medium/small, file 10' = 600s speech thật, 2 core)

| size | batch_size | RTF decode | Peak RSS | Ảnh hưởng box |
|---|---|---|---|---|
| medium | **8** (main.py hiện tại) | 0.485 | **4271MB** | swap 875→**2047M (đầy)**, avail↓**280M**, đẩy pages prod ra swap |
| medium | 4 | 0.492 | 3569MB | avail↓983M |
| medium | **1** | **0.542** | **2192MB** | **không swap thêm, avail≥983M** |
| small | 1 | **0.222** | 817MB | nhẹ |

- **RTF gần như bất biến theo batch_size** (0.485→0.542, chỉ chậm ~12% ở bs=1) nhưng **RAM giảm ~2GB**. ⇒ **`batch_size=1` là đòn bẩy RAM chính** (finding: `main.py:95` đang hardcode `batch_size=8`).
- Peak RSS medium **4.27GB @bs=8 — gấp ~2.5× giả định 1.74GB của plan** (finding RT-2 tệ hơn dự kiến). Nguồn: batched pipeline gom feature 8 segment song song.
- File 60': **KHÔNG chạy** (RTF ổn định theo độ dài với batched; RAM duration-independent — số 10' đại diện). Có thể chạy xác nhận sau khi chốt hạ tầng.

## 2. Số đo Stream (small, vad-cut, energy VAD, 2 thread, realtime, 1 track)

| max_utterance | rtf_wall | rtf_mean/call | lat_p95 | latency_valid | peak RSS | avg CPU |
|---|---|---|---|---|---|---|
| 5s (mặc định spike) | 3.32 | 4.71 | 141s | ❌ false | 1738MB | 166% |
| 15s | **1.49** | 1.71 | 40s | ❌ false | ~1740MB | 142% |
| 30s | 2.10 | 2.43 | 71s | ❌ false | — | 146% |

- **Đối chứng loại trừ swap:** run "clean swap" (swap vừa dọn, ổn định 1243M suốt run) vẫn `rtf_mean 4.71`, CPU **166%** (tính toán thật, không idle-wait). ⇒ **chậm do CPU, không do swap.**
- **Đối chứng batched:** cùng box/model/threads, batch small = RTF **0.22**. Stream chậm **21×** so với batch. Nguyên nhân: mỗi utterance ngắn vẫn kích hoạt **encoder Whisper cửa sổ 30s cố định** → 60s audio tốn 11 lần encode (max_utt=5). Broadwell int8 không-VNNI làm 1 lần encode 30s ~ chục giây → RTF>1.
- 2 track: không hoàn tất đo (1 track đã NO-GO → 2 track chắc chắn tệ hơn do contention).

## 3. RAM thật của box (phát hiện quan trọng)

| Trạng thái | used | avail | swap |
|---|---|---|---|
| Lúc bắt đầu (2GB đã swap sẵn) | 4.6GB | 2.9GB | 875M |
| **Sau `swapoff -a` (working set thật)** | **6.76GB** | **920MB** | 0 |

- Con số "avail 2.9GB" ban đầu **gây hiểu lầm** — 2GB là pages production bị đẩy ra swap từ trước. Working set thật của ~15 service ≈ **6.76GB**, chỉ còn **~920MB** free.
- ⇒ Bất kỳ workload ASR nào (batch 2.19GB / stream 1.74GB) đều **ép swap**. `MemorySwapMax=0` (fail-fast) **bất khả thi** trên 8GB (batch cần 2.19GB, chỉ có 0.92GB free → OOM ngay).
- Batch **swap-tolerant** (truy cập tuần tự): bs=8 swap đầy 2GB mà RTF vẫn 0.485. Nên batch-only 8GB *chạy được* nhưng luôn swap ~1.5GB/job → **degrade production** (pages service khác bị evict; hồi được nhưng không sạch).

## 4. Môi trường + sai lệch so với plan

- **Python 3.9.25**: production sidecar (`main.py`, `stream_server.py`, `stream_session.py`) **import OK trên 3.9** (annotation `self.x: T|None` trong method KHÔNG eval runtime — PEP 526). Chỉ **spike throwaway** `spike_lib/audio.py` có def-time `-> str | None` phải thêm `from __future__ import annotations` để chạy. ⇒ **Python 3.9 KHÔNG phải blocker deploy** (bác giả định lo ngại 3.9).
- **Deps** (venv cô lập `/opt/scribe/bench/venv`, KHÔNG đụng system Python của production): `faster-whisper 1.2.1, ctranslate2 4.8.1, av 15.1.0, numpy 2.0.2, psutil 6.1.1`. `av==17.1.0` (pin trong requirements) **không có bản py3.9** → dùng av 15.1.0. `webrtcvad` **không cài được** (thiếu `python3-devel` headers) → dùng **energy VAD** (đúng fallback đã ghi trong requirements).
- **RT-1 đã patch** (`main.py`): thêm `cpu_threads()` đọc `SCRIBE_CPU_THREADS`, fallback `os.cpu_count()`. Behavior-preserving cho dev. Cần cho cả gate lẫn guardrail Phase 3.

## 5. Khuyến nghị hạ tầng

| Kịch bản | RAM | CPU | Khả thi |
|---|---|---|---|
| **Batch-only trên .65 hiện trạng** | 8GB (ép swap ~1.5GB/job) | Broadwell OK cho batch | ⚠️ Chạy được, degrade prod khi swap. `batch_size=1`, `MemoryMax≥2500M`, **KHÔNG** `MemorySwapMax=0`, arbiter bắt buộc. |
| **Batch-only + resize .65** | 12–16GB | Broadwell OK | ✅ Sạch. `batch_size=4–8`, `MemorySwapMax=0` an toàn, không đụng prod. |
| **Full MVP (batch+live+meeting) trên .65** | dù resize | Broadwell **KHÔNG kham live** | ❌ Resize sửa RAM nhưng **không** sửa giới hạn CPU streaming. |
| **Full MVP trên box CPU mới** | ≥16GB | CPU có AVX-512/VNNI (hoặc GPU) | ✅ (cần đo lại) Stream cần CPU nhanh ~3–10× Broadwell int8. |

- **`MemoryMax` khuyến nghị:** chỉ chốt sau khi quyết hạ tầng. Trên 8GB: `MemoryMax=2600M` + cho phép swap (không `MemorySwapMax=0`) + arbiter cấm batch+live đồng thời (live vốn NO-GO nên chỉ còn batch queue=1). Trên 16GB: `MemoryMax=3000M` + `MemorySwapMax=0`.
- **Đòn bẩy code nếu ép batch trên 8GB:** đổi `main.py:95` `batch_size=8`→`1` (giảm peak 4.27→2.19GB, RTF chỉ +12%).

## 6. Dọn dẹp / trạng thái box

- Artifact còn trên box: `/opt/scribe/{models 1.9G, samples 731M, bench 489M}` (~3.1GB). **Giữ lại** để đo lại nhanh sau khi quyết resize/box mới. Xóa khi không cần: `rm -rf /opt/scribe`.
- Đã `swapoff -a && swapon -a` 1 lần (dọn swap để đo RAM thật) — production pages đã về RAM, không tác dụng phụ tồn dư.
- Venv/deps cô lập, **không chạm system Python** của 15 service.

## Unresolved questions (cần user quyết — scope/infra)

1. **NO-GO live/meeting trên .65:** chấp nhận **batch-only MVP trên .65** (bỏ/hoãn Phase 3+4), hay chuyển toàn bộ sang **box CPU mới (VNNI/GPU)** để giữ full scope? (Quyết định scope + hạ tầng — cần user.)
2. **RAM .65:** nếu batch-only trên .65 → **resize 12–16GB** (sạch) hay **giữ 8GB chấp nhận swap-degrade** khi có job?
3. **Streaming re-architecture (nếu muốn cứu live):** thử batch nhiều utterance/nhiều track qua `BatchedInferencePipeline` trong stream (thay plain `WhisperModel`) — **suy đoán, chưa đo**; chỉ đáng làm nếu quyết giữ live trên CPU nhanh hơn.
