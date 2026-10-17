"""Gọi API chép lời của OpenAI, ElevenLabs và AssemblyAI theo hình dạng API chính thức (kiểm 17/10/2026).

Key chỉ nằm trong bộ nhớ: đọc từ tệp `$ACCESS_TOKENS_JSON`, đưa vào header, không in, không ghi ra đĩa, không
đưa vào thông báo lỗi.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import time
import urllib.error
import urllib.request
import uuid
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field
from typing import Any

from metrics import looks_looped

# Giá niêm yết, USD cho một giờ audio (kiểm 17/10/2026; nguồn trong plan 261017-1355-stt-provider-benchmark).
PRICE_PER_HOUR = {
    "gpt-4o-mini-transcribe": 0.18,
    "gpt-transcribe": 0.27,
    "scribe_v2": 0.22,
    "universal-3-5-pro": 0.21,
    "universal-2": 0.15,
}
ASSEMBLYAI_DIARIZATION_PER_HOUR = 0.02
# `gpt-4o-mini-transcribe` trả `usage` theo token: USD cho 1 triệu token vào (audio) và ra (chữ).
MINI_TOKEN_PRICE = {"input": 1.25, "output": 5.00}

# Mảnh cho OpenAI, giống `server/scribe-be/src/transcribe/chunking.rs`.
CHUNK_MIN, CHUNK_TARGET, CHUNK_MAX = 25.0, 40.0, 45.0
SILENT_PEAK_DB = -30.0
OPENAI_PARALLEL = 4
HTTP_TIMEOUT_S = 600


class ProviderError(Exception):
    """Lỗi gọi nhà cung cấp; thông báo không bao giờ chứa key."""


@dataclass
class Result:
    """Kết quả một nhà cung cấp trên một tệp."""

    text: str
    raw: Any
    billed_seconds: float
    cost_usd: float
    wall_seconds: float
    turns: list[dict[str, Any]] = field(default_factory=list)
    extra: dict[str, Any] = field(default_factory=dict)


def load_key(service: str) -> str | None:
    """Token của `service` trong `$ACCESS_TOKENS_JSON` (`.tokens[] | select(.service == …) | .token`)."""
    path = os.environ.get("ACCESS_TOKENS_JSON")
    if not path or not os.path.isfile(path):
        return None
    try:
        with open(path, encoding="utf-8") as fh:
            tokens = json.load(fh).get("tokens", [])
    except (OSError, ValueError):
        return None
    for entry in tokens:
        if isinstance(entry, dict) and entry.get("service") == service and entry.get("token"):
            return str(entry["token"])
    return None


# ---------------------------------------------------------------- HTTP


def _multipart(fields: list[tuple[str, str]], file_field: str, filename: str, data: bytes, mime: str) -> tuple[bytes, str]:
    boundary = uuid.uuid4().hex
    parts = []
    for name, value in fields:
        parts.append(f'--{boundary}\r\nContent-Disposition: form-data; name="{name}"\r\n\r\n{value}\r\n'.encode())
    parts.append(
        f'--{boundary}\r\nContent-Disposition: form-data; name="{file_field}"; filename="{filename}"\r\n'
        f"Content-Type: {mime}\r\n\r\n".encode()
    )
    parts.append(data)
    parts.append(f"\r\n--{boundary}--\r\n".encode())
    return b"".join(parts), f"multipart/form-data; boundary={boundary}"


def _request(method: str, url: str, headers: dict[str, str], body: bytes | None = None, retries: int = 3) -> Any:
    """Gửi request, thử lại khi 429/5xx/lỗi mạng; trả JSON. Lỗi chỉ nêu mã HTTP và đoạn đầu của body."""
    for attempt in range(1, retries + 1):
        req = urllib.request.Request(url, data=body, headers=headers, method=method)
        try:
            with urllib.request.urlopen(req, timeout=HTTP_TIMEOUT_S) as resp:
                return json.loads(resp.read().decode("utf-8"))
        except urllib.error.HTTPError as err:
            detail = err.read().decode("utf-8", "replace")[:300]
            if err.code in (429, 500, 502, 503, 504) and attempt < retries and "insufficient_quota" not in detail:
                time.sleep(3 * attempt)
                continue
            raise ProviderError(f"HTTP {err.code}: {detail}") from None
        except (urllib.error.URLError, TimeoutError) as err:
            if attempt < retries:
                time.sleep(3 * attempt)
                continue
            raise ProviderError(f"lỗi mạng: {type(err).__name__}") from None
        except ValueError:
            raise ProviderError("phản hồi không phải JSON") from None
    raise ProviderError("hết số lần thử")


# ---------------------------------------------------------------- ffmpeg


def run_ffmpeg(args: list[str]) -> subprocess.CompletedProcess[bytes]:
    proc = subprocess.run(["ffmpeg", "-hide_banner", "-nostats", *args], capture_output=True, check=False)
    if proc.returncode != 0:
        tail = proc.stderr.decode("utf-8", "replace").strip().splitlines()[-1:] or [""]
        raise ProviderError(f"ffmpeg lỗi: {tail[0]}")
    return proc


def probe_duration(path: str) -> float:
    proc = subprocess.run(
        ["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", path],
        capture_output=True, check=False, text=True,
    )
    try:
        return float(proc.stdout.strip())
    except ValueError:
        raise ProviderError(f"ffprobe không đọc được thời lượng của {os.path.basename(path)}") from None


def plan_chunks(path: str, duration: float) -> list[tuple[float, float]]:
    """Mảnh 25–45 giây cắt ở giữa khoảng lặng dài nhất (silencedetect −35 dB, 0,4 s) như backend."""
    stderr = run_ffmpeg(["-i", path, "-vn", "-af", "silencedetect=noise=-35dB:d=0.4", "-f", "null", "-"]).stderr.decode(
        "utf-8", "replace"
    )
    starts = [float(x) for x in re.findall(r"silence_start: (-?[\d.]+)", stderr)]
    ends = [float(x) for x in re.findall(r"silence_end: (-?[\d.]+)", stderr)]
    silences = [(max(s, 0.0), e) for s, e in zip(starts, ends + [duration] * (len(starts) - len(ends)))]
    gaps = [((s + e) / 2, e - s) for s, e in silences]
    cuts = [0.0]
    while duration - cuts[-1] > CHUNK_MAX:
        a = cuts[-1]
        window = [g for g in gaps if a + CHUNK_MIN <= g[0] <= a + CHUNK_MAX]
        cuts.append(max(window, key=lambda g: g[1])[0] if window else a + CHUNK_TARGET)
    cuts.append(duration)
    return list(zip(cuts, cuts[1:]))


def encode_chunk(path: str, start: float, length: float) -> tuple[bytes, float | None]:
    """Mảnh → Opus mono 16 kHz 32 kbps (ogg) trong bộ nhớ, kèm âm lượng đỉnh (dB)."""
    proc = run_ffmpeg([
        "-loglevel", "info", "-ss", f"{start:.3f}", "-t", f"{length:.3f}", "-i", path, "-vn",
        "-af", "volumedetect", "-ac", "1", "-ar", "16000", "-c:a", "libopus", "-b:a", "32k", "-f", "ogg", "pipe:1",
    ])
    match = re.search(r"max_volume: (-?[\d.]+) dB", proc.stderr.decode("utf-8", "replace"))
    return proc.stdout, float(match.group(1)) if match else None


# ---------------------------------------------------------------- OpenAI


def openai_transcribe(key: str, model: str, audio_path: str) -> Result:
    """Cắt mảnh như backend, bỏ mảnh im lặng, gửi song song 4 mảnh; một lần gửi mỗi mảnh (không thử lại khi lặp)."""
    duration = probe_duration(audio_path)
    chunks = plan_chunks(audio_path, duration)
    started = time.monotonic()

    def one(chunk: tuple[float, float]) -> dict[str, Any]:
        start, end = chunk
        audio, peak = encode_chunk(audio_path, start, end - start)
        if peak is not None and peak < SILENT_PEAK_DB:
            return {"start": start, "end": end, "skipped": "silent", "text": "", "seconds": 0.0, "cost": 0.0}
        fields = [("model", model), ("response_format", "json")]
        if model == "gpt-transcribe":
            fields.append(("languages[]", "vi"))  # `languages` thay `language` ở model mới
        else:
            fields += [("language", "vi"), ("temperature", "0")]
        body, ctype = _multipart(fields, "file", "chunk.ogg", audio, "audio/ogg")
        t0 = time.monotonic()
        data = _request("POST", "https://api.openai.com/v1/audio/transcriptions",
                        {"Authorization": f"Bearer {key}", "Content-Type": ctype}, body)
        usage = data.get("usage") or {}
        seconds = float(usage["seconds"]) if usage.get("type") == "duration" and "seconds" in usage else end - start
        if usage.get("type") == "tokens":
            cost = (usage.get("input_tokens", 0) * MINI_TOKEN_PRICE["input"]
                    + usage.get("output_tokens", 0) * MINI_TOKEN_PRICE["output"]) / 1e6
        else:
            cost = seconds / 3600 * PRICE_PER_HOUR[model]
        text = str(data.get("text", "")).strip()
        return {"start": start, "end": end, "text": text, "seconds": seconds, "cost": cost, "usage": usage,
                "latency": time.monotonic() - t0, "looped": looks_looped(text, end - start)}

    with ThreadPoolExecutor(OPENAI_PARALLEL) as pool:
        parts = list(pool.map(one, chunks))
    wall = time.monotonic() - started
    sent = [p for p in parts if not p.get("skipped")]
    return Result(
        text="\n".join(p["text"] for p in parts if p["text"]),
        raw={"model": model, "chunks": parts},
        billed_seconds=sum(p["seconds"] for p in parts),
        cost_usd=sum(p["cost"] for p in parts),
        wall_seconds=wall,
        extra={
            "chunks": len(parts),
            "silentChunks": len(parts) - len(sent),
            "loopedChunks": sum(1 for p in sent if p["looped"]),
            "chunkLatencyMedian": sorted(p["latency"] for p in sent)[len(sent) // 2] if sent else None,
        },
    )


# ---------------------------------------------------------------- ElevenLabs


def _turns_from_words(words: list[dict[str, Any]], speaker_key: str, scale: float = 1.0) -> list[dict[str, Any]]:
    """Gộp các từ liên tiếp cùng người nói thành lượt nói `{speaker, start, end, text}` (giây)."""
    turns: list[dict[str, Any]] = []
    for w in words:
        speaker = w.get(speaker_key)
        if speaker is None or w.get("start") is None:
            continue
        start, end = float(w["start"]) * scale, float(w.get("end") or w["start"]) * scale
        if turns and turns[-1]["speaker"] == speaker:
            turns[-1]["end"] = end
            turns[-1]["text"] += " " + str(w.get("text", "")).strip()
        else:
            turns.append({"speaker": str(speaker), "start": start, "end": end, "text": str(w.get("text", "")).strip()})
    return turns


def elevenlabs_transcribe(key: str, audio_path: str) -> Result:
    """Cả tệp trong một request: `scribe_v2`, `language_code=vi`, `diarize=true`, timestamp theo từ."""
    with open(audio_path, "rb") as fh:
        audio = fh.read()
    fields = [("model_id", "scribe_v2"), ("language_code", "vi"), ("diarize", "true"),
              ("timestamps_granularity", "word"), ("tag_audio_events", "false")]
    body, ctype = _multipart(fields, "file", os.path.basename(audio_path), audio, "application/octet-stream")
    started = time.monotonic()
    data = _request("POST", "https://api.elevenlabs.io/v1/speech-to-text",
                    {"xi-api-key": key, "Content-Type": ctype}, body)
    wall = time.monotonic() - started
    words = [w for w in data.get("words", []) if w.get("type") == "word"]
    turns = _turns_from_words(words, "speaker_id")
    billed = float(data.get("audio_duration_secs") or probe_duration(audio_path))
    return Result(
        text=str(data.get("text", "")),
        raw=data,
        billed_seconds=billed,
        cost_usd=billed / 3600 * PRICE_PER_HOUR["scribe_v2"],
        wall_seconds=wall,
        turns=turns,
        extra={"speakers": len({t["speaker"] for t in turns})},
    )


# ---------------------------------------------------------------- AssemblyAI


def assemblyai_transcribe(key: str, audio_path: str, poll_every: float = 5.0, max_wait: float = 3600.0) -> Result:
    """Tải tệp lên, tạo transcript (`universal-3-5-pro` rồi `universal-2`, `vi`, `speaker_labels`), chờ xong."""
    base = "https://api.assemblyai.com/v2"
    with open(audio_path, "rb") as fh:
        audio = fh.read()
    started = time.monotonic()
    upload = _request("POST", f"{base}/upload",
                      {"authorization": key, "Content-Type": "application/octet-stream"}, audio)
    request = {
        "audio_url": upload["upload_url"],
        "speech_models": ["universal-3-5-pro", "universal-2"],
        "language_code": "vi",
        "speaker_labels": True,
        "punctuate": True,
        "format_text": True,
    }
    job = _request("POST", f"{base}/transcript",
                   {"authorization": key, "Content-Type": "application/json"}, json.dumps(request).encode())
    data = job
    while data.get("status") not in ("completed", "error"):
        if time.monotonic() - started > max_wait:
            raise ProviderError(f"AssemblyAI quá {max_wait:.0f} giây chưa xong (id {job.get('id')})")
        time.sleep(poll_every)
        data = _request("GET", f"{base}/transcript/{job['id']}", {"authorization": key})
    wall = time.monotonic() - started
    if data["status"] == "error":
        raise ProviderError(f"AssemblyAI: {str(data.get('error'))[:300]}")
    model = str(data.get("speech_model_used") or data.get("speech_model") or "universal-3-5-pro")
    turns = [{"speaker": str(u.get("speaker")), "start": u["start"] / 1000, "end": u["end"] / 1000,
              "text": u.get("text", "")} for u in data.get("utterances") or []]
    billed = float(data.get("audio_duration") or probe_duration(audio_path))
    per_hour = PRICE_PER_HOUR.get(model, PRICE_PER_HOUR["universal-3-5-pro"]) + ASSEMBLYAI_DIARIZATION_PER_HOUR
    return Result(
        text=str(data.get("text") or ""),
        raw=data,
        billed_seconds=billed,
        cost_usd=billed / 3600 * per_hour,
        wall_seconds=wall,
        turns=turns,
        extra={"speakers": len({t["speaker"] for t in turns}), "model": model},
    )


@dataclass(frozen=True)
class Provider:
    name: str
    key_service: str


PROVIDERS = {
    "openai-mini": Provider("openai-mini", "api.openai.com"),
    "openai-transcribe": Provider("openai-transcribe", "api.openai.com"),
    "elevenlabs": Provider("elevenlabs", "api.elevenlabs.io"),
    "assemblyai": Provider("assemblyai", "api.assemblyai.com"),
}


def run(provider: str, key: str, chunk_source: str, whole_file: str) -> Result:
    """Chạy một nhà cung cấp: OpenAI nhận mảnh cắt từ `chunk_source`, hai hãng kia nhận cả `whole_file`."""
    if provider == "openai-mini":
        return openai_transcribe(key, "gpt-4o-mini-transcribe", chunk_source)
    if provider == "openai-transcribe":
        return openai_transcribe(key, "gpt-transcribe", chunk_source)
    if provider == "elevenlabs":
        return elevenlabs_transcribe(key, whole_file)
    if provider == "assemblyai":
        return assemblyai_transcribe(key, whole_file)
    raise ValueError(f"nhà cung cấp không biết: {provider}")
