# Spike Streaming ASR — Kết quả & GO/NO-GO (Phase 1)

**Ngày:** 2026-06-26 · **Máy đo:** dev (16 lõi) · **OS target:** Windows-first (đã chốt)
**Trạng thái:** Harness **HOÀN THÀNH + verified chạy**. Số đo GO/NO-GO chính thức **chờ mẫu speech tiếng Việt thật** (xem §6).

> Spike = code throwaway để LẤY SỐ, KHÔNG phải production. Lõi `WhisperModel` trần int8 (KHÔNG BatchedInferencePipeline) theo [C1].

## 1. Deliverable

| File | Vai trò |
|---|---|
| `sidecar/transcriber/stream_asr_spike.py` | Entry/CLI + realtime feeder + per-track threads + RAM probe |
| `sidecar/transcriber/spike_lib/audio.py` | Nạp audio→PCM 16k mono qua ffmpeg sidecar + chunker |
| `sidecar/transcriber/spike_lib/vad.py` | VAD energy-RMS (zero-dep) + webrtcvad opt-in |
| `sidecar/transcriber/spike_lib/engine.py` | `WhisperModel` trần int8 + 2 chiến lược (VAD-cut, LocalAgreement-2) |
| `sidecar/transcriber/spike_lib/metrics.py` | RSS (psutil→fallback ctypes Win32) + CPU% + latency/RTF p50/p95 |
| `sidecar/transcriber/requirements-spike.txt` | Deps TÙY CHỌN (psutil, webrtcvad) — KHÔNG đụng requirements production |

## 2. Phát hiện đã VERIFIED (không cần mẫu speech)

Các hardening red-team được xác nhận **khả thi trên stack thật**:

- **[C1] `WhisperModel` trần int8 CPU** chạy được, đo RTF/RSS đúng đường (không qua BatchedInferencePipeline, không cuda/float16). ✓
- **[C3] LocalAgreement KHÔNG cần PyPI:** tự implement LocalAgreement-2 (word-level, commit prefix khớp 2 lần liên tiếp) ngay trong `engine.py`. KHÔNG cần `whisper_online.py` UFAL, KHÔNG `pip install whisper-streaming`. ✓
- **[C4] VAD KHÔNG cần PyTorch:** `webrtcvad==2.0.10` **build wheel thành công trên Python 3.14 Windows** (cp314 wheel, ~20KB, MSVC). Fallback energy-RMS zero-dep nếu môi trường thiếu compiler. KHÔNG silero-vad. ✓
- **psutil==6.1.1** cài được (wheel cp37-abi3). Fallback ctypes Win32 `GetProcessMemoryInfo` cũng đo RSS đúng (verified 21.6 MB standalone) → đo RAM được kể cả khi thiếu psutil. ✓
- **stream_pcm schema:** PCM **16kHz mono int16 little-endian**, chunk mặc định 1000ms; biên int16↔float32 mô phỏng đúng dữ liệu sẽ chảy từ WebView2. ✓
- **[C2] Tách luồng:** mỗi track 1 thread riêng, model dùng chung sau `Lock` (serialize compute = đúng nút thắt CPU) HOẶC `--isolated-models` (mỗi track 1 model, đo RAM/parallel). Không nhét vào vòng `for line in sys.stdin` đơn luồng của `main.py`. ✓
- **Cơ chế đo verified end-to-end** (smoke-test medium/small + tone tổng hợp): caption partial/final + latency_s per-final + RTF per-transcribe + peak RSS + avg CPU% + threading 2 track không crash, không lỗi unicode (stdout reconfigure utf-8 + emit_line guard). ✓

### Hardening sau code-review (3 lỗi đo lường High đã sửa + verified)
Code-review phát hiện 3 lỗi làm SAI số gate — đã sửa (đo sai = GO/NO-GO sai):
- **[H1] RTF không còn tính nhầm thời gian chờ lock:** timer đặt TRONG lock (`engine.transcribe_words`) → `rtf_*` = compute thuần. Thêm `aggregate_rtf_wall` = wall/total_audio = throughput trung thực (phản ánh serialize khi shared model). ✓
- **[H2] LocalAgreement buffer bị BOUND:** flush theo silence-hangover (utterance end) + hard-cap `max_buffer_s` ép flush khi không commit. **Unit-test:** 120s liên tục không commit + không silence → peak buffer 18.0s (không phình). Trước fix sẽ phình vô hạn → treo/RTF suy biến trên speech thật. ✓
- **[H3] Latency realtime tự gắn cờ tin cậy:** theo dõi backlog feeder; nếu tụt > 1 chunk sau wall-clock → `latency_valid:false` + warn "latency LẠC QUAN". Verified live (tone+small → đúng cờ false). ✓
- Phụ: space tránh dính câu (M2), bỏ mẫu CPU đầu (M3), chunk slice (L1), log ffmpeg lỗi (L2), `cpu_threads_per_model` vào summary (M4).

