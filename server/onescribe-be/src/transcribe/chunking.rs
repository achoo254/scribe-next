//! Cắt bản ghi thành mảnh gửi OpenAI + phát hiện chữ bị lặp + chia câu có mốc ước lượng.
//!
//! Chuyển từ `sidecar/transcriber/openai_transcriber.py` (bản desktop đã đo: khớp TurboScribe ~84%).
//! Mảnh dài (~2 phút) làm model lặp nguyên khối câu; mảnh ~40 giây thì sạch. Vùng có tiếng nói lấy
//! từ ffmpeg `silencedetect` (phần bù của các khoảng lặng) thay cho silero VAD của bản desktop.

use std::collections::HashMap;

/// Điểm cắt ưu tiên quanh mốc này (giây).
const CHUNK_TARGET: f64 = 40.0;
/// Không cắt sớm hơn, tránh mảnh vụn.
const CHUNK_MIN: f64 = 25.0;
/// Không có khoảng lặng trong `[MIN, MAX]` thì cắt cứng ở `TARGET`.
pub const CHUNK_MAX: f64 = 45.0;
/// Câu dài hơn thì tách tiếp (model hay bỏ dấu chấm cả mảnh 40 giây): ~2 dòng phụ đề, ~8 giây nói.
const MAX_SENTENCE_CHARS: usize = 120;
/// Nhanh hơn mức này là model bịa hoặc lặp (người nói nhanh ~200 từ/phút).
const MAX_WORDS_PER_MIN: f64 = 260.0;
const MIN_SECONDS_FOR_SPEED: f64 = 10.0;

/// Một vùng thời gian `[start, end)` tính bằng giây.
pub type Span = (f64, f64);

/// Vùng có tiếng nói = phần bù của các khoảng lặng trong `[0, duration]`.
pub fn speech_from_silences(silences: &[Span], duration: f64) -> Vec<Span> {
    let mut speech = Vec::new();
    let mut cursor = 0.0;
    for &(start, end) in silences {
        if start > cursor {
            speech.push((cursor, start.min(duration)));
        }
        cursor = cursor.max(end);
    }
    if duration > cursor {
        speech.push((cursor, duration));
    }
    speech.retain(|(s, e)| e > s);
    speech
}

/// Đọc các khoảng lặng từ stderr của `ffmpeg -af silencedetect`. Khoảng lặng chưa đóng khi hết file
/// thì kéo tới `duration`.
pub fn parse_silencedetect(stderr: &str, duration: f64) -> Vec<Span> {
    let mut silences = Vec::new();
    let mut open: Option<f64> = None;
    for line in stderr.lines() {
        if let Some(v) = value_after(line, "silence_start:") {
            open = Some(v.max(0.0));
        } else if let Some(v) = value_after(line, "silence_end:") {
            silences.push((open.take().unwrap_or(0.0), v));
        }
    }
    if let Some(start) = open {
        silences.push((start, duration));
    }
    silences
}

fn value_after(line: &str, key: &str) -> Option<f64> {
    let rest = &line[line.find(key)? + key.len()..];
    rest.split_whitespace().next()?.parse().ok()
}

/// Các mảnh `[a, z)` phủ kín `[0, duration]`, cắt ở giữa khoảng lặng dài nhất trong cửa sổ
/// `[a + MIN, a + MAX]`.
pub fn plan_chunks(speech: &[Span], duration: f64) -> Vec<Span> {
    let gaps: Vec<(f64, f64)> = speech
        .windows(2)
        .filter(|w| w[1].0 > w[0].1)
        .map(|w| ((w[0].1 + w[1].0) / 2.0, w[1].0 - w[0].1))
        .collect();
    let mut cuts = vec![0.0];
    while let Some(&a) = cuts.last() {
        if duration - a <= CHUNK_MAX {
            break;
        }
        let best = gaps
            .iter()
            .filter(|(mid, _)| *mid >= a + CHUNK_MIN && *mid <= a + CHUNK_MAX)
            .max_by(|x, y| x.1.total_cmp(&y.1));
        cuts.push(best.map_or(a + CHUNK_TARGET, |g| g.0));
    }
    cuts.push(duration);
    cuts.windows(2).map(|w| (w[0], w[1])).collect()
}

/// Mảnh có tiếng nói không. Mảnh im lặng hoàn toàn dễ bị bịa chữ và tốn tiền vô ích.
pub fn has_speech(chunk: Span, speech: &[Span]) -> bool {
    speech.iter().any(|&(s, e)| e > chunk.0 && s < chunk.1)
}

