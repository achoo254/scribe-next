"""Kiểm phần tính điểm: python3 -I scripts/stt-bench/test_metrics.py"""

import random
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import metrics  # noqa: E402
import providers  # noqa: E402


def naive_distance(a, b):
    row = list(range(len(b) + 1))
    for i, x in enumerate(a, 1):
        prev, row[0] = row[0], i
        for j, y in enumerate(b, 1):
            prev, row[j] = row[j], min(row[j] + 1, row[j - 1] + 1, prev + (x != y))
    return row[-1]


class NormalizeTest(unittest.TestCase):
    def test_keeps_diacritics_and_drops_punctuation(self):
        self.assertEqual(metrics.normalize("Xin CHÀO, các bạn!  Hẹn gặp lại…"), "xin chào các bạn hẹn gặp lại")

    def test_nfd_input_equals_nfc(self):
        self.assertEqual(metrics.normalize("Việt"), metrics.normalize("Việt"))

    def test_no_diacritics_variant(self):
        self.assertEqual(metrics.normalize("Đường Việt Nam", keep_diacritics=False), "duong viet nam")


class DistanceTest(unittest.TestCase):
    def test_matches_naive_dp_on_random_sequences(self):
        rng = random.Random(7)
        for _ in range(300):
            a = [rng.choice("abcd") for _ in range(rng.randint(0, 90))]
            b = [rng.choice("abcd") for _ in range(rng.randint(0, 90))]
            self.assertEqual(metrics.edit_distance(a, b), naive_distance(a, b), (a, b))

    def test_word_error_rate(self):
        r = metrics.error_rates("hôm nay trời đẹp", "Hôm nay trời rất đẹp.")
        self.assertAlmostEqual(r["wer"], 0.25)
        self.assertEqual(r["refWords"], 4)

    def test_tone_error_only_counts_with_diacritics(self):
        self.assertAlmostEqual(metrics.error_rates("vướng mắc", "vướng mắt")["wer"], 0.5)
        self.assertAlmostEqual(metrics.error_rates("vướng mắc", "vuong mac", keep_diacritics=False)["wer"], 0.0)

    def test_agreement_symmetric(self):
        self.assertAlmostEqual(metrics.agreement("a b c d", "a b x d"), 0.75)
        self.assertAlmostEqual(metrics.agreement("a b c d", "a b x d"), metrics.agreement("a b x d", "a b c d"))
        self.assertEqual(metrics.agreement("", ""), 1.0)


class SrtTest(unittest.TestCase):
    SRT = "1\n00:00:01,000 --> 00:00:02,000\nmột hai\n\n2\n00:00:10,500 --> 00:00:12,000\nba\nbốn\n\n3\n00:01:00,000 --> 00:01:01,000\nnăm\n"

    def test_parse(self):
        cues = metrics.parse_srt(self.SRT)
        self.assertEqual(cues[1], (10.5, 12.0, "ba bốn"))

    def test_window_by_cue_midpoint(self):
        self.assertEqual(metrics.srt_window(self.SRT, 5, 60), "ba bốn")
        self.assertEqual(metrics.srt_window(self.SRT, 0, None), "một hai ba bốn năm")


class LoopTest(unittest.TestCase):
    def test_repeated_phrase_is_loop(self):
        self.assertTrue(metrics.looks_looped(" ".join(["một hai ba bốn năm sáu bảy tám"] * 6), 40))

    def test_normal_speech_is_not_loop(self):
        self.assertFalse(metrics.looks_looped("hôm nay chúng ta họp về kế hoạch quý bốn", 40))


class TurnsTest(unittest.TestCase):
    def test_groups_consecutive_words_by_speaker(self):
        words = [
            {"text": "xin", "start": 0.0, "end": 0.3, "speaker_id": "speaker_0"},
            {"text": "chào", "start": 0.3, "end": 0.6, "speaker_id": "speaker_0"},
            {"text": "vâng", "start": 1.0, "end": 1.4, "speaker_id": "speaker_1"},
            {"text": "ừ", "start": None, "end": None, "speaker_id": "speaker_1"},
        ]
        self.assertEqual(providers._turns_from_words(words, "speaker_id"), [
            {"speaker": "speaker_0", "start": 0.0, "end": 0.6, "text": "xin chào"},
            {"speaker": "speaker_1", "start": 1.0, "end": 1.4, "text": "vâng"},
        ])


if __name__ == "__main__":
    unittest.main()
