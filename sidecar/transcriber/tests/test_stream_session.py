"""Test StreamSession — state machine vad-cut + max-cap, KHÔNG cần faster-whisper/model thật.

Inject fake model (duck-typed .transcribe) để verify logic cắt utterance, hangover, max-cap, flush,
bỏ utterance ngắn. Chạy: python tests/test_stream_session.py  (chỉ cần numpy).
"""
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from stream_session import StreamSession  # noqa: E402

SR = 16000


class _FakeSeg:
    def __init__(self, text: str):
        self.text = text


class FakeModel:
    """Trả text cố định; đếm số lần transcribe để kiểm số utterance được cắt."""

    def __init__(self, text: str = "xin chào mọi người"):
        self.text = text
        self.calls = 0

    def transcribe(self, audio, **_kw):
        self.calls += 1
        return iter([_FakeSeg(self.text)]), None


class _NoLock:
    def __enter__(self):
        return self

    def __exit__(self, *a):
        return False


def speech(dur_s: float = 0.5, amp: float = 0.1) -> bytes:
    n = int(dur_s * SR)
    t = np.arange(n)
    return (amp * np.sin(2 * np.pi * 220 * t / SR) * 32767).astype("<i2").tobytes()


def silence(dur_s: float = 0.5) -> bytes:
    return np.zeros(int(dur_s * SR), "<i2").tobytes()


def _session(model, **kw):
    return StreamSession(model, _NoLock(), vad_kind="energy", **kw)


def test_silence_no_caption():
    m = FakeModel()
    s = _session(m)
    caps = []
    for _ in range(6):
        caps += s.feed_pcm(silence())
    assert caps == [], caps
    assert m.calls == 0


def test_utterance_then_hangover_emits_final():
    m = FakeModel("xin chào")
    s = _session(m, hangover_chunks=2)
    assert s.feed_pcm(speech()) == []          # bắt đầu nói
    assert s.feed_pcm(speech()) == []          # vẫn nói
    assert s.feed_pcm(silence()) == []         # silence=1 < hangover
    caps = s.feed_pcm(silence())               # silence=2 → finalize
    assert len(caps) == 1, caps
    assert caps[0]["kind"] == "final"
    assert caps[0]["text"] == "xin chào"
    assert caps[0]["t0"] == 0.0
    assert caps[0]["t1"] > 0.0
    assert m.calls == 1


def test_max_utterance_cap_force_cuts():
    m = FakeModel("nói dài")
    s = _session(m, max_utterance_s=1.0, hangover_chunks=2)
    caps = []
    for _ in range(4):  # 4 × 0.5s nói liên tục → phải force-cut ~mỗi 1s
        caps += s.feed_pcm(speech())
    assert len(caps) >= 1, caps
    assert m.calls >= 1


def test_short_utterance_dropped():
    m = FakeModel()
    s = _session(m, hangover_chunks=1)
    s.feed_pcm(speech(dur_s=0.1))   # utterance < 0.2s → bỏ
    caps = s.feed_pcm(silence())    # hangover → finalize nhưng audio quá ngắn
    assert caps == [], caps
    assert m.calls == 0


def test_flush_finalizes_open_utterance():
    m = FakeModel("chưa xong")
    s = _session(m)
    s.feed_pcm(speech())
    s.feed_pcm(speech())
    caps = s.flush()
    assert len(caps) == 1, caps
    assert caps[0]["text"] == "chưa xong"


def test_empty_pcm_ignored():
    s = _session(FakeModel())
    assert s.feed_pcm(b"") == []


def _run_all():
    fns = [v for k, v in sorted(globals().items()) if k.startswith("test_") and callable(v)]
    failed = 0
    for fn in fns:
        try:
            fn()
            print(f"PASS {fn.__name__}")
        except AssertionError as e:
            failed += 1
            print(f"FAIL {fn.__name__}: {e}")
        except Exception as e:  # noqa: BLE001
            failed += 1
            print(f"ERROR {fn.__name__}: {type(e).__name__}: {e}")
    print(f"\n{len(fns) - failed}/{len(fns)} passed")
    return failed


if __name__ == "__main__":
    sys.exit(1 if _run_all() else 0)