## 3. Cách chạy (dev)

```powershell
cd sidecar\transcriber
# (tùy chọn) deps đo tốt hơn:
.venv\Scripts\python.exe -m pip install -r requirements-spike.txt

# Đo 1 người nói (RTF + latency), mô phỏng i5 4 nhân, LocalAgreement, realtime:
.venv\Scripts\python.exe stream_asr_spike.py `
  --model-dir "$env:APPDATA\vn.inet.onescribe\models\small\PhoWhisper-small-ct2-fasterWhisper" `
  --wav mau_1nguoi.wav --cpu-threads 4 --strategy local-agreement --realtime --vad webrtc `
  --out spike_1track.json

# 2 người nói chồng (kịch bản gate ≤4s latency):
.venv\Scripts\python.exe stream_asr_spike.py --model-dir "<small>" `
  --wav nguoi_a.wav --wav nguoi_b.wav --cpu-threads 4 --strategy local-agreement --realtime --out spike_2track.json

# Đo RAM small+medium đồng thời (kiểm OOM 8GB) [C1]:
.venv\Scripts\python.exe stream_asr_spike.py --probe-ram `
  --model-dir "<small>" --medium-dir "$env:APPDATA\vn.inet.onescribe\models\medium\PhoWhisper-medium-ct2-fasterWhisper"
```

Tham số nên quét (Step 5 plan): `--chunk-ms` 500↔1000, `--strategy` vad-cut↔local-agreement, `--vad` energy↔webrtc, `--cpu-threads` 4 (i5) ↔ full (dev baseline).

## 4. Tiêu chí GO — trạng thái

| # | Tiêu chí | Ngưỡng | Trạng thái |
|---|---|---|---|
| G1 | RTF mỗi luồng đang nói (WhisperModel CPU int8, small) | < 1.0 | ⏳ chờ đo (cần speech thật) |
| G2 | Latency caption final sau khi nói xong câu, 2 người chồng, máy tối thiểu | ≤ ~4s | ⏳ chờ đo |
| G3 | RAM small+medium đồng thời KHÔNG OOM trên 8GB | peak < ~6.5GB | ✅ **ĐẠT** — peak **2085.7 MB** (small 349.9 MB → +medium = ~2.1GB). 8GB còn ~6GB headroom |
| G4 | VAD chọn xong, KHÔNG kéo PyTorch | webrtcvad/energy | ✅ ĐẠT (webrtcvad build OK Py3.14) |
| G5 | Chất lượng caption small "đủ đọc" tiếng Việt kỹ thuật | định tính | ⏳ chờ đánh giá trên mẫu thật |
| G6 | Chân AudioWorklet→Rust→sidecar trong WebView2 chạy + đo được | round-trip đo được | ❌ CHƯA — mini-spike riêng (xem §5) |

## 5. Unknown khó nhất còn lại — chân audio WebView2 [C5]

`stream_asr_spike.py` chứng minh phần **ASR Python** (latency/RTF/RAM). Phần **chưa phủ**: đường audio thật `getUserMedia`/remote track → `AudioWorklet` → `invoke()` Rust → sidecar **trong WebView2**. Đây là mini-spike riêng (cần app chạy thật, không đo offline được), gồm:

- Kiểm `MediaStreamTrackProcessor` có bật trong WebView2 (Windows) không; nếu không → dùng `AudioWorklet` + `port.postMessage` chuyển PCM.
- `SharedArrayBuffer` cần COOP/COEP — kiểm `tauri://` set header được không; nếu không, fallback copy qua `postMessage` (ArrayBuffer transfer).
- Tauri command Rust nhận chunk PCM (binary, length-prefix) → ghi stdin sidecar streaming. Đo throughput + round-trip latency thật.

**Khuyến nghị:** chạy mini-spike này NGAY SAU khi §6 cho RTF/latency Python đạt — vì nếu chân WebView2 không đạt, latency Python không cứu được (theo plan).

## 6. Việc cần người dùng để chốt GO/NO-GO

