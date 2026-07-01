"""Test stream_server — routing, per-track worker, backpressure drop, sanitize, cô lập C2.

Inject fake model + capture _send. Verify: router KHÔNG bị ASR chậm chặn (C2 nội-process), track id
sanitize, backpressure drop khi queue đầy, caption phát đúng track. Chạy: python tests/test_stream_server.py
"""
import base64
import os
import sys
import time

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
import stream_server as ss  # noqa: E402

SR = 16000


class _FakeSeg:
    def __init__(self, text):
        self.text = text


class FakeModel:
    def __init__(self, text="xin chào", delay=0.0):
        self.text, self.delay = text, delay

    def transcribe(self, audio, **_kw):
        if self.delay:
            time.sleep(self.delay)
        return iter([_FakeSeg(self.text)]), None


def _speech_b64(dur_s=0.5, amp=0.1):
    n = int(dur_s * SR)
    t = np.arange(n)
    pcm = (amp * np.sin(2 * np.pi * 220 * t / SR) * 32767).astype("<i2").tobytes()
    return base64.b64encode(pcm).decode()


def _silence_b64(dur_s=0.5):
    return base64.b64encode(np.zeros(int(dur_s * SR), "<i2").tobytes()).decode()


def _reset(model):
    """Đưa module về state sạch + inject fake model + capture _send (thread-safe qua GIL append)."""
    with ss._registry_lock:
        ss._tracks.clear()
    ss._model = model
    cap = []
    ss._send = lambda o: cap.append(o)  # StreamSession dùng vad_kind mặc định webrtc→fallback energy OK
    return cap


def _wait(pred, timeout=5.0):
    end = time.time() + timeout
    while time.time() < end:
        if pred():
            return True
        time.sleep(0.01)
    return False


def test_caption_flows_for_track():
    cap = _reset(FakeModel("xin chào"))
    ss._stream_start("trackA", "/fake", "small")
    for _ in range(2):
        ss._stream_pcm("trackA", _speech_b64())
    for _ in range(2):
        ss._stream_pcm("trackA", _silence_b64())  # hangover → finalize
    assert _wait(lambda: any(m.get("type") == "caption" for m in cap)), cap
    c = next(m for m in cap if m.get("type") == "caption")
    assert c["track"] == "trackA" and c["text"] == "xin chào", c
    ss._stream_end("trackA")


def test_router_not_blocked_by_slow_asr():
    """C2 (nội-process): _stream_pcm chỉ enqueue → trả ngay dù ASR (worker) chậm 0.3s/utterance."""
    _reset(FakeModel("chậm", delay=0.3))
    ss._stream_start("slow", "/fake", "small")
    t0 = time.perf_counter()
    for _ in range(12):
        ss._stream_pcm("slow", _speech_b64(dur_s=0.3))
    elapsed = time.perf_counter() - t0
    assert elapsed < 0.5, f"router bị chặn: {elapsed:.2f}s cho 12 push (ASR 0.3s/lần)"
    ss._stream_end("slow")


def test_invalid_track_id_rejected():
    cap = _reset(FakeModel())
    ss._stream_start("bad id!", "/fake", "small")
    assert "bad id!" not in ss._tracks
    assert any(m.get("type") == "stream_error" for m in cap), cap


def test_unknown_track_pcm_ignored():
    _reset(FakeModel())
    ss._stream_pcm("ghost", _speech_b64())  # chưa stream_start → bỏ qua, không dựng ngầm, không crash
    assert "ghost" not in ss._tracks


def test_unknown_size_rejected_no_crash():
    # M2: size lạ → stream_error + KHÔNG dựng track (tránh KeyError giết router → mất caption mọi track).
    cap = _reset(FakeModel())
    ss._stream_start("t1", "/fake", "gigantic")
    assert "t1" not in ss._tracks
    assert any(m.get("type") == "stream_error" for m in cap), cap


def test_backpressure_drop_when_queue_full():
    cap = _reset(FakeModel())
    t = ss._Track("bp", "/fake")  # KHÔNG start thread → queue không được rút → đầy
    for _ in range(ss._QUEUE_MAX + 30):
        t.push(b"\x00\x00")
    assert t.dropped >= 30, t.dropped
    assert any(m.get("type") == "backpressure" for m in cap), cap


def test_stop_when_queue_full_still_sets_sentinel():
    _reset(FakeModel())
    t = ss._Track("full", "/fake")
    for _ in range(ss._QUEUE_MAX):
        t.push(b"\x00\x00")
    t.stop()  # queue đầy nhưng vẫn đặt được sentinel (xả 1 slot) → worker sẽ dừng
    assert t.q.qsize() <= ss._QUEUE_MAX


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
