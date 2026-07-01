"""Per-track streaming ASR session cho meeting live-transcribe (Phase 3).

Port production-grade từ spike Phase 1 (đo đạc trong spike_lib/, throwaway). Chiến lược = **vad-cut
utterance-based + max-utterance cap ~8s**. KHÔNG LocalAgreement: spike đo NO-GO trên CPU yếu
(re-transcribe buffer mỗi commit → RTF wall ~5x, feeder tụt hàng chục giây).

Vào : PCM 16kHz mono int16 little-endian (bytes) theo chunk (WebView2 → Rust → đây, base64 qua stdio).
Ra  : caption dict {"kind":"partial"|"final","text","t0","t1"} — hiện chỉ phát "final" theo utterance.

Mỗi track = 1 StreamSession độc lập. WhisperModel(small,int8) CHIA SẺ giữa track, bảo vệ bằng lock
(encoder Whisper serialize + cap số track ASR đồng thời ≤2 ở tầng trên = active-speaker gating).
"""
import os
import time

import numpy as np

SR = 16000
_MIN_UTTERANCE_S = 0.2  # bỏ utterance ngắn hơn (nhiễu/khịt mũi) — dưới ngưỡng này không transcribe

# Glossary domain (IT/DevOps) → bias model nhận đúng tên riêng/tiếng Anh. Đồng bộ main.DEFAULT_PROMPT.
DEFAULT_PROMPT = (
    "Bài nói kỹ thuật tiếng Việt về hệ thống và DevOps. Thuật ngữ thường gặp: "
    "VictoriaLogs, VictoriaMetrics, Grafana, OpenTelemetry, observability, log, parse, "
    "JSON, query, cheatsheet, cluster, HA, Docker, server, port, IP, API, systemctl, rsyslog."
)


def load_stream_model(model_path: str, cpu_threads: int = 0, compute_type: str = "int8"):
    """Nạp WhisperModel trần (KHÔNG BatchedInferencePipeline — sai mô hình streaming, RTF lệch)."""
    from faster_whisper import WhisperModel

    return WhisperModel(
        model_path,
        device="cpu",
        compute_type=compute_type,
        cpu_threads=cpu_threads or (os.cpu_count() or 4),
    )


def _transcribe_utterance(model, audio: np.ndarray, lock, beam: int, prompt: str) -> str:
    """Transcribe 1 utterance float32 → text. Lock serialize model chia sẻ giữa các track."""
    with lock:
        segments, _info = model.transcribe(
            audio,
            language="vi",
            beam_size=beam,
            vad_filter=False,  # VAD đã gate ở tầng ngoài (per-chunk)
            condition_on_previous_text=False,
            initial_prompt=prompt,
        )
        return "".join(seg.text for seg in segments).strip()


class EnergyVad:
    """Speech nếu RMS(chunk) vượt ngưỡng dBFS. Zero-dep — fallback khi webrtcvad không cài được."""

    def __init__(self, threshold_dbfs: float = -45.0):
        self.threshold = threshold_dbfs

    def is_speech(self, chunk: np.ndarray) -> bool:
        if len(chunk) == 0:
            return False
        rms = float(np.sqrt(np.mean(chunk.astype(np.float64) ** 2)))
        if rms <= 1e-9:
            return False
        return 20.0 * np.log10(rms) > self.threshold


class WebrtcVad:
    """webrtcvad trên frame 30ms; chunk = speech nếu >=`ratio` số frame là speech. Pure C ~50KB."""

    def __init__(self, aggressiveness: int = 2, ratio: float = 0.3):
        import webrtcvad  # ImportError → make_vad fallback energy

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
    """Trả (vad, tên_thực_dùng). webrtc → fallback energy nếu import/khởi tạo lỗi."""
    if kind == "webrtc":
        try:
            return WebrtcVad(aggressiveness), "webrtc"
        except Exception:
            return EnergyVad(threshold_dbfs), "energy(webrtc-unavailable)"
    return EnergyVad(threshold_dbfs), "energy"


class StreamSession:
    """Gom chunk khi VAD=speech; im lặng quá `hangover` chunk → transcribe cả utterance, emit final.

    Max-utterance cap: nói dài không nghỉ → force-cut mỗi `max_utterance_s` để bound latency caption
    (không chờ hết câu mới có chữ). Whisper có overhead encoder ~cố định/lần gọi (cửa sổ 30s) → cắt
    quá vụn làm tổng compute nổ (spike: cap 5s → rtf_mean 0.5→3.1) → cap ~8s cân bằng latency↔overhead.

    Nhận PCM chunk kích thước tùy ý (frontend gửi ~0.25–0.5s/chunk); mỗi chunk = 1 đơn vị VAD.
    Thời gian audio (t0/t1) đếm nội bộ theo tổng số mẫu đã nhận từ khi track bắt đầu.
    """

    def __init__(
        self,
        model,
        lock,
        *,
        beam: int = 1,
        prompt: str = DEFAULT_PROMPT,
        max_utterance_s: float = 8.0,
        vad_kind: str = "webrtc",
        vad_aggressiveness: int = 2,
        hangover_chunks: int = 2,
    ):
        self.model, self.lock, self.beam, self.prompt = model, lock, beam, prompt
        self.max_utterance_s = max_utterance_s
        self.hangover = hangover_chunks
        self.vad, self.vad_name = make_vad(vad_kind, vad_aggressiveness)
        self._cursor = 0.0     # audio-time (s) tổng đã nhận
        self._buf: list[np.ndarray] = []
        self._start: float | None = None
        self._last_speech_t1 = 0.0
        self._in_speech = False
        self._silence = 0

    def feed_pcm(self, pcm_bytes: bytes) -> list[dict]:
        """Nhận 1 chunk PCM int16 LE → 0..n caption (final). Trả list rỗng nếu chưa hết utterance."""
        audio = np.frombuffer(pcm_bytes, "<i2").astype(np.float32) / 32768.0
        if len(audio) == 0:
            return []
        t0 = self._cursor
        t1 = t0 + len(audio) / SR
        self._cursor = t1
        is_speech = self.vad.is_speech(audio)

        if is_speech:
            if not self._in_speech:
                self._in_speech, self._buf, self._start = True, [], t0
            self._buf.append(audio)
            self._last_speech_t1 = t1
            self._silence = 0
            if self.max_utterance_s > 0 and (self._last_speech_t1 - self._start) >= self.max_utterance_s:
                caps = self._finalize()
                self._in_speech, self._buf, self._start = True, [], t1  # mở utterance mới ngay
                return caps
            return []
        if self._in_speech:
            self._silence += 1
            if self._silence >= self.hangover:
                return self._finalize()
        return []

    def _finalize(self) -> list[dict]:
        audio = np.concatenate(self._buf) if self._buf else np.zeros(0, np.float32)
        start = self._start if self._start is not None else 0.0
        self._in_speech, self._buf, self._silence = False, [], 0
        if len(audio) < int(_MIN_UTTERANCE_S * SR):
            return []
        text = _transcribe_utterance(self.model, audio, self.lock, self.beam, self.prompt)
        if not text:
            return []
        return [{"kind": "final", "text": text, "t0": start, "t1": self._last_speech_t1}]

    def flush(self) -> list[dict]:
        """Track kết thúc (stream_end / rời phòng): chốt nốt utterance đang mở nếu có."""
        return self._finalize() if self._in_speech else []
