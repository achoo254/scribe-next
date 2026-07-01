"""Server streaming ASR cho meeting live-transcribe (Phase 3) — tiến trình RIÊNG (`main.py --stream`).

[C2] Cô lập khỏi batch Mode C: đây là process khác hẳn → `BatchedInferencePipeline.transcribe()`
blocking của batch KHÔNG bao giờ chặn caption. TRONG process này, luồng đọc stdin CHỈ định tuyến
(không chạy ASR) → mỗi track có worker thread + hàng đợi riêng; ASR nặng chạy trên worker, không
chặn router. Nhiều worker cùng ghi stdout → serialize bằng `_send_lock`.

Model WhisperModel(small,int8) nạp 1 lần (warm), CHIA SẺ mọi track, transcribe serialize bằng
`_model_lock` (encoder Whisper vốn serialize; active-speaker gating giới hạn ≤2 track nói cùng lúc).

Giao thức (line-delimited JSON qua stdio):
  stdin :  {"cmd":"stream_start","track":"<id>","model_dir":"<app models dir>","size":"small"}
           {"cmd":"stream_pcm","track":"<id>","pcm":"<base64 int16le>"}
           {"cmd":"stream_end","track":"<id>"}
           {"cmd":"shutdown"}
  stdout:  {"type":"ready"}
           {"type":"caption","track":"<id>","kind":"final","text":"...","t0":..,"t1":..}
           {"type":"backpressure","track":"<id>","dropped":N}
           {"type":"stream_error","track":"<id>","message":"..."} | {"type":"info","message":"..."}
"""
import base64
import json
import queue
import re
import sys
import threading

from model_paths import MODELS, resolve_model_path
from stream_session import StreamSession, load_stream_model

_QUEUE_MAX = 64          # ~ vài chục chunk (mỗi chunk ~0.25–0.5s) → bound RAM; đầy = drop + báo
_TRACK_RE = re.compile(r"^[A-Za-z0-9_-]{1,64}$")  # sanitize track id (defense-in-depth với Rust)

_send_lock = threading.Lock()      # nhiều worker ghi stdout → serialize
_model_lock = threading.Lock()     # serialize transcribe model chia sẻ giữa track
_load_lock = threading.Lock()      # RIÊNG cho nạp model → KHÔNG chặn router (_registry_lock) khi load
_registry_lock = threading.Lock()  # CHỈ bảo vệ dict _tracks (start/end/shutdown) — router phản hồi nhanh
_model = None
_tracks: dict[str, "_Track"] = {}


def _send(obj) -> None:
    with _send_lock:
        sys.stdout.write(json.dumps(obj, ensure_ascii=False) + "\n")
        sys.stdout.flush()


def _ensure_model(model_path: str):
    """Nạp model small 1 lần, chia sẻ. Dùng _load_lock RIÊNG (không phải _registry_lock) → nạp model
    KHÔNG chặn router xử lý start/end/shutdown. Gọi từ worker thread."""
    global _model
    if _model is not None:
        return _model
    with _load_lock:
        if _model is None:  # double-check trong lock
            _model = load_stream_model(model_path)
            _send({"type": "info", "message": "Streaming model small (CPU int8) sẵn sàng"})
    return _model


