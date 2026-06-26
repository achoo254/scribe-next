"""Nạp audio bất kỳ → PCM 16kHz mono float32, và cắt chunk cho streaming.

Decode qua ffmpeg sidecar (chất lượng resample tốt + nhận mọi định dạng: wav/mp3/mp4/...).
Fallback `wave` stdlib cho WAV PCM thuần nếu không tìm thấy ffmpeg.

stream_pcm schema (chốt ở Phase 1): PCM 16kHz mono int16 little-endian. Ở đây giữ float32
trong [-1,1] cho faster-whisper; biên int16↔float32 mô phỏng đúng dữ liệu sẽ chảy từ WebView2.
"""
import os
import subprocess
import sys
import wave
from pathlib import Path

import numpy as np

SR = 16000


def find_ffmpeg() -> str | None:
    """Tìm ffmpeg: binary sidecar trong repo (mọi tổ tiên) → 'ffmpeg' trên PATH."""
    here = Path(__file__).resolve()
    names = ["ffmpeg-x86_64-pc-windows-msvc.exe", "ffmpeg.exe", "ffmpeg"]
    for anc in here.parents:
        bindir = anc / "src-tauri" / "binaries"
        for n in names:
            p = bindir / n
            if p.exists():
                return str(p)
    return "ffmpeg"  # để subprocess tự báo lỗi nếu không có trên PATH


def _load_via_ffmpeg(path: str, ffmpeg: str) -> np.ndarray | None:
    cmd = [ffmpeg, "-v", "error", "-i", path, "-ac", "1", "-ar", str(SR), "-f", "s16le", "-"]
    try:
        flags = 0x0800_0000 if os.name == "nt" else 0  # CREATE_NO_WINDOW
        r = subprocess.run(cmd, capture_output=True, creationflags=flags)
    except FileNotFoundError:
        return None
    if not r.stdout:
        err = r.stderr.decode("utf-8", "replace").strip()
        if err:  # ffmpeg có nhưng decode lỗi → báo để khỏi âm thầm thành track rỗng
            print(f"[warn] ffmpeg decode {path}: {err}", file=sys.stderr)
        return None
    return np.frombuffer(r.stdout, np.int16).astype(np.float32) / 32768.0


def _load_via_wave(path: str) -> np.ndarray:
    with wave.open(path, "rb") as w:
        n, ch, sw, sr = w.getnframes(), w.getnchannels(), w.getsampwidth(), w.getframerate()
        raw = w.readframes(n)
    if sw != 2:
        raise ValueError(f"WAV cần 16-bit PCM (sampwidth=2), nhận {sw}. Dùng ffmpeg để convert.")
    a = np.frombuffer(raw, np.int16).astype(np.float32) / 32768.0
    if ch > 1:
        a = a.reshape(-1, ch).mean(axis=1)  # downmix mono
    if sr != SR:  # resample linear (đủ cho spike đo; ffmpeg cho chất lượng tốt hơn)
        idx = np.linspace(0, len(a) - 1, int(len(a) * SR / sr))
        a = np.interp(idx, np.arange(len(a)), a).astype(np.float32)
    return a


def load_pcm_16k_mono(path: str, ffmpeg: str | None) -> np.ndarray:
    if ffmpeg:
        a = _load_via_ffmpeg(path, ffmpeg)
        if a is not None and len(a):
            return a
    if path.lower().endswith(".wav"):
        return _load_via_wave(path)
    print(f"[warn] không decode được {path} (cần ffmpeg cho định dạng này)", file=sys.stderr)
    return np.zeros(0, np.float32)


def chunk_indices(n_samples: int, chunk_ms: int):
    """Sinh (pos, end, t0, t1) cho từng chunk audio theo chunk_ms (end = chỉ số mẫu kết thúc)."""
    step = max(1, int(chunk_ms / 1000 * SR))
    pos = 0
    while pos < n_samples:
        end = min(pos + step, n_samples)
        yield pos, end, pos / SR, end / SR
        pos = end
