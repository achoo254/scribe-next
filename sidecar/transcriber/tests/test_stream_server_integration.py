"""Integration test: chạy THẬT stream_server.run() qua stdio (fake model) — verify protocol wiring
end-to-end (parse lệnh → router → worker → base64 decode → StreamSession → caption ra stdout).

Chỉ WhisperModel được fake (khỏi cần faster-whisper/model 490MB). Đường đi còn lại = production.
Chạy: python tests/test_stream_server_integration.py  (cần numpy).
"""
import base64
import io
import json
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
    def transcribe(self, audio, **_kw):
        return iter([_FakeSeg("xin chào cả nhà")]), None


def _speech(dur_s=0.5, amp=0.1):
    n = int(dur_s * SR)
    t = np.arange(n)
    return (amp * np.sin(2 * np.pi * 220 * t / SR) * 32767).astype("<i2").tobytes()


def _silence(dur_s=0.5):
    return np.zeros(int(dur_s * SR), "<i2").tobytes()


def _pcm_cmd(track, pcm):
    return json.dumps({"cmd": "stream_pcm", "track": track, "pcm": base64.b64encode(pcm).decode()})


def test_end_to_end_protocol_emits_caption():
    # Fake model: _ensure_model gọi load_stream_model → trả FakeModel, không đụng faster-whisper.
    ss.load_stream_model = lambda *a, **k: FakeModel()
    with ss._registry_lock:
        ss._tracks.clear()
    ss._model = None

    lines = [
        json.dumps({"cmd": "stream_start", "track": "spk1", "model_dir": "/fake", "size": "small"}),
        _pcm_cmd("spk1", _speech()),
        _pcm_cmd("spk1", _speech()),
        _pcm_cmd("spk1", _silence()),
        _pcm_cmd("spk1", _silence()),   # hangover → finalize
    ]

    out = io.StringIO()
    real_stdin, real_stdout = sys.stdin, sys.stdout
    sys.stdin = iter(lines)   # run() lặp `for line in sys.stdin` → EOF sau list → return
    sys.stdout = out
    try:
        ss.run()  # router chạy hết input rồi return; worker (daemon) vẫn emit caption sau đó
        # Chờ caption từ worker thread (transcribe fake nhanh, nhưng vẫn async).
        deadline = time.time() + 5.0
        cap = None
        while time.time() < deadline:
            for ln in out.getvalue().splitlines():
                try:
                    m = json.loads(ln)
                except Exception:
                    continue
                if m.get("type") == "caption":
                    cap = m
                    break
            if cap:
                break
            time.sleep(0.02)
    finally:
        sys.stdin, sys.stdout = real_stdin, real_stdout

    assert cap is not None, f"không thấy caption trong stdout:\n{out.getvalue()}"
    assert cap["track"] == "spk1", cap
    assert cap["kind"] == "final", cap
    assert cap["text"] == "xin chào cả nhà", cap
    # ready phải phát trước tiên
    first = json.loads(out.getvalue().splitlines()[0])
    assert first.get("type") == "ready", first


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