/// Chữ có dấu hiệu model lặp hoặc bịa: cụm 8 từ lặp lại nhiều, hoặc nói nhanh bất thường.
pub fn looks_looped(text: &str, seconds: f64) -> bool {
    let lower = text.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    let mut grams: HashMap<&[&str], usize> = HashMap::new();
    for gram in words.windows(8) {
        *grams.entry(gram).or_default() += 1;
    }
    let repeated: usize = grams.values().filter(|&&c| c > 1).map(|c| c - 1).sum();
    // Tốc độ chỉ có nghĩa với mảnh đủ dài: mảnh cuối 2 giây có 10 từ đã là 300 từ/phút.
    let too_fast = seconds >= MIN_SECONDS_FOR_SPEED && words.len() as f64 / (seconds / 60.0) > MAX_WORDS_PER_MIN;
    repeated > 3 || too_fast
}

/// Một câu có mốc thời gian (ước lượng) trong bản ghi.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Sentence {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// Chia chữ của một mảnh thành câu, rải mốc theo độ dài chữ trên phần thời gian CÓ tiếng nói của mảnh.
/// Model không trả timestamp nên mốc là ước lượng, sai số trong phạm vi một mảnh.
pub fn sentences(text: &str, chunk: Span, speech: &[Span]) -> Vec<Sentence> {
    let parts = split_sentences(text);
    if parts.is_empty() {
        return Vec::new();
    }
    let mut spans: Vec<Span> = speech
        .iter()
        .map(|&(s, e)| (s.max(chunk.0), e.min(chunk.1)))
        .filter(|(s, e)| e > s)
        .collect();
    if spans.is_empty() {
        spans.push(chunk);
    }
    let spoken: f64 = spans.iter().map(|(s, e)| e - s).sum();
    let total_chars: usize = parts.iter().map(|p| p.chars().count()).sum();
    let per_char = spoken / total_chars.max(1) as f64;

    let mut used = 0usize;
    parts
        .into_iter()
        .map(|part| {
            let chars = part.chars().count();
            let start = to_clock(&spans, used as f64 * per_char, false);
            used += chars;
            let end = to_clock(&spans, used as f64 * per_char, true);
            Sentence { start, end, text: part }
        })
        .collect()
}

/// Đổi "giây nói thứ t" ra giờ thực. Tại điểm nối hai vùng: mốc đầu thuộc vùng SAU, mốc cuối thuộc vùng TRƯỚC.
fn to_clock(spans: &[Span], mut t: f64, is_end: bool) -> f64 {
    for &(s, e) in spans {
        let len = e - s;
        if t < len || (is_end && t <= len + 1e-9) {
            return s + t;
        }
        t -= len;
    }
    spans.last().map_or(0.0, |sp| sp.1)
}

/// Tách câu tại `.`, `?`, `!`, `…` theo sau là khoảng trắng; câu quá dài tách tiếp ([`split_long`]).
fn split_sentences(text: &str) -> Vec<String> {
    split_at_stops(text).into_iter().flat_map(|s| split_long(&s)).collect()
}

fn split_at_stops(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        current.push(c);
        let ends = matches!(c, '.' | '?' | '!' | '…');
        if ends && chars.peek().is_none_or(|n| n.is_whitespace()) {
            push_trimmed(&mut out, &mut current);
        }
    }
    push_trimmed(&mut out, &mut current);
    out
}

/// Câu dài hơn [`MAX_SENTENCE_CHARS`]: gom các vế (tách sau ", ") cho tới gần giới hạn; vế nào vẫn
/// quá dài thì gom theo từ.
fn split_long(sentence: &str) -> Vec<String> {
    if sentence.chars().count() <= MAX_SENTENCE_CHARS {
        return vec![sentence.to_string()];
    }
    let clauses = sentence.split_inclusive(", ").flat_map(|clause| {
        if clause.chars().count() <= MAX_SENTENCE_CHARS {
            vec![clause.trim().to_string()]
        } else {
            pack(clause.split_whitespace())
        }
    });
    pack(clauses)
}

