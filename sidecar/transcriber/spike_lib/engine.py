"""Lõi ASR streaming cho spike: WhisperModel trần (KHÔNG BatchedInferencePipeline) + 2 chiến lược.

[C1] Dùng `WhisperModel(device="cpu", compute_type="int8")` trực tiếp — BatchedInferencePipeline
chỉ hợp batch file, sai mô hình streaming và đo RTF lệch.
[C3] LocalAgreement-2 tự implement (không cần whisper_online.py UFAL, không PyPI).

Caption record: {"kind":"partial"|"final","text","t0","t1"}.
Run record (đo tải): {"compute": giây, "audio_dur": giây}.
strategy.feed(...) -> (captions:list, runs:list). flush(t) -> (captions, runs).
"""
import time

import numpy as np

SR = 16000

# Glossary domain (IT/DevOps) bias tên riêng — đồng bộ với main.py DEFAULT_PROMPT.
DEFAULT_PROMPT = (
    "Bài nói kỹ thuật tiếng Việt về hệ thống và DevOps. Thuật ngữ thường gặp: "
    "VictoriaLogs, VictoriaMetrics, Grafana, OpenTelemetry, observability, log, parse, "
    "JSON, query, cheatsheet, cluster, HA, Docker, server, port, IP, API, systemctl, rsyslog."
)


def load_model(model_dir: str, cpu_threads: int, compute_type: str = "int8"):
    from faster_whisper import WhisperModel

    return WhisperModel(
        model_dir, device="cpu", compute_type=compute_type, cpu_threads=cpu_threads
    )


def transcribe_words(model, audio: np.ndarray, lock, beam: int, prompt: str):
    """Transcribe 1 đoạn float32 → (words[(text,start,end)], compute_time).

    Timer đặt TRONG lock để compute KHÔNG gồm thời gian chờ lock (shared model) → RTF đo đúng
    là thời gian tính toán thuần. Nút thắt serialize đã phản ánh ở wall_s/total_audio_s.
    """
    with lock:
        t = time.perf_counter()
        segments, _info = model.transcribe(
            audio,
            language="vi",
            beam_size=beam,
            word_timestamps=True,
            vad_filter=False,            # VAD đã gate ở tầng ngoài
            condition_on_previous_text=False,
            initial_prompt=prompt,
        )
        words = []
        for seg in segments:  # iterate = nơi compute thực sự xảy ra
            if seg.words:
                words += [(w.word, w.start, w.end) for w in seg.words]
            elif seg.text.strip():
                words.append((" " + seg.text.strip(), seg.start, seg.end))  # space tránh dính câu
        compute = time.perf_counter() - t
    return words, compute


def _norm(w):
    return w[0].strip().lower()


def _join(words):
    return "".join(w[0] for w in words).strip()


class VadCutStrategy:
    """Gom chunk khi VAD=speech; im lặng quá hangover → transcribe cả utterance, emit final.

    Max-utterance cap: người nói dài không nghỉ (no pause) → force-cut mỗi `max_utterance_s` để
    bound latency caption (không chờ tới hết câu mới có chữ). Cùng tinh thần với hard-cap của
    LocalAgreementStrategy. max_utterance_s<=0 = tắt cap (chỉ cắt theo silence).
    """

    def __init__(self, model, lock, beam: int, prompt: str, max_utterance_s: float = 5.0):
        self.model, self.lock, self.beam, self.prompt = model, lock, beam, prompt
        self.max_utterance_s = max_utterance_s
        self.buf = []
        self.start = None
        self.last_speech_t1 = 0.0
        self.in_speech = False
        self.silence = 0

    def feed(self, chunk, t0, t1, is_speech, hangover):
        if is_speech:
            if not self.in_speech:
                self.in_speech, self.buf, self.start = True, [], t0
            self.buf.append(chunk)
            self.last_speech_t1 = t1
            self.silence = 0
            # Force-cut khi utterance dài quá cap (người nói liên tục không nghỉ) → bound latency.
            if self.max_utterance_s > 0 and (self.last_speech_t1 - self.start) >= self.max_utterance_s:
                caps, runs = self._finalize()
                self.in_speech, self.buf, self.start = True, [], t1  # mở utterance mới ngay
                return caps, runs
            return [], []
        if self.in_speech:
            self.silence += 1
            if self.silence >= hangover:
                return self._finalize()
        return [], []

    def _finalize(self):
        audio = np.concatenate(self.buf) if self.buf else np.zeros(0, np.float32)
        self.in_speech, self.buf = False, []
        if len(audio) < int(0.2 * SR):  # bỏ utterance quá ngắn
            return [], []
        words, compute = transcribe_words(self.model, audio, self.lock, self.beam, self.prompt)
        text = _join(words)
        runs = [{"compute": compute, "audio_dur": len(audio) / SR}]
        if not text:
            return [], runs
        return [{"kind": "final", "text": text, "t0": self.start, "t1": self.last_speech_t1}], runs

    def flush(self, _t):
        return self._finalize() if self.in_speech else ([], [])


