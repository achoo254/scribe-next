"""OneScribe transcriber sidecar — faster-whisper (PhoWhisper) over JSON stdio.

Giao thức (line-delimited JSON):
  stdin :  {"cmd":"ensure_model","model_dir":"...","size":"small|medium|large|large-v3-mlx"}
           {"cmd":"transcribe","file":"...","out_dir":"...","outputs":["txt","srt","vtt","json"],
            "model_dir":"...","size":"...","prompt":"<glossary optional>"}
           {"cmd":"shutdown"}
  stdout:  {"type":"ready"} | {"type":"download","pct":N} | {"type":"model_ready"}
           {"type":"progress","file":...,"pct":N} | {"type":"done",...} | {"type":"error","message":...}

Đa model + 2 backend (chọn theo size, xem model_paths.backend_of):
- ct2 (small/medium/large): faster-whisper CPU int8, decode tuần tự, giữ ấm 1 model.
- mlx (large-v3-mlx): mlx-whisper Metal GPU (Apple Silicon), tự cache model. Lazy import.
Offline sau khi tải model về model_dir.

word_timestamps BẬT mặc định cả 2 backend → có mốc từng từ để (1) resegment SRT/VTT chuẩn phụ đề
(≤42 ký tự/dòng, ≤2 dòng, ≤6s/cue) và (2) xuất JSON có words[] cho player click-to-seek (UI).
Glossary (initial_prompt) truyền từ app (settings) — rỗng thì không kèm prompt.
"""
import json
import os
import sys
from pathlib import Path

from model_paths import MODELS, backend_of, resolve_model_path
import cuda_runtime

# Tắt hf-xet: tải qua HTTP/LFS thường (ổn định + tiến độ track được; xet hay kẹt model.bin).
os.environ.setdefault("HF_HUB_DISABLE_XET", "1")

_model = None        # pipeline giữ ấm
_model_size = None   # size đang nạp — đổi size thì reload
_model_device = None # device đang nạp ("cpu"/"cuda") — đổi device (toggle GPU) thì reload

# Giới hạn phụ đề (chuẩn thường dùng): ~42 ký tự/dòng, tối đa 2 dòng, ~6s mỗi cue.
SRT_MAX_CHARS = 42
SRT_MAX_DUR = 6.0

# Chống vòng lặp/hallucination, dùng chung 2 backend. Decode tuần tự (KHÔNG BatchedInferencePipeline):
# batched ép temperature=0 không fallback → model rơi vào vòng lặp ("vâng, vâng…") là kẹt tới hết chunk.
# Tuần tự giữ temperature fallback + compression_ratio_threshold; không nối text đoạn trước để lặp không
# lan sang đoạn sau; bỏ đoạn hallucination nằm trong khoảng lặng >2s (cần word_timestamps).
ANTI_LOOP_OPTS = {"condition_on_previous_text": False, "hallucination_silence_threshold": 2.0}