/// Nối các mảnh chữ liên tiếp (cách nhau một khoảng trắng) thành dòng không vượt
/// [`MAX_SENTENCE_CHARS`], trừ mảnh tự nó đã dài hơn.
fn pack<S: AsRef<str>>(pieces: impl Iterator<Item = S>) -> Vec<String> {
    let mut out = Vec::new();
    let mut line = String::new();
    for piece in pieces {
        let piece = piece.as_ref().trim();
        if piece.is_empty() {
            continue;
        }
        if !line.is_empty() && line.chars().count() + 1 + piece.chars().count() > MAX_SENTENCE_CHARS {
            out.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(piece);
    }
    if !line.is_empty() {
        out.push(line);
    }
    out
}

fn push_trimmed(out: &mut Vec<String>, current: &mut String) {
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        out.push(trimmed.to_string());
    }
    current.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speech_is_the_complement_of_silences() {
        let speech = speech_from_silences(&[(0.0, 1.5), (10.0, 12.0)], 20.0);
        assert_eq!(speech, vec![(1.5, 10.0), (12.0, 20.0)]);
        assert_eq!(speech_from_silences(&[], 5.0), vec![(0.0, 5.0)]);
        assert!(speech_from_silences(&[(0.0, 5.0)], 5.0).is_empty());
    }

    #[test]
    fn reads_ffmpeg_silencedetect_output() {
        let stderr = "\
[silencedetect @ 0x1] silence_start: 3.2
[silencedetect @ 0x1] silence_end: 4.75 | silence_duration: 1.55
[silencedetect @ 0x1] silence_start: -0.01
[silencedetect @ 0x1] silence_end: 0.5 | silence_duration: 0.51
[silencedetect @ 0x1] silence_start: 58.1";
        let silences = parse_silencedetect(stderr, 60.0);
        assert_eq!(silences, vec![(3.2, 4.75), (0.0, 0.5), (58.1, 60.0)]);
    }

    #[test]
    fn short_file_is_one_chunk() {
        assert_eq!(plan_chunks(&[(0.0, 30.0)], 30.0), vec![(0.0, 30.0)]);
    }

    #[test]
    fn cuts_in_the_longest_pause_inside_the_window() {
        // Khoảng lặng 28–29 (1s) và 35–38 (3s) đều trong [25, 45] → cắt giữa khoảng dài: 36,5.
        let speech = [(0.0, 28.0), (29.0, 35.0), (38.0, 80.0)];
        let chunks = plan_chunks(&speech, 80.0);
        assert_eq!(chunks[0], (0.0, 36.5));
        assert!(chunks.iter().all(|(a, z)| z - a <= CHUNK_MAX));
        assert_eq!(chunks.last().map(|c| c.1), Some(80.0));
    }

    #[test]
    fn cuts_hard_when_nobody_pauses() {
        let chunks = plan_chunks(&[(0.0, 100.0)], 100.0);
        assert_eq!(chunks, vec![(0.0, 40.0), (40.0, 80.0), (80.0, 100.0)]);
    }

    #[test]
    fn silent_chunks_are_skipped() {
        let speech = [(0.0, 10.0)];
        assert!(has_speech((0.0, 40.0), &speech));
        assert!(!has_speech((40.0, 80.0), &speech));
    }

    #[test]
    fn detects_loops_and_impossible_speed() {
        let clean = "hôm nay chúng ta họp về kế hoạch quý bốn và phân công công việc cho từng nhóm";
        assert!(!looks_looped(clean, 10.0));
        let looped = "một hai ba bốn năm sáu bảy tám ".repeat(6);
        assert!(looks_looped(&looped, 60.0));
        let fast = "chữ ".repeat(300);
        assert!(looks_looped(&fast, 60.0));
        // Mảnh cuối ngắn nói vài từ không bị coi là bịa.
        assert!(!looks_looped("xin cảm ơn mọi người nhé", 1.0));
    }

    #[test]
    fn sentences_get_times_inside_speech() {
        let text = "Xin chào. Hôm nay họp nhé? Được!";
        let speech = [(0.0, 4.0), (6.0, 10.0)];
        let out = sentences(text, (0.0, 10.0), &speech);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].text, "Xin chào.");
        assert_eq!(out[0].start, 0.0);
        assert!(out.windows(2).all(|w| w[0].end <= w[1].start + 1e-9));
        assert!((out[2].end - 10.0).abs() < 1e-9);
        // Không câu nào bắt đầu trong khoảng lặng 4–6.
        assert!(out.iter().all(|s| !(s.start > 4.0 && s.start < 6.0)));
    }

    #[test]
    fn long_unpunctuated_text_is_split_at_commas_then_words() {
        let clause = "bọn em phát triển nền tảng tập trung cho khách hàng";
        let text = format!("{clause}, {clause}, {clause}, {clause}");
        let parts = split_sentences(&text);
        assert!(parts.len() >= 2);
        assert!(parts.iter().all(|p| p.chars().count() <= MAX_SENTENCE_CHARS));
        assert_eq!(parts.join(" "), text);

        let long_clause = "một hai ba bốn năm ".repeat(10);
        let text = format!("lớp mù, {}, lớp network, xong", long_clause.trim());
        assert_eq!(split_sentences(&text).join(" "), text);

        let words = "chữ ".repeat(100);
        let parts = split_sentences(words.trim());
        assert!(parts.len() >= 3 && parts.iter().all(|p| p.chars().count() <= MAX_SENTENCE_CHARS));
    }

    #[test]
    fn decimal_points_do_not_split_sentences() {
        assert_eq!(split_sentences("Giá 3.5 triệu. Xong"), vec!["Giá 3.5 triệu.", "Xong"]);
    }
}