class LocalAgreementStrategy:
    """LocalAgreement-2: chạy ASR lặp trên buffer trượt; commit prefix khớp 2 lần liên tiếp = final."""

    def __init__(self, model, lock, beam: int, prompt: str, commit_ms: int = 1000,
                 max_buffer_s: float = 18.0, keep_context_s: float = 1.0):
        self.model, self.lock, self.beam, self.prompt = model, lock, beam, prompt
        self.commit_samples = int(commit_ms / 1000 * SR)
        self.max_buffer_s, self.keep_context_s = max_buffer_s, keep_context_s
        self.buf = np.zeros(0, np.float32)
        self.buf_start = None          # audio-time (s) của buf[0]
        self.prev_hyp = []             # hypothesis chưa-commit lần trước
        self.last_commit_end = 0.0
        self.since_run = 0
        self.silence = 0

    def feed(self, chunk, t0, t1, is_speech, hangover):
        if self.buf_start is None:
            self.buf_start = t0
        self.buf = np.concatenate([self.buf, chunk])
        self.since_run += len(chunk)
        self.silence = 0 if is_speech else self.silence + 1
        caps, runs = [], []
        # Flush theo utterance: im lặng quá hangover HOẶC buffer chạm hard-cap (chống phình vô hạn
        # khi không có commit nào — trim theo commit không đủ). Cả hai đều bound buffer.
        long_enough = len(self.buf) > int(0.2 * SR)
        too_long = len(self.buf) / SR > self.max_buffer_s
        if (self.silence >= hangover and long_enough) or too_long:
            c, r = self._run(final=True)
            self._reset()
            return c, r
        if self.since_run >= self.commit_samples:
            self.since_run = 0
            c, r = self._run(final=False)
            caps += c
            runs += r
        return caps, runs

    def _reset(self):
        self.buf = np.zeros(0, np.float32)
        self.buf_start = None
        self.prev_hyp = []
        self.since_run = 0
        self.silence = 0

    def _run(self, final: bool):
        if len(self.buf) < int(0.2 * SR):
            return [], []
        words, compute = transcribe_words(self.model, self.buf, self.lock, self.beam, self.prompt)
        words = [(w, s + self.buf_start, e + self.buf_start) for (w, s, e) in words]
        unc = [w for w in words if w[2] > self.last_commit_end + 1e-3]
        runs = [{"compute": compute, "audio_dur": len(self.buf) / SR}]
        caps = []
        if final:
            if unc:
                self.last_commit_end = unc[-1][2]
                caps.append({"kind": "final", "text": _join(unc),
                             "t0": unc[0][1], "t1": unc[-1][2]})
            self.prev_hyp = []
            return caps, runs
        agreed = self._lcp(unc, self.prev_hyp)
        if agreed:
            self.last_commit_end = agreed[-1][2]
            caps.append({"kind": "final", "text": _join(agreed),
                         "t0": agreed[0][1], "t1": agreed[-1][2]})
        self.prev_hyp = unc[len(agreed):]
        if self.prev_hyp:
            caps.append({"kind": "partial", "text": _join(self.prev_hyp),
                         "t0": self.prev_hyp[0][1], "t1": self.prev_hyp[-1][2]})
        self._trim()
        return caps, runs

    @staticmethod
    def _lcp(a, b):
        out = []
        for x, y in zip(a, b):
            if _norm(x) == _norm(y):
                out.append(x)
            else:
                break
        return out

    def _trim(self):
        if self.last_commit_end - self.buf_start > self.max_buffer_s:
            keep_from = self.last_commit_end - self.keep_context_s
            cut = int((keep_from - self.buf_start) * SR)
            if cut > 0:
                self.buf = self.buf[cut:]
                self.buf_start = keep_from

    def flush(self, _t):
        return self._run(final=True)