class _Track:
    def __init__(self, track_id: str, model_path: str):
        self.track_id = track_id
        self.model_path = model_path
        self.q: "queue.Queue[bytes | None]" = queue.Queue(maxsize=_QUEUE_MAX)
        self.dropped = 0
        self.thread = threading.Thread(target=self._run, name=f"stream-{track_id}", daemon=True)

    def _run(self) -> None:
        try:
            model = _ensure_model(self.model_path)
        except Exception as e:  # model không nạp được → track chết, báo rõ
            _send({"type": "stream_error", "track": self.track_id, "message": f"load model: {e}"})
            return
        session = StreamSession(model, _model_lock)
        while True:
            item = self.q.get()
            if item is None:  # stream_end / shutdown sentinel → chốt nốt utterance đang mở rồi thoát
                for cap in session.flush():
                    self._emit(cap)
                return
            try:
                for cap in session.feed_pcm(item):
                    self._emit(cap)
            except Exception as e:
                _send({"type": "stream_error", "track": self.track_id, "message": str(e)})

    def _emit(self, cap: dict) -> None:
        _send({"type": "caption", "track": self.track_id, **cap})

    def push(self, pcm: bytes) -> None:
        try:
            self.q.put_nowait(pcm)
        except queue.Full:
            # Đẩy nhanh hơn ASR xử lý → drop chunk (bound RAM, KHÔNG deadlock). Báo backpressure
            # gộp mỗi 25 lần để không spam stdout.
            self.dropped += 1
            if self.dropped % 25 == 1:
                _send({"type": "backpressure", "track": self.track_id, "dropped": self.dropped})

    def stop(self) -> None:
        try:
            self.q.put_nowait(None)
        except queue.Full:
            # Hàng đợi đầy nhưng vẫn phải dừng: xả 1 slot rồi đặt sentinel.
            try:
                self.q.get_nowait()
            except queue.Empty:
                pass
            self.q.put_nowait(None)


def _stream_start(track_id: str, model_dir: str, size: str) -> None:
    if not _TRACK_RE.match(track_id or ""):
        _send({"type": "stream_error", "track": track_id, "message": "track id không hợp lệ"})
        return
    size = size or "small"
    if size not in MODELS:  # chặn KeyError trong resolve_model_path (nếu không → giết router, mất mọi caption)
        _send({"type": "stream_error", "track": track_id, "message": f"size không hợp lệ: {size}"})
        return
    model_path = resolve_model_path(model_dir, size)
    with _registry_lock:
        if track_id in _tracks:
            return  # đã có → idempotent
        t = _Track(track_id, model_path)
        _tracks[track_id] = t
    t.thread.start()


def _stream_pcm(track_id: str, pcm_b64: str) -> None:
    t = _tracks.get(track_id)
    if t is None:
        return  # chưa stream_start (hoặc đã end) → bỏ qua, không dựng ngầm
    try:
        t.push(base64.b64decode(pcm_b64))
    except Exception as e:
        _send({"type": "stream_error", "track": track_id, "message": f"pcm decode: {e}"})


def _stream_end(track_id: str) -> None:
    with _registry_lock:
        t = _tracks.pop(track_id, None)
    if t is not None:
        t.stop()


def _shutdown_all() -> None:
    with _registry_lock:
        tracks = list(_tracks.values())
        _tracks.clear()
    for t in tracks:
        t.stop()


def run() -> None:
    try:
        sys.stdout.reconfigure(encoding="utf-8")
        sys.stdin.reconfigure(encoding="utf-8")
    except Exception:
        pass
    _send({"type": "ready"})
    for line in sys.stdin:
        line = line.strip().lstrip("﻿")  # phòng BOM lẫn vào dòng JSON
        if not line:
            continue
        try:
            req = json.loads(line)
        except Exception as e:
            _send({"type": "error", "message": f"JSON lỗi: {e}"})
            continue
        cmd = req.get("cmd")
        # Bọc dispatch: 1 lệnh lỗi (vd size lạ, payload méo) KHÔNG được giết router → mất caption mọi track.
        try:
            if cmd == "stream_start":
                _stream_start(req.get("track"), req.get("model_dir", ""), req.get("size", "small"))
            elif cmd == "stream_pcm":
                _stream_pcm(req.get("track"), req.get("pcm", ""))
            elif cmd == "stream_end":
                _stream_end(req.get("track"))
            elif cmd == "shutdown":
                _shutdown_all()
                break
        except Exception as e:  # noqa: BLE001 — router phải sống sót qua lệnh lỗi
            _send({"type": "error", "message": f"lệnh {cmd} lỗi: {e}"})
