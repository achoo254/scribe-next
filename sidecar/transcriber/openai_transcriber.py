"""Nguồn chép OpenAI (gpt-4o-mini-transcribe) cho Mode C — trả segments cùng shape với backend local.

- Cắt audio tại khoảng lặng (silero VAD của faster-whisper) thành mảnh ≤ CHUNK_MAX giây. Mảnh dài
  (~2 phút) làm model lặp nguyên khối câu và chạm trần 2048 token output; mảnh ~40s thì sạch.
- Mỗi mảnh nén Opus trong bộ nhớ (PyAV) rồi gửi multipart bằng urllib (không thêm dependency).
- Mảnh có dấu hiệu lặp → gọi lại với temperature cao hơn (RETRY_TEMPS).
- Model không trả timestamp → mốc từng từ là ƯỚC LƯỢNG: rải đều số từ theo các vùng có tiếng nói
  (VAD) trong mảnh. Đủ cho resegment phụ đề + click-to-seek gần đúng, sai số trong phạm vi 1 mảnh.
"""
import io
import json
import time
import urllib.error
import urllib.request
import uuid
from collections import Counter
from concurrent.futures import ThreadPoolExecutor

SAMPLE_RATE = 16000
MODEL = "gpt-4o-mini-transcribe"
API_URL = "https://api.openai.com/v1/audio/transcriptions"
CHUNK_TARGET = 40.0   # giây — điểm cắt ưu tiên quanh mốc này
CHUNK_MIN = 25.0      # không cắt sớm hơn (tránh mảnh vụn)
CHUNK_MAX = 45.0      # không có khoảng lặng trong [MIN, MAX] → cắt cứng ở TARGET
RETRY_TEMPS = ("0", "0.4", "0.8")
MAX_WORDS_PER_MIN = 260  # nhanh hơn mức này = bịa/lặp (người nói nhanh ~200 từ/phút)
PARALLEL = 4
HTTP_RETRIES = 3


def plan_chunks(speech, duration):
    """speech: [(start_s, end_s)] vùng có tiếng nói (đã sort). Trả [(a, z)] phủ kín [0, duration],
    cắt ở GIỮA khoảng lặng dài nhất trong cửa sổ [a+MIN, a+MAX]."""
    gaps = [((e1 + s2) / 2, s2 - e1) for (_, e1), (s2, _) in zip(speech, speech[1:]) if s2 > e1]
    cuts = [0.0]
    while duration - cuts[-1] > CHUNK_MAX:
        a = cuts[-1]
        window = [g for g in gaps if a + CHUNK_MIN <= g[0] <= a + CHUNK_MAX]
        cuts.append(max(window, key=lambda g: g[1])[0] if window else a + CHUNK_TARGET)
    cuts.append(duration)
    return list(zip(cuts, cuts[1:]))


def looks_looped(text: str, seconds: float) -> bool:
    words = text.lower().split()
    grams = Counter(tuple(words[i:i + 8]) for i in range(len(words) - 7))
    repeated = sum(c - 1 for c in grams.values() if c > 1)
    wpm = len(words) / max(seconds / 60, 1e-6)
    return repeated > 3 or wpm > MAX_WORDS_PER_MIN


def estimate_words(text: str, a: float, z: float, speech):
    """Rải từ của 1 mảnh đều theo thời gian CÓ TIẾNG NÓI giao với [a, z): từ thứ i chiếm đoạn
    [i*step, (i+1)*step) trên trục thời gian-nói, rồi quy đổi về giờ thực theo từng vùng nói."""
    words = text.split()
    spans = [(max(s, a), min(e, z)) for s, e in speech if e > a and s < z]
    spans = [sp for sp in spans if sp[1] > sp[0]] or [(a, z)]
    step = sum(e - s for s, e in spans) / max(len(words), 1)

    def to_clock(t: float, is_end: bool) -> float:
        # Mốc đúng tại điểm nối 2 vùng: start thuộc vùng SAU, end thuộc vùng TRƯỚC.
        for s, e in spans:
            if t < e - s or (is_end and t <= e - s):
                return s + t
            t -= e - s
        return spans[-1][1]

    return [
        {"start": to_clock(i * step, False), "end": to_clock((i + 1) * step, True), "word": " " + w}
        for i, w in enumerate(words)
    ]


def _encode_opus(pcm) -> bytes:
    import av

    buf = io.BytesIO()
    with av.open(buf, "w", format="ogg") as container:
        stream = container.add_stream("libopus", rate=SAMPLE_RATE)
        stream.layout = "mono"
        stream.bit_rate = 32000
        frame = av.AudioFrame.from_ndarray(pcm.reshape(1, -1), format="flt", layout="mono")
        frame.sample_rate = SAMPLE_RATE
        for packet in stream.encode(frame):
            container.mux(packet)
        for packet in stream.encode(None):
            container.mux(packet)
    return buf.getvalue()