def send(obj):
    sys.stdout.write(json.dumps(obj, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def fmt_ts(seconds: float, sep: str = ",") -> str:
    # sep="," → SRT; sep="." → VTT.
    ms = max(0, int(round(seconds * 1000)))
    h, ms = divmod(ms, 3_600_000)
    m, ms = divmod(ms, 60_000)
    s, ms = divmod(ms, 1000)
    return f"{h:02d}:{m:02d}:{s:02d}{sep}{ms:03d}"


def ensure_model(model_dir: str, size: str):
    # Tiến độ % do phía Rust poll dung lượng thư mục (tin cậy hơn tqdm khi nhiều file/xet).
    from huggingface_hub import snapshot_download

    repo, subdir = MODELS[size]
    Path(model_dir).mkdir(parents=True, exist_ok=True)
    patterns = [f"{subdir}/*"] if subdir else None  # chỉ tải đúng subfolder của size
    snapshot_download(repo, local_dir=model_dir, allow_patterns=patterns)
    send({"type": "model_ready"})


def get_model(model_dir: str, size: str, use_gpu: bool = False, cuda_dir=None):
    # GPU (CUDA float16) nếu bật + CUDA runtime sẵn sàng, else CPU int8 (ổn định mọi máy, offline).
    # Giữ ấm 1 model; đổi size HOẶC device → giải phóng cũ rồi nạp mới (device vào cache key để
    # toggle GPU on/off giữa 2 lần transcribe cùng size không dùng nhầm model đã nạp sai device).
    global _model, _model_size, _model_device
    device_key = "cuda" if use_gpu else "cpu"
    if _model is None or _model_size != size or _model_device != device_key:
        # gpu_device TRƯỚC: nó gọi os.add_dll_directory(cuda_dir) rồi mới import faster_whisper —
        # phải là lần import faster_whisper/ctranslate2 ĐẦU TIÊN của process để DLL search thấy CUDA.
        import gpu_device
        _model, actual_device, actual_compute = gpu_device.load_whisper_model(
            resolve_model_path(model_dir, size), use_gpu, cuda_dir,
            os.cpu_count() or 8, send_info=lambda m: send({"type": "info", "message": m}),
        )
        _model_size, _model_device = size, actual_device  # actual_device (GPU fail → "cpu")
        send({"type": "info", "message": f"Đang dùng {actual_device.upper()} ({actual_compute}) — model {size}"})
    return _model


# ── Chuẩn hoá segment về shape chung: {start, end, text, words:[{start,end,word}]} ──────────────

def _wrap_two_lines(text: str, max_chars: int) -> str:
    """Gói text thành 2 dòng, cắt ở ranh giới từ CÂN BẰNG (điểm cắt gần giữa) → cả 2 dòng ngắn +
    ≤max_chars khi tổng ≤2*max_chars. Từ đơn dài hơn max_chars thì đành tràn (không cắt giữa từ)."""
    text = " ".join(text.split())
    if len(text) <= max_chars:
        return text
    words = text.split()
    best_i, best_diff = 1, None
    for i in range(1, len(words)):
        a = len(" ".join(words[:i]))
        b = len(" ".join(words[i:]))
        # ưu tiên điểm cắt mà dòng 1 ≤max_chars; trong đó chọn cái cân nhất (|a-b| nhỏ nhất).
        if a <= max_chars and (best_diff is None or abs(a - b) < best_diff):
            best_i, best_diff = i, abs(a - b)
    return " ".join(words[:best_i]) + "\n" + " ".join(words[best_i:])


def resegment_cues(segments, max_chars=SRT_MAX_CHARS, max_dur=SRT_MAX_DUR):
    """Từ segment (có words) → cue phụ đề chuẩn: gộp/tách theo ngân sách ký tự + thời lượng +
    ranh giới câu. Không có words (fallback) → mỗi segment 1 cue (chỉ wrap 2 dòng)."""
    budget = max_chars * 2
    cues, cur = [], []

    def flush():
        nonlocal cur
        if cur:
            text = "".join(w["word"] for w in cur).strip()
            if text:
                cues.append({
                    "start": cur[0]["start"],
                    "end": cur[-1]["end"],
                    "text": _wrap_two_lines(text, max_chars),
                })
            cur = []

    for seg in segments:
        words = seg.get("words") or []
        if not words:
            flush()
            txt = (seg.get("text") or "").strip()
            if txt:
                cues.append({
                    "start": seg["start"], "end": seg["end"],
                    "text": _wrap_two_lines(txt, max_chars),
                })
            continue
        for w in words:
            if cur:
                cur_len = sum(len(x["word"]) for x in cur)
                dur = w["end"] - cur[0]["start"]
                if cur_len + len(w["word"]) > budget or dur > max_dur:
                    flush()
            cur.append(w)
            if w["word"].strip().endswith((".", "?", "!", "…")):
                flush()
    flush()
    return cues


def to_srt(cues) -> str:
    return "\n".join(
        f"{i}\n{fmt_ts(c['start'])} --> {fmt_ts(c['end'])}\n{c['text']}\n"
        for i, c in enumerate(cues, 1)
    )


def to_vtt(cues) -> str:
    body = "\n".join(
        f"{fmt_ts(c['start'], '.')} --> {fmt_ts(c['end'], '.')}\n{c['text']}\n"
        for c in cues
    )
    return "WEBVTT\n\n" + body


def to_json(segments, text: str) -> str:
    return json.dumps({"text": text, "segments": segments}, ensure_ascii=False, indent=2)


# ── Backend ─────────────────────────────────────────────────────────────────────────────────

def _transcribe_ct2(file: str, model_dir: str, size: str, prompt, use_gpu=False, cuda_dir=None):
    model = get_model(model_dir, size, use_gpu, cuda_dir)
    # beam=1 + VAD (bỏ khoảng lặng) → nhanh. word_timestamps → resegment phụ đề + JSON.
    segments, info = model.transcribe(
        file, beam_size=1, language="vi",
        vad_filter=True, word_timestamps=True, initial_prompt=prompt or None, **ANTI_LOOP_OPTS,
    )
    duration = getattr(info, "duration", 0.0) or 0.0
    norm, full = [], []
    for seg in segments:
        full.append(seg.text)
        words = [
            {"start": float(w.start), "end": float(w.end), "word": w.word}
            for w in (seg.words or [])
        ]
        norm.append({
            "start": float(seg.start), "end": float(seg.end), "text": seg.text, "words": words,
        })
        if duration > 0:
            send({"type": "progress", "file": file, "pct": min(100, int(seg.end / duration * 100))})
    return "".join(full).strip(), norm


def _transcribe_mlx(file: str, model_dir: str, size: str, prompt):
    # Apple Silicon Metal GPU. Lazy import: sidecar Windows/Intel không cài mlx → không chạm nhánh này.
    # mlx_whisper tự cache model (ModelHolder); không dùng get_model warm-pool của ct2.
    import mlx_whisper
    from mlx_whisper.audio import SAMPLE_RATE

    # Decode bằng PyAV (faster_whisper.audio, đã bundle) thay vì để mlx_whisper tự gọi CLI `ffmpeg`
    # từ PATH: app bundle mở từ Finder có PATH tối thiểu (không /opt/homebrew/bin) → Errno 2.
    from faster_whisper.audio import decode_audio

    send({"type": "info", "message": f"Đang dùng MLX (Metal GPU) — model {size}"})
    audio = decode_audio(file, sampling_rate=SAMPLE_RATE)
    result = mlx_whisper.transcribe(
        audio, path_or_hf_repo=resolve_model_path(model_dir, size),
        language="vi", word_timestamps=True, initial_prompt=prompt or None, **ANTI_LOOP_OPTS,
    )
    norm = []
    for s in result.get("segments", []) or []:
        # .get + lọc word thiếu mốc → degrade nhẹ (bỏ word đó), KHÔNG KeyError hỏng cả file.
        words = [
            {"start": float(w["start"]), "end": float(w["end"]), "word": w.get("word", "")}
            for w in (s.get("words") or [])
            if w.get("start") is not None and w.get("end") is not None
        ]
        norm.append({
            "start": float(s["start"]), "end": float(s["end"]),
            "text": s.get("text", ""), "words": words,
        })
    return (result.get("text") or "").strip(), norm


def _write_outputs(file: str, out_dir: str, outputs, text: str, segments):
    stem = Path(file).stem
    out = Path(out_dir)
    out.mkdir(parents=True, exist_ok=True)
    cues = resegment_cues(segments)
    paths = {"txt": None, "srt": None, "vtt": None, "json": None}
    writers = {
        "txt": lambda: text,
        "srt": lambda: to_srt(cues),
        "vtt": lambda: to_vtt(cues),
        "json": lambda: to_json(segments, text),
    }
    for fmt, make in writers.items():
        if fmt in outputs:
            p = out / f"{stem}.{fmt}"
            p.write_text(make(), encoding="utf-8")
            paths[fmt] = str(p)
    return paths


def transcribe(file: str, out_dir: str, outputs, model_dir: str, size: str, prompt=None,
               use_gpu=False, cuda_dir=None):
    if backend_of(size) == "mlx":
        text, segments = _transcribe_mlx(file, model_dir, size, prompt)  # MLX: GPU riêng, bỏ qua cuda
    else:
        text, segments = _transcribe_ct2(file, model_dir, size, prompt, use_gpu, cuda_dir)
    paths = _write_outputs(file, out_dir, outputs, text, segments)
    send({
        "type": "done", "file": file, "text": text, "segments": segments,
        "txt_path": paths["txt"], "srt_path": paths["srt"],
        "vtt_path": paths["vtt"], "json_path": paths["json"],
    })


def _install_orphan_guard():
    """Sidecar tự kết liễu khi app cha chết ĐỘT NGỘT (crash/force-quit — Rust không kịp kill),
    tránh transcribe chạy ngầm ngốn CPU/GPU. Theo dõi PID app (env Rust truyền); app chết → kill
    cả process group của sidecar (Rust đã đặt group: worker + resource_tracker của onefile) rồi thoát cứng.
    """
    parent = os.environ.get("ONESCRIBE_PARENT_PID")
    if not parent:
        return
    try:
        ppid = int(parent)
    except ValueError:
        return
    import signal
    import threading
    import time

    def _watch():
        while True:
            time.sleep(1.0)
            try:
                os.kill(ppid, 0)  # signal 0 = chỉ kiểm tra tồn tại
            except ProcessLookupError:
                try:
                    os.killpg(os.getpgrp(), signal.SIGKILL)  # Unix: dọn cả group; Windows: AttributeError → bỏ qua
                except (OSError, AttributeError):
                    pass
                os._exit(1)
            except OSError:
                pass  # PermissionError… → app còn sống, tiếp tục

    threading.Thread(target=_watch, daemon=True, name="orphan-guard").start()


def main():
    try:
        sys.stdout.reconfigure(encoding="utf-8")
        sys.stdin.reconfigure(encoding="utf-8")
    except Exception:
        pass
    _install_orphan_guard()
    send({"type": "ready"})
    for line in sys.stdin:
        line = line.strip().lstrip("﻿")  # phòng BOM lẫn vào dòng JSON
        if not line:
            continue
        try:
            req = json.loads(line)
        except Exception as e:
            send({"type": "error", "message": f"JSON lỗi: {e}"})
            continue
        cmd = req.get("cmd")
        try:
            if cmd == "ensure_model":
                ensure_model(req["model_dir"], req.get("size", "medium"))
            elif cmd == "ensure_cuda":
                cuda_runtime.ensure_cuda_runtime(req["cuda_dir"])
                send({"type": "cuda_ready"})
            elif cmd == "transcribe":
                transcribe(
                    req["file"], req["out_dir"], req.get("outputs", ["txt", "srt"]),
                    req["model_dir"], req.get("size", "medium"), req.get("prompt"),
                    req.get("use_gpu", False), req.get("cuda_dir"),
                )
            elif cmd == "shutdown":
                break
        except Exception as e:
            # Đính kèm "file" để mỗi file luôn settle đúng 1 message (done HOẶC error) — batch không kẹt.
            send({"type": "error", "file": req.get("file"), "message": str(e)})


if __name__ == "__main__":
    main()
