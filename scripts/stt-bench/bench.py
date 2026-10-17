#!/usr/bin/env python3
"""Đo chép lời tiếng Việt trên bản ghi họp thật: OpenAI, ElevenLabs Scribe v2, AssemblyAI.

    ~/.claude/skills/.venv/bin/python3 -I scripts/stt-bench/bench.py MANIFEST.json --out DIR \
        [--providers openai-mini,openai-transcribe,elevenlabs,assemblyai] [--budget 3] [--force]
    ~/.claude/skills/.venv/bin/python3 -I scripts/stt-bench/bench.py MANIFEST.json --out DIR --score-only

MANIFEST.json (đường dẫn tương đối tính từ thư mục của manifest):

    {"items": [{"id": "hop-a", "audio": "a.m4a", "start": 454, "end": 1770, "ref": "a.srt"},
               {"id": "hop-b", "audio": "b.m4a"}]}

`ref` là .srt (lấy cue có điểm giữa trong [start, end)) hoặc .txt; không có `ref` thì chỉ đo độ khớp giữa các
nhà cung cấp. Kết quả đã có của một (tệp, nhà cung cấp) được dùng lại, nên chạy lại sau khi thêm key chỉ gọi
nhà cung cấp mới. Key đọc từ `$ACCESS_TOKENS_JSON`; thiếu key thì bỏ qua nhà cung cấp đó. Audio và chữ chép
chỉ nằm trong `--out`: đặt thư mục này ngoài repo.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time
from itertools import combinations
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))

import metrics  # noqa: E402
import providers  # noqa: E402

ALL_PROVIDERS = list(providers.PROVIDERS)


def load_manifest(path: Path) -> list[dict[str, Any]]:
    """Đọc và kiểm manifest; trả danh sách mục với đường dẫn tuyệt đối."""
    try:
        items = json.loads(path.read_text(encoding="utf-8"))["items"]
    except (OSError, ValueError, KeyError, TypeError) as err:
        raise SystemExit(f"manifest không đọc được: {err}") from None
    base = path.resolve().parent
    seen: set[str] = set()
    for item in items:
        if not isinstance(item, dict) or not str(item.get("id", "")).strip() or "audio" not in item:
            raise SystemExit("mỗi mục manifest cần `id` và `audio`")
        if item["id"] in seen or "/" in item["id"]:
            raise SystemExit(f"id trùng hoặc có '/': {item['id']}")
        seen.add(item["id"])
        for key in ("audio", "ref"):
            if item.get(key):
                item[key] = str((base / os.path.expanduser(item[key])).resolve())
                if not os.path.isfile(item[key]):
                    raise SystemExit(f"{item['id']}: không thấy tệp {key}")
        for key in ("start", "end"):
            if item.get(key) is not None and not isinstance(item[key], (int, float)):
                raise SystemExit(f"{item['id']}: `{key}` phải là số giây")
    return items


def prepare_audio(item: dict[str, Any], folder: Path) -> tuple[Path, Path]:
    """Cắt đoạn [start, end) thành WAV mono 16 kHz (nguồn để cắt mảnh) và Opus 32 kbps (gửi cả tệp)."""
    folder.mkdir(parents=True, exist_ok=True)
    wav, ogg = folder / "audio.wav", folder / "audio.ogg"
    window = []
    if item.get("start") is not None:
        window += ["-ss", str(item["start"])]
    if item.get("end") is not None:
        window += ["-to", str(item["end"])]
    if not wav.exists():
        providers.run_ffmpeg(["-y", *window, "-i", item["audio"], "-vn", "-ac", "1", "-ar", "16000", str(wav)])
    if not ogg.exists():
        providers.run_ffmpeg(["-y", "-i", str(wav), "-c:a", "libopus", "-b:a", "32k", str(ogg)])
    return wav, ogg


def reference_text(item: dict[str, Any]) -> str | None:
    if not item.get("ref"):
        return None
    text = Path(item["ref"]).read_text(encoding="utf-8")
    if item["ref"].lower().endswith(".srt"):
        return metrics.srt_window(text, float(item.get("start") or 0), item.get("end"))
    return text


def write_json(path: Path, data: Any) -> None:
    path.write_text(json.dumps(data, ensure_ascii=False, indent=1), encoding="utf-8")


def run_all(items: list[dict[str, Any]], out: Path, wanted: list[str], budget: float, force: bool) -> None:
    keys: dict[str, str] = {}
    for name in wanted:
        key = providers.load_key(providers.PROVIDERS[name].key_service)
        if key:
            keys[name] = key
        else:
            print(f"[bỏ qua] {name}: chưa có key `{providers.PROVIDERS[name].key_service}` trong $ACCESS_TOKENS_JSON")

    prepared = {}
    for item in items:
        prepared[item["id"]] = prepare_audio(item, out / item["id"])
    hours = {i["id"]: providers.probe_duration(str(prepared[i["id"]][0])) / 3600 for i in items}

    todo = [(i["id"], p) for i in items for p in keys
            if force or not (out / i["id"] / f"{p}.meta.json").exists()]
    model_of = {"openai-mini": "gpt-4o-mini-transcribe", "openai-transcribe": "gpt-transcribe",
                "elevenlabs": "scribe_v2", "assemblyai": "universal-3-5-pro"}
    estimate = sum(hours[f] * (providers.PRICE_PER_HOUR[model_of[p]]
                               + (providers.ASSEMBLYAI_DIARIZATION_PER_HOUR if p == "assemblyai" else 0))
                   for f, p in todo)
    print(f"Cần chạy {len(todo)} lượt, ước tính ${estimate:.2f} (trần ${budget:.2f})")
    if estimate > budget:
        raise SystemExit("Ước tính vượt trần --budget, dừng trước khi gửi audio.")

    for file_id, name in todo:
        wav, ogg = prepared[file_id]
        folder = out / file_id
        print(f"[chạy] {file_id} · {name} …", flush=True)
        started = time.time()
        try:
            result = providers.run(name, keys[name], str(wav), str(ogg))
        except providers.ProviderError as err:
            print(f"[lỗi] {file_id} · {name}: {err}")
            write_json(folder / f"{name}.error.json", {"error": str(err), "at": started})
            continue
        write_json(folder / f"{name}.raw.json", result.raw)
        (folder / f"{name}.txt").write_text(result.text, encoding="utf-8")
        if result.turns:
            write_json(folder / f"{name}.turns.json", result.turns)
        write_json(folder / f"{name}.meta.json", {
            "provider": name, "audioSeconds": hours[file_id] * 3600, "billedSeconds": result.billed_seconds,
            "costUsd": result.cost_usd, "wallSeconds": result.wall_seconds,
            "speakers": result.extra.get("speakers"), "extra": result.extra, "at": started,
        })
        print(f"[xong] {file_id} · {name}: {result.wall_seconds:.0f} s, ${result.cost_usd:.4f}")


def score(items: list[dict[str, Any]], out: Path) -> dict[str, Any]:
    """Tính WER/CER (khi có văn bản gốc), độ khớp từng cặp, chi phí, độ trễ; ghi summary.json và summary.md."""
    files = []
    totals: dict[str, dict[str, float]] = {}
    for item in items:
        folder = out / item["id"]
        ref = reference_text(item)
        entry: dict[str, Any] = {"id": item["id"], "hasReference": ref is not None, "providers": {}, "agreement": {}}
        texts = {}
        for name in ALL_PROVIDERS:
            meta_path = folder / f"{name}.meta.json"
            if not meta_path.exists():
                continue
            meta = json.loads(meta_path.read_text(encoding="utf-8"))
            text = (folder / f"{name}.txt").read_text(encoding="utf-8")
            texts[name] = text
            row = {k: meta[k] for k in ("audioSeconds", "billedSeconds", "costUsd", "wallSeconds", "speakers")}
            row["extra"] = meta.get("extra", {})
            row["words"] = len(metrics.normalize(text).split())
            if ref is not None:
                row["diacritics"] = metrics.error_rates(ref, text, keep_diacritics=True)
                row["noDiacritics"] = metrics.error_rates(ref, text, keep_diacritics=False)
                write_json(folder / f"{name}.diffs.json", metrics.differences(ref, text))
            entry["providers"][name] = row
            t = totals.setdefault(name, {"audio": 0, "cost": 0, "wall": 0, "refWords": 0, "wordErr": 0,
                                         "charErr": 0, "refChars": 0, "wordErrNd": 0})
            t["audio"] += meta["audioSeconds"]
            t["cost"] += meta["costUsd"]
            t["wall"] += meta["wallSeconds"]
            if ref is not None:
                d, nd = row["diacritics"], row["noDiacritics"]
                ref_chars = len(metrics.normalize(ref).replace(" ", ""))
                t["refWords"] += d["refWords"]
                t["wordErr"] += d["wer"] * d["refWords"]
                t["wordErrNd"] += nd["wer"] * nd["refWords"]
                t["charErr"] += d["cer"] * ref_chars
                t["refChars"] += ref_chars
        for a, b in combinations(sorted(texts), 2):
            entry["agreement"][f"{a}|{b}"] = metrics.agreement(texts[a], texts[b])
        if ref is not None:
            entry["refWords"] = len(metrics.normalize(ref).split())
        files.append(entry)

    overall = {}
    for name, t in totals.items():
        overall[name] = {
            "audioHours": t["audio"] / 3600,
            "costUsd": t["cost"],
            "costPerHour": t["cost"] / (t["audio"] / 3600) if t["audio"] else None,
            "realTimeFactor": t["wall"] / t["audio"] if t["audio"] else None,
            "wer": t["wordErr"] / t["refWords"] if t["refWords"] else None,
            "werNoDiacritics": t["wordErrNd"] / t["refWords"] if t["refWords"] else None,
            "cer": t["charErr"] / t["refChars"] if t["refChars"] else None,
        }
    summary = {"files": files, "overall": overall}
    write_json(out / "summary.json", summary)
    (out / "summary.md").write_text(render_markdown(summary), encoding="utf-8")
    return summary


def pct(value: float | None) -> str:
    return "—" if value is None else f"{value * 100:.1f}%".replace(".", ",")


def render_markdown(summary: dict[str, Any]) -> str:
    lines = ["| Tệp | Nhà cung cấp | Phút | WER | WER không dấu | CER | Số từ | $/giờ | Thời gian chạy | Người nói |",
             "|---|---|---|---|---|---|---|---|---|---|"]
    for f in summary["files"]:
        for name, r in f["providers"].items():
            d, nd = r.get("diacritics", {}), r.get("noDiacritics", {})
            per_hour = r["costUsd"] / (r["audioSeconds"] / 3600) if r["audioSeconds"] else 0
            lines.append(
                f"| {f['id']} | {name} | {r['audioSeconds'] / 60:.1f} | {pct(d.get('wer'))} | {pct(nd.get('wer'))} | "
                f"{pct(d.get('cer'))} | {r['words']} | {per_hour:.3f} | {r['wallSeconds']:.0f} s | {r['speakers'] or '—'} |"
            )
    lines += ["", "| Tệp | Cặp | Độ khớp |", "|---|---|---|"]
    for f in summary["files"]:
        for pair, value in f["agreement"].items():
            lines.append(f"| {f['id']} | {pair.replace('|', ' / ')} | {pct(value)} |")
    lines += ["", "| Nhà cung cấp | Giờ audio | Chi phí | $/giờ | WER | WER không dấu | CER | Thời gian chạy / thời lượng |",
              "|---|---|---|---|---|---|---|---|"]
    for name, o in summary["overall"].items():
        lines.append(
            f"| {name} | {o['audioHours']:.2f} | ${o['costUsd']:.3f} | {o['costPerHour'] or 0:.3f} | {pct(o['wer'])} | "
            f"{pct(o['werNoDiacritics'])} | {pct(o['cer'])} | {pct(o['realTimeFactor'])} |"
        )
    return "\n".join(lines) + "\n"


def main() -> None:
    parser = argparse.ArgumentParser(description="Đo chép lời tiếng Việt trên nhiều nhà cung cấp.")
    parser.add_argument("manifest", type=Path)
    parser.add_argument("--out", type=Path, required=True, help="thư mục kết quả, đặt ngoài repo")
    parser.add_argument("--providers", default=",".join(ALL_PROVIDERS))
    parser.add_argument("--budget", type=float, default=3.0, help="trần USD ước tính cho các lượt sắp chạy")
    parser.add_argument("--force", action="store_true", help="chạy lại cả lượt đã có kết quả")
    parser.add_argument("--score-only", action="store_true", help="chỉ tính điểm từ kết quả đã có")
    args = parser.parse_args()

    wanted = [p.strip() for p in args.providers.split(",") if p.strip()]
    unknown = [p for p in wanted if p not in providers.PROVIDERS]
    if unknown:
        raise SystemExit(f"nhà cung cấp không biết: {', '.join(unknown)} (có: {', '.join(ALL_PROVIDERS)})")
    repo = Path(__file__).resolve().parents[2]
    if args.out.resolve().is_relative_to(repo):
        raise SystemExit("--out phải nằm ngoài repo: audio và chữ chép của cuộc họp không được commit")
    items = load_manifest(args.manifest)
    args.out.mkdir(parents=True, exist_ok=True)
    if not args.score_only:
        run_all(items, args.out, wanted, args.budget, args.force)
    score(items, args.out)
    print((args.out / "summary.md").read_text(encoding="utf-8"))


if __name__ == "__main__":
    main()