def _api_error(code: int, body: str) -> str:
    try:
        err = json.loads(body).get("error", {})
    except ValueError:
        err = {}
    if code == 401:
        return "OpenAI: API key không hợp lệ hoặc đã bị thu hồi"
    if err.get("code") in ("insufficient_quota", "credit_balance_exhausted") or err.get("type") == "insufficient_quota":
        return "OpenAI: tài khoản đã hết credit — nạp thêm tại platform.openai.com/settings/organization/billing"
    return f"OpenAI HTTP {code}: {err.get('message') or body[:200]}"


def _post(audio: bytes, api_key: str, temperature: str, prompt) -> str:
    boundary = uuid.uuid4().hex
    fields = {"model": MODEL, "language": "vi", "response_format": "json", "temperature": temperature}
    if prompt:
        fields["prompt"] = prompt
    body = b"".join(
        f'--{boundary}\r\nContent-Disposition: form-data; name="{k}"\r\n\r\n{v}\r\n'.encode()
        for k, v in fields.items()
    )
    body += (
        f'--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="chunk.ogg"\r\n'
        "Content-Type: audio/ogg\r\n\r\n"
    ).encode() + audio + f"\r\n--{boundary}--\r\n".encode()
    req = urllib.request.Request(API_URL, data=body, headers={
        "Authorization": f"Bearer {api_key}",
        "Content-Type": f"multipart/form-data; boundary={boundary}",
    })
    for attempt in range(HTTP_RETRIES):
        try:
            with urllib.request.urlopen(req, timeout=180) as resp:
                return (json.load(resp).get("text") or "").strip()
        except urllib.error.HTTPError as e:
            detail = e.read().decode("utf-8", "replace")
            quota = "insufficient_quota" in detail or "credit_balance_exhausted" in detail
            if e.code in (429, 500, 502, 503) and not quota and attempt < HTTP_RETRIES - 1:
                time.sleep(3 * (attempt + 1))  # rate limit / lỗi tạm thời phía server
                continue
            raise RuntimeError(_api_error(e.code, detail)) from None
        except urllib.error.URLError as e:
            if attempt < HTTP_RETRIES - 1:
                time.sleep(3 * (attempt + 1))
                continue
            raise RuntimeError(f"OpenAI: không kết nối được ({e.reason})") from None
    raise RuntimeError("OpenAI: hết số lần thử")


def _transcribe_chunk(pcm, a, z, api_key, prompt) -> str:
    audio = _encode_opus(pcm[int(a * SAMPLE_RATE):int(z * SAMPLE_RATE)])
    text = ""
    for temperature in RETRY_TEMPS:
        text = _post(audio, api_key, temperature, prompt)
        if not looks_looped(text, z - a):
            break
    return text


def transcribe(file: str, api_key: str, prompt, on_progress):
    """Trả (text, segments) — segments 1-1 với mảnh, words có mốc ước lượng. on_progress(pct)."""
    from faster_whisper.audio import decode_audio
    from faster_whisper.vad import VadOptions, get_speech_timestamps

    pcm = decode_audio(file, sampling_rate=SAMPLE_RATE)
    duration = len(pcm) / SAMPLE_RATE
    speech = [
        (t["start"] / SAMPLE_RATE, t["end"] / SAMPLE_RATE)
        for t in get_speech_timestamps(pcm, VadOptions())
    ]
    # Chỉ gửi mảnh có tiếng nói — mảnh im lặng hoàn toàn dễ bị bịa chữ + tốn tiền vô ích.
    chunks = [(a, z) for a, z in plan_chunks(speech, duration) if any(e > a and s < z for s, e in speech)]
    texts = [""] * len(chunks)
    done = 0
    pool = ThreadPoolExecutor(max_workers=PARALLEL)
    try:
        futures = [pool.submit(_transcribe_chunk, pcm, a, z, api_key, prompt) for a, z in chunks]
        for i, fut in enumerate(futures):
            texts[i] = fut.result()  # lỗi 1 mảnh → raise → cả file báo lỗi (main gắn "file")
            done += 1
            on_progress(min(99, int(done / max(len(chunks), 1) * 100)))
    finally:
        # Lỗi (key sai / hết credit) → huỷ các mảnh chưa chạy, khỏi gọi API tốn tiền vô ích.
        pool.shutdown(wait=True, cancel_futures=True)
    segments = [
        {"start": a, "end": z, "text": " " + t, "words": estimate_words(t, a, z, speech)}
        for (a, z), t in zip(chunks, texts) if t
    ]
    return " ".join(t for t in texts if t).strip(), segments
