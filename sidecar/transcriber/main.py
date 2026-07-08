"""OneScribe transcriber sidecar — faster-whisper (PhoWhisper) over JSON stdio.

Giao thức (line-delimited JSON):
  stdin :  {"cmd":"ensure_model","model_dir":"...","size":"small|medium|large|large-v3-mlx"}
           {"cmd":"transcribe","file":"...","out_dir":"...","outputs":["txt","srt"],"model_dir":"...","size":"..."}
           {"cmd":"shutdown"}
  stdout:  {"type":"ready"} | {"type":"download","pct":N} | {"type":"model_ready"}
           {"type":"progress","file":...,"pct":N} | {"type":"done",...} | {"type":"error","message":...}

Đa model + 2 backend (chọn theo size, xem model_paths.backend_of):
- ct2 (small/medium/large): faster-whisper CPU int8 + BatchedInferencePipeline, giữ ấm 1 model.
- mlx (large-v3-mlx): mlx-whisper Metal GPU (Apple Silicon), tự cache model. Lazy import.
Offline sau khi tải model về model_dir.
"""
import json
import os
import sys
from pathlib import Path

from model_paths import MODELS, backend_of, resolve_model_path

# Tắt hf-xet: tải qua HTTP/LFS thường (ổn định + tiến độ track được; xet hay kẹt model.bin).
os.environ.setdefault("HF_HUB_DISABLE_XET", "1")

_model = None       # pipeline giữ ấm
_model_size = None  # size đang nạp — đổi size thì reload

# Glossary domain (IT/DevOps) → bias model nhận đúng tên riêng/tiếng Anh. Sửa/mở rộng tùy nội dung.
DEFAULT_PROMPT = (
    "Bài nói kỹ thuật tiếng Việt về hệ thống và DevOps. Thuật ngữ thường gặp: "
    "VictoriaLogs, VictoriaMetrics, Grafana, OpenTelemetry, observability, log, parse, "
    "JSON, query, cheatsheet, cluster, HA, Docker, server, port, IP, API, systemctl, rsyslog."
)


def send(obj):
    sys.stdout.write(json.dumps(obj, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def fmt_ts(seconds: float) -> str:
    ms = max(0, int(round(seconds * 1000)))
    h, ms = divmod(ms, 3_600_000)
    m, ms = divmod(ms, 60_000)
    s, ms = divmod(ms, 1000)
    return f"{h:02d}:{m:02d}:{s:02d},{ms:03d}"


def ensure_model(model_dir: str, size: str):
    # Tiến độ % do phía Rust poll dung lượng thư mục (tin cậy hơn tqdm khi nhiều file/xet).
    from huggingface_hub import snapshot_download

    repo, subdir = MODELS[size]
    Path(model_dir).mkdir(parents=True, exist_ok=True)
    patterns = [f"{subdir}/*"] if subdir else None  # chỉ tải đúng subfolder của size
    snapshot_download(repo, local_dir=model_dir, allow_patterns=patterns)
    send({"type": "model_ready"})


def get_model(model_dir: str, size: str):
    # CPU int8: chạy ổn định trên mọi máy + offline, không phụ thuộc DLL CUDA
    # (binary đóng gói không gói được cublas/cudnn → CUDA chỉ lỗi lúc decode, không fallback kịp).
    # Giữ ấm 1 model; đổi size → giải phóng cũ rồi nạp mới (tránh ngốn RAM nhiều model cùng lúc).
    global _model, _model_size
    if _model is None or _model_size != size:
        from faster_whisper import WhisperModel, BatchedInferencePipeline
        base = WhisperModel(
            resolve_model_path(model_dir, size), device="cpu", compute_type="int8",
            cpu_threads=os.cpu_count() or 8,
        )
        _model = BatchedInferencePipeline(model=base)  # batched → nhanh hơn ~2x trên CPU
        _model_size = size
        send({"type": "info", "message": f"Đang dùng CPU (int8) — model {size}"})
    return _model


def _write_outputs(file: str, out_dir: str, outputs, text: str, srt_lines):
    stem = Path(file).stem
    out = Path(out_dir)
    out.mkdir(parents=True, exist_ok=True)
    txt_path = srt_path = None
    if "txt" in outputs:
        txt_path = str(out / f"{stem}.txt")
        Path(txt_path).write_text(text, encoding="utf-8")
    if "srt" in outputs:
        srt_path = str(out / f"{stem}.srt")
        Path(srt_path).write_text("\n".join(srt_lines), encoding="utf-8")
    return txt_path, srt_path


def _transcribe_ct2(file: str, model_dir: str, size: str):
    model = get_model(model_dir, size)
    # batched + beam=1 + VAD (bỏ khoảng lặng) → nhanh. initial_prompt (glossary) → đúng thuật ngữ.
    segments, info = model.transcribe(
        file, batch_size=8, beam_size=1, language="vi",
        vad_filter=True, initial_prompt=DEFAULT_PROMPT,
    )
    duration = getattr(info, "duration", 0.0) or 0.0
    full, srt_lines = [], []
    for i, seg in enumerate(segments, 1):
        full.append(seg.text)
        srt_lines.append(f"{i}\n{fmt_ts(seg.start)} --> {fmt_ts(seg.end)}\n{seg.text.strip()}\n")
        if duration > 0:
            send({"type": "progress", "file": file, "pct": min(100, int(seg.end / duration * 100))})
    return "".join(full).strip(), srt_lines


def _transcribe_mlx(file: str, model_dir: str, size: str):
    # Apple Silicon Metal GPU. Lazy import: sidecar Windows/Intel không cài mlx → không chạm nhánh này.
    # mlx_whisper tự cache model (ModelHolder); không dùng get_model warm-pool của ct2.
    import mlx_whisper

    send({"type": "info", "message": f"Đang dùng MLX (Metal GPU) — model {size}"})
    result = mlx_whisper.transcribe(
        file, path_or_hf_repo=resolve_model_path(model_dir, size),
        language="vi", initial_prompt=DEFAULT_PROMPT,
    )
    segs = result.get("segments", []) or []
    srt_lines = [
        f"{i}\n{fmt_ts(s['start'])} --> {fmt_ts(s['end'])}\n{s['text'].strip()}\n"
        for i, s in enumerate(segs, 1)
    ]
    return (result.get("text") or "").strip(), srt_lines


def transcribe(file: str, out_dir: str, outputs, model_dir: str, size: str):
    if backend_of(size) == "mlx":
        text, srt_lines = _transcribe_mlx(file, model_dir, size)
    else:
        text, srt_lines = _transcribe_ct2(file, model_dir, size)
    txt_path, srt_path = _write_outputs(file, out_dir, outputs, text, srt_lines)
    send({"type": "done", "file": file, "text": text, "txt_path": txt_path, "srt_path": srt_path})


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
            elif cmd == "transcribe":
                transcribe(
                    req["file"], req["out_dir"], req.get("outputs", ["txt", "srt"]),
                    req["model_dir"], req.get("size", "medium"),
                )
            elif cmd == "shutdown":
                break
        except Exception as e:
            # Đính kèm "file" để mỗi file luôn settle đúng 1 message (done HOẶC error) — batch không kẹt.
            send({"type": "error", "file": req.get("file"), "message": str(e)})


if __name__ == "__main__":
    main()
