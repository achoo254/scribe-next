"""VAD gate per-chunk cho active-speaker gating.

Mặc định: energy-RMS (numpy, zero-dep) — đủ cho spike đo tải.
Tùy chọn: webrtcvad (pure C ~50KB) nếu cài được — KHÔNG silero-vad (kéo PyTorch ~2GB, C4).
"""
import numpy as np

SR = 16000


class EnergyVad:
    """Speech nếu RMS(chunk) vượt ngưỡng dBFS. Đơn giản, không phụ thuộc thư viện ngoài."""

    def __init__(self, threshold_dbfs: float = -45.0):
        self.threshold = threshold_dbfs

    def is_speech(self, chunk: np.ndarray) -> bool:
        if len(chunk) == 0:
            return False
        rms = float(np.sqrt(np.mean(chunk.astype(np.float64) ** 2)))
        if rms <= 1e-9:
            return False
        dbfs = 20.0 * np.log10(rms)
        return dbfs > self.threshold


class WebrtcVad:
    """webrtcvad trên frame 30ms; chunk = speech nếu >=`ratio` frame là speech."""

    def __init__(self, aggressiveness: int = 2, ratio: float = 0.3):
        import webrtcvad  # ImportError → caller fallback energy

        self.vad = webrtcvad.Vad(aggressiveness)
        self.ratio = ratio
        self.frame = int(0.03 * SR)  # 30ms = 480 mẫu @16k

    def is_speech(self, chunk: np.ndarray) -> bool:
        pcm16 = (np.clip(chunk, -1.0, 1.0) * 32767).astype("<i2").tobytes()
        fb = self.frame * 2  # bytes/frame
        total = sp = 0
        for i in range(0, len(pcm16) - fb + 1, fb):
            total += 1
            if self.vad.is_speech(pcm16[i:i + fb], SR):
                sp += 1
        return total > 0 and (sp / total) >= self.ratio


def make_vad(kind: str, aggressiveness: int = 2, threshold_dbfs: float = -45.0):
    """Trả (vad, tên_thực_dùng). webrtc fallback energy nếu import lỗi."""
    if kind == "webrtc":
        try:
            return WebrtcVad(aggressiveness), "webrtc"
        except Exception:
            return EnergyVad(threshold_dbfs), "energy(webrtc-unavailable)"
    if kind == "none":
        return _AlwaysSpeech(), "none"
    return EnergyVad(threshold_dbfs), "energy"


class _AlwaysSpeech:
    def is_speech(self, chunk):
        return len(chunk) > 0
