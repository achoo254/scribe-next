"""OneScribe transcriber sidecar — faster-whisper (PhoWhisper-large) over JSON stdio.

Giao thức (line-delimited JSON):
  stdin :  {"cmd":"ensure_model","model_dir":"..."}
           {"cmd":"transcribe","file":"...","out_dir":"...","outputs":["txt","srt"],"model_dir":"..."}
           {"cmd":"shutdown"}
  stdout:  {"type":"ready"} | {"type":"download","pct":N} | {"type":"model_ready"}
           {"type":"progress","file":...,"pct":N} | {"type":"done",...} | {"type":"error","message":...}

Model giữ ấm (load 1 lần) → batch nhanh. CPU int8. Offline sau khi tải model về model_dir.
"""
import json
import os
import sys
from pathlib import Path

# Tắt hf-xet: tải qua HTTP/LFS thường (ổn định + tiến độ track được; xet hay kẹt model.bin).
os.environ.setdefault("HF_HUB_DISABLE_XET", "1")

MODEL_REPO = "kiendt/PhoWhisper-large-ct2"
_model = None  # giữ ấm giữa các lần transcribe

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


def ensure_model(model_dir: str):
    # Tiến độ % do phía Rust poll dung lượng thư mục (tin cậy hơn tqdm khi nhiều file/xet).
    from huggingface_hub import snapshot_download

    Path(model_dir).mkdir(parents=True, exist_ok=True)
    snapshot_download(MODEL_REPO, local_dir=model_dir)
    send({"type": "model_ready"})


def _add_cuda_dll_dirs():
    # Windows: nạp DLL CUDA (cublas/cudnn) từ package nvidia-*-cu12 trong site-packages.
    if os.name != "nt":
        return
    base = Path(sys.prefix) / "Lib" / "site-packages" / "nvidia"
    if not base.is_dir():
        return
    dirs = [str(sub / "bin") for sub in base.iterdir() if (sub / "bin").is_dir()]
    for d in dirs:
        try:
            os.add_dll_directory(d)
        except Exception:
            pass
    # ctranslate2 nạp cublas64_12.dll qua LoadLibrary theo PATH → prepend PATH cho chắc.
    if dirs:
        os.environ["PATH"] = os.pathsep.join(dirs) + os.pathsep + os.environ.get("PATH", "")


def get_model(model_dir: str):
    global _model
    if _model is None:
        _add_cuda_dll_dirs()
        from faster_whisper import WhisperModel
        try:
            _model = WhisperModel(model_dir, device="cuda", compute_type="int8_float16")
            send({"type": "info", "message": "Đang dùng GPU (CUDA)"})
        except Exception as e:
            send({"type": "info", "message": f"GPU không khả dụng, dùng CPU. ({e})"})
            _model = WhisperModel(
                model_dir, device="cpu", compute_type="int8", cpu_threads=os.cpu_count() or 8
            )
    return _model


def transcribe(file: str, out_dir: str, outputs, model_dir: str):
    model = get_model(model_dir)
    # beam=1 + VAD (bỏ khoảng lặng) → nhanh. initial_prompt (glossary) → đúng thuật ngữ.
    # no_repeat_ngram_size=3 → chặn câu/cụm lặp + chuỗi "..." (whisper artifact trên khoảng lặng).
    segments, info = model.transcribe(
        file,
        beam_size=1,
        language="vi",
        vad_filter=True,
        initial_prompt=DEFAULT_PROMPT,
        no_repeat_ngram_size=3,
    )
    duration = getattr(info, "duration", 0.0) or 0.0
    full, srt = [], []
    for i, seg in enumerate(segments, 1):
        full.append(seg.text)
        srt.append(f"{i}\n{fmt_ts(seg.start)} --> {fmt_ts(seg.end)}\n{seg.text.strip()}\n")
        if duration > 0:
            send({"type": "progress", "file": file, "pct": min(100, int(seg.end / duration * 100))})
    text = "".join(full).strip()
    stem = Path(file).stem
    out = Path(out_dir)
    out.mkdir(parents=True, exist_ok=True)
    txt_path = srt_path = None
    if "txt" in outputs:
        txt_path = str(out / f"{stem}.txt")
        Path(txt_path).write_text(text, encoding="utf-8")
    if "srt" in outputs:
        srt_path = str(out / f"{stem}.srt")
        Path(srt_path).write_text("\n".join(srt), encoding="utf-8")
    send({"type": "done", "file": file, "text": text, "txt_path": txt_path, "srt_path": srt_path})


def main():
    try:
        sys.stdout.reconfigure(encoding="utf-8")
        sys.stdin.reconfigure(encoding="utf-8")
    except Exception:
        pass
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
                ensure_model(req["model_dir"])
            elif cmd == "transcribe":
                transcribe(req["file"], req["out_dir"], req.get("outputs", ["txt", "srt"]), req["model_dir"])
            elif cmd == "shutdown":
                break
        except Exception as e:
            # Đính kèm "file" để mỗi file luôn settle đúng 1 message (done HOẶC error) — batch không kẹt.
            send({"type": "error", "file": req.get("file"), "message": str(e)})


if __name__ == "__main__":
    main()
