"""Chuẩn hoá chữ tiếng Việt và đo WER/CER, độ khớp giữa hai bản chép (chỉ dùng thư viện chuẩn).

Khoảng cách sửa (Levenshtein) tính bằng thuật toán bit-vector của Myers/Hyyrö trên số nguyên Python, nên
so được bản chép 15.000 từ (hay 70.000 ký tự) trong vài giây thay vì ma trận O(n·m).
"""

from __future__ import annotations

import difflib
import re
import unicodedata
from collections.abc import Hashable, Sequence


def normalize(text: str, keep_diacritics: bool = True) -> str:
    """NFC, chữ thường, bỏ dấu câu/ký hiệu, gộp khoảng trắng. `keep_diacritics=False` bỏ cả dấu tiếng Việt."""
    text = unicodedata.normalize("NFC", text).lower()
    text = "".join(" " if unicodedata.category(ch)[0] in "PS" else ch for ch in text)
    if not keep_diacritics:
        text = unicodedata.normalize("NFD", text.replace("đ", "d"))
        text = "".join(ch for ch in text if not unicodedata.combining(ch))
    return " ".join(text.split())


def edit_distance(a: Sequence[Hashable], b: Sequence[Hashable]) -> int:
    """Khoảng cách Levenshtein giữa hai dãy (từ hoặc ký tự)."""
    if len(a) < len(b):
        a, b = b, a  # mặt nạ bit theo dãy dài hơn, vòng lặp theo dãy ngắn hơn
    m = len(a)
    if m == 0:
        return len(b)
    peq: dict[Hashable, int] = {}
    for i, token in enumerate(a):
        peq[token] = peq.get(token, 0) | (1 << i)
    mask = (1 << m) - 1
    last = 1 << (m - 1)
    pv, mv, score = mask, 0, m
    for token in b:
        eq = peq.get(token, 0)
        xv = eq | mv
        xh = (((eq & pv) + pv) ^ pv) | eq
        ph = (mv | ~(xh | pv)) & mask
        mh = pv & xh
        if ph & last:
            score += 1
        elif mh & last:
            score -= 1
        ph = ((ph << 1) | 1) & mask
        mh = (mh << 1) & mask
        pv = (mh | ~(xv | ph)) & mask
        mv = ph & xv
    return score


def error_rates(reference: str, hypothesis: str, keep_diacritics: bool = True) -> dict[str, float | int]:
    """WER và CER của `hypothesis` so với `reference` sau khi chuẩn hoá."""
    ref = normalize(reference, keep_diacritics)
    hyp = normalize(hypothesis, keep_diacritics)
    ref_words, hyp_words = ref.split(), hyp.split()
    ref_chars, hyp_chars = ref.replace(" ", ""), hyp.replace(" ", "")
    return {
        "wer": edit_distance(ref_words, hyp_words) / max(len(ref_words), 1),
        "cer": edit_distance(ref_chars, hyp_chars) / max(len(ref_chars), 1),
        "refWords": len(ref_words),
        "hypWords": len(hyp_words),
    }


def agreement(a: str, b: str) -> float:
    """Độ khớp đối xứng theo từ giữa hai bản chép: 1 − khoảng cách sửa / số từ của bản dài hơn."""
    wa, wb = normalize(a).split(), normalize(b).split()
    longest = max(len(wa), len(wb))
    return 1.0 if longest == 0 else 1.0 - edit_distance(wa, wb) / longest


def differences(reference: str, hypothesis: str, limit: int = 10, context: int = 3) -> list[dict[str, str | int]]:
    """Vài chỗ hai bản khác nhau (sau chuẩn hoá, giữ dấu), kèm vài từ ngữ cảnh, ưu tiên chỗ khác dài nhất."""
    ref, hyp = normalize(reference).split(), normalize(hypothesis).split()
    found: list[dict[str, str | int]] = []
    for tag, i1, i2, j1, j2 in difflib.SequenceMatcher(None, ref, hyp, autojunk=False).get_opcodes():
        if tag == "equal":
            continue
        before = " ".join(ref[max(i1 - context, 0):i1])
        after = " ".join(ref[i2:i2 + context])
        found.append({
            "ref": f"{before} [{' '.join(ref[i1:i2])}] {after}".strip(),
            "hyp": f"{before} [{' '.join(hyp[j1:j2])}] {after}".strip(),
            "size": max(i2 - i1, j2 - j1),
        })
    found.sort(key=lambda d: -int(d["size"]))
    return found[:limit]


_SRT_TIME = re.compile(r"(\d+):(\d+):(\d+)[,.](\d+)\s*-->\s*(\d+):(\d+):(\d+)[,.](\d+)")


def parse_srt(text: str) -> list[tuple[float, float, str]]:
    """Các cue `(start, end, chữ)` của một tệp SRT."""
    cues = []
    for block in re.split(r"\n\s*\n", text.replace("\r\n", "\n").strip()):
        lines = block.split("\n")
        for i, line in enumerate(lines):
            match = _SRT_TIME.search(line)
            if match:
                g = [int(x) for x in match.groups()]
                start = g[0] * 3600 + g[1] * 60 + g[2] + g[3] / 1000
                end = g[4] * 3600 + g[5] * 60 + g[6] + g[7] / 1000
                cues.append((start, end, " ".join(lines[i + 1:]).strip()))
                break
    return cues


def srt_window(text: str, start: float, end: float | None) -> str:
    """Chữ của các cue có điểm giữa nằm trong `[start, end)`; `end=None` là tới hết tệp."""
    picked = []
    for a, b, words in parse_srt(text):
        mid = (a + b) / 2
        if mid >= start and (end is None or mid < end):
            picked.append(words)
    return " ".join(picked)


def looks_looped(text: str, seconds: float) -> bool:
    """Giống `chunking::looks_looped` của backend: cụm 8 từ lặp nhiều, hoặc nói nhanh quá 260 từ/phút."""
    words = text.lower().split()
    grams: dict[tuple[str, ...], int] = {}
    for i in range(len(words) - 7):
        gram = tuple(words[i:i + 8])
        grams[gram] = grams.get(gram, 0) + 1
    repeated = sum(c - 1 for c in grams.values() if c > 1)
    too_fast = seconds >= 10 and len(words) / (seconds / 60) > 260
    return repeated > 3 or too_fast
