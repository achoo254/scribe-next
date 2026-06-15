"""OneScribe transcriber sidecar — faster-whisper (PhoWhisper) over JSON stdio.

Giao thức (line-delimited JSON):
  stdin :  {"cmd":"ensure_model","model_dir":"...","size":"small|medium|large"}
           {"cmd":"transcribe","file":"...","out_dir":"...","outputs":["txt","srt"],"model_dir":"...","size":"..."}
           {"cmd":"shutdown"}
  stdout:  {"type":"ready"} | {"type":"download","pct":N} | {"type":"model_ready"}
           {"type":"progress","file":...,"pct":N} | {"type":"done",...} | {"type":"error","message":...}

Đa model (small/medium/large): giữ ấm 1 model tại 1 thời điểm, đổi size → load lại.
CPU int8 + BatchedInferencePipeline (nhanh). Offline sau khi tải model về model_dir.
"""
import json
import os
import sys
from pathlib import Path

# Tắt hf-xet: tải qua HTTP/LFS thường (ổn định + tiến độ track được; xet hay kẹt model.bin).
os.environ.setdefault("HF_HUB_DISABLE_XET", "1")

# size → (repo HF, thư mục con CT2 trong repo). large dùng bản đã tải sẵn (kiendt, không subfolder);
# small/medium từ quocphu (mỗi size 1 subfolder CT2). Bỏ tiny/base (chất lượng quá thấp).
MODELS = {
    "small": ("quocphu/PhoWhisper-ct2-FasterWhisper", "PhoWhisper-small-ct2-fasterWhisper"),
    "medium": ("quocphu/PhoWhisper-ct2-FasterWhisper", "PhoWhisper-medium-ct2-fasterWhisper"),
    "large": ("kiendt/PhoWhisper-large-ct2", ""),
}
_model = None       # pipeline giữ ấm
_model_size = None  # size đang nạp — đổi size thì reload


def resolve_model_path(model_dir: str, size: str) -> str:
    # Đường dẫn thật chứa config.json/model.bin (small/medium nằm trong subfolder của repo).
    _, subdir = MODELS[size]
    return os.path.join(model_dir, subdir) if subdir else model_dir

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


def transcribe(file: str, out_dir: str, outputs, model_dir: str, size: str):
    model = get_model(model_dir, size)
    # batched + beam=1 + VAD (bỏ khoảng lặng) → nhanh. initial_prompt (glossary) → đúng thuật ngữ.
    segments, info = model.transcribe(
        file,
        batch_size=8,
        beam_size=1,
        language="vi",
        vad_filter=True,
        initial_prompt=DEFAULT_PROMPT,
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