1. **Mẫu speech tiếng Việt thật** (plan Step 2): 1 file 1 người nói liên tục; 1 file/kịch bản 2–3 người nói chồng (ghép từ mic/loopback hoặc nhiều WAV); có thuật ngữ kỹ thuật. → chưa có trong repo.
2. **Chạy trên máy host tối thiểu thật** (laptop i5 4 nhân, 8–16GB, no GPU) — hoặc dùng `--cpu-threads 4` trên máy dev làm proxy + ghi rõ là proxy.
3. Điền số vào §4 (G1/G2/G3/G5) + kết luận GO/NO-GO.

> Model **PhoWhisper-small** đã được tải về `models/small/` trong phiên này (app cũng nhận ra). Sẵn sàng chạy ngay khi có mẫu audio.

## 7. Kết luận sơ bộ

- **Khả thi kỹ thuật của hướng tiếp cận = TÍCH CỰC:** mọi rào cản red-team (C1/C3/C4, dep Py3.14, đo RAM/CPU/latency, tách luồng, schema PCM) đều **đã gỡ được trên stack thật** — không có blocker "chết người" ở tầng thiết kế.
- **G3 (RAM) + G4 (VAD) đã ĐẠT bằng số đo thật.** Nỗi lo Critical [C1] về OOM dual-model trên 8GB được **bác bỏ** (peak ~2.1GB).
- **GO/NO-GO định lượng còn chờ:** RTF/latency trên speech thật (G1/G2) + chất lượng định tính (G5) + chân WebView2 (G6).
- **KHÔNG nên cấp ngân sách Phase 2+ trước khi G1/G2 + G6 có số** (đúng cổng tái đánh giá của plan).

## 8. Tiếp tục trên MÁY KHÁC (handoff — đọc trước khi làm tiếp)

> State đã commit+push. Code spike + plan + report nằm trong git. NHƯNG model + venv deps +
> mẫu audio là LOCAL máy cũ, KHÔNG theo git — phải dựng lại trên máy mới:

**Setup (1 lần):**
```powershell
cd sidecar\transcriber
# 1. venv + deps (nếu .venv chưa có): python -m venv .venv
.venv\Scripts\python.exe -m pip install -r requirements.txt -r requirements-spike.txt
# 2. Tải model small (+ medium cho probe-ram). Cách nhanh: mở app OneScribe, picker chọn small+medium
#    để app tự tải về %APPDATA%\vn.inet.onescribe\models\. HOẶC chạy snapshot_download:
.venv\Scripts\python.exe -c "import os;os.environ['HF_HUB_DISABLE_XET']='1';from huggingface_hub import snapshot_download as d;d('quocphu/PhoWhisper-ct2-FasterWhisper',local_dir=os.path.expandvars(r'%APPDATA%\vn.inet.onescribe\models\small'),allow_patterns=['PhoWhisper-small-ct2-fasterWhisper/*'])"
# 3. Chuẩn bị WAV speech tiếng Việt (xem §6).
```

**Việc tiếp theo (ưu tiên):**
1. **G1/G2** — chạy `stream_asr_spike.py` (lệnh §3) trên mẫu speech thật, `--cpu-threads 4` mô phỏng i5, cả `vad-cut` lẫn `local-agreement`, `--realtime`. **Kiểm `latency_valid` trong summary** — nếu `false` thì latency không tin được (máy/threads không kham nổi → đó cũng là dữ liệu NO-GO). Quét `--chunk-ms 500/1000`, `--vad energy/webrtc`. Điền §4 + kết luận.
2. **G6** — mini-spike chân audio WebView2 (§5): unknown khó nhất, cần app chạy. Nếu G6 fail thì latency Python không cứu được.
3. Khi G1/G2/G5/G6 xong → chốt GO/NO-GO → mới mở cổng Phase 2.

**Đã ĐẠT, không cần làm lại:** G3 (RAM ~2.1GB no-OOM), G4 (webrtcvad Py3.14 OK). Harness đã qua code-review (H1/H2/H3 fixed).

**Quyết định môi trường đã chốt phiên này:** Meeting MVP = **Windows-first**.

## Câu hỏi mở

- Mẫu speech tiếng Việt đa người: người dùng tự thu hay tạo bằng TTS (skill `ai-multimodal`)? TTS tiện nhưng không phản ánh cross-talk/giọng thật của họp.
- Mini-spike WebView2 (§5): làm trong Phase 1 (đúng ý [C5] "không để tới Phase 3") hay tách phase con riêng?
