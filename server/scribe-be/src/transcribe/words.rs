//! Dựng câu từ timestamp từng từ (ElevenLabs): mốc câu là mốc thật của từ đầu và từ cuối, mỗi câu một người nói.
//!
//! Một lượt chép có thể là cả phiên iPhone ghép từ nhiều đoạn: từ được chia về đoạn chứa mốc bắt đầu của nó
//! ([`split_by_offsets`]), mốc tính lại trong đoạn, rồi mới dựng câu cho từng đoạn — câu không bao giờ vắt qua
//! hai đoạn, nên lưu theo từng file như bản OpenAI.

use super::chunking::Sentence;
use std::collections::HashMap;

/// Câu dài hơn thì tách tiếp (cùng giới hạn với bản OpenAI): ~2 dòng phụ đề.
const MAX_SENTENCE_CHARS: usize = 120;
/// Ngừng nói lâu hơn chừng này (giây) thì sang câu mới dù chưa có dấu câu.
const PAUSE_SPLIT_S: f64 = 1.5;

/// Một từ có mốc (giây) và người nói đã chuẩn hoá (`s0`, `s1`…).
#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub text: String,
    pub start: f64,
    pub end: f64,
    pub speaker: Option<String>,
}

/// Đổi id người nói của nhà cung cấp (`speaker_0`…) sang `s0`, `s1`… theo thứ tự xuất hiện, để "Người 1" luôn là
/// người nói đầu tiên. Từ không có người nói lấy người nói của từ trước nó.
pub fn normalize_speakers(words: &mut [Word]) {
    let mut ids: HashMap<String, String> = HashMap::new();
    let mut previous: Option<String> = None;
    for word in words.iter_mut() {
        let speaker = match word.speaker.take() {
            Some(raw) => {
                let next = format!("s{}", ids.len());
                Some(ids.entry(raw).or_insert(next).clone())
            }
            None => previous.clone(),
        };
        word.speaker = speaker.clone();
        previous = speaker;
    }
}

/// Chia từ về các đoạn bắt đầu ở `offsets` (giây, tăng dần, đoạn đầu ở 0): từ thuộc đoạn chứa mốc bắt đầu của nó,
/// mốc tính lại từ đầu đoạn đó. Từ trước mốc đầu tiên về đoạn đầu; trả đúng `offsets.len()` danh sách.
pub fn split_by_offsets(words: Vec<Word>, offsets: &[f64]) -> Vec<Vec<Word>> {
    let mut out: Vec<Vec<Word>> = vec![Vec::new(); offsets.len()];
    if offsets.is_empty() {
        return out;
    }
    for word in words {
        let index = offsets.iter().rposition(|&o| word.start >= o).unwrap_or(0);
        let base = offsets[index];
        out[index].push(Word { start: (word.start - base).max(0.0), end: (word.end - base).max(0.0), ..word });
    }
    out
}

/// Câu từ các từ theo thứ tự: tách sau `.`, `?`, `!`, `…`; khi đổi người nói; khi ngừng nói ≥ [`PAUSE_SPLIT_S`];
/// câu sắp vượt [`MAX_SENTENCE_CHARS`] thì cắt sau dấu phẩy cuối cùng, không có dấu phẩy thì cắt trước từ mới.
pub fn sentences(words: &[Word]) -> Vec<Sentence> {
    let mut out = Vec::new();
    let mut current: Vec<&Word> = Vec::new();
    for word in words.iter().filter(|w| !w.text.trim().is_empty()) {
        if let Some(last) = current.last() {
            if last.speaker != word.speaker || word.start - last.end >= PAUSE_SPLIT_S {
                flush(&mut out, &mut current);
            }
        }
        while !current.is_empty() && chars(&current) + 1 + word.text.trim().chars().count() > MAX_SENTENCE_CHARS {
            match current.iter().rposition(|w| w.text.trim_end().ends_with(',')) {
                Some(i) if i + 1 < current.len() => {
                    let rest = current.split_off(i + 1);
                    flush(&mut out, &mut current);
                    current = rest;
                }
                _ => flush(&mut out, &mut current),
            }
        }
        current.push(word);
        if ends_sentence(&word.text) {
            flush(&mut out, &mut current);
        }
    }
    flush(&mut out, &mut current);
    out
}

/// Chữ liền của các câu (trường `text` của lượt chép).
pub fn joined_text(sentences: &[Sentence]) -> String {
    sentences.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join(" ")
}

fn ends_sentence(text: &str) -> bool {
    text.trim_end().ends_with(['.', '?', '!', '…'])
}

/// Số ký tự của câu đang gom (các từ cách nhau một khoảng trắng).
fn chars(words: &[&Word]) -> usize {
    let letters: usize = words.iter().map(|w| w.text.trim().chars().count()).sum();
    letters + words.len().saturating_sub(1)
}

fn flush(out: &mut Vec<Sentence>, current: &mut Vec<&Word>) {
    if let (Some(first), Some(last)) = (current.first(), current.last()) {
        let text = current.iter().map(|w| w.text.trim()).collect::<Vec<_>>().join(" ");
        out.push(Sentence { start: first.start, end: last.end.max(first.start), text, speaker: first.speaker.clone() });
    }
    current.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(text: &str, start: f64, end: f64, speaker: &str) -> Word {
        Word { text: text.into(), start, end, speaker: Some(speaker.into()) }
    }

    #[test]
    fn sentences_end_at_punctuation_and_keep_real_times() {
        let words = [
            w("Xin", 0.2, 0.4, "s0"),
            w("chào.", 0.4, 0.9, "s0"),
            w("Hôm", 1.0, 1.2, "s0"),
            w("nay", 1.2, 1.4, "s0"),
            w("họp", 1.4, 1.6, "s0"),
            w("nhé?", 1.6, 2.0, "s0"),
        ];
        let out = sentences(&words);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0], Sentence { start: 0.2, end: 0.9, text: "Xin chào.".into(), speaker: Some("s0".into()) });
        assert_eq!((out[1].start, out[1].end, out[1].text.as_str()), (1.0, 2.0, "Hôm nay họp nhé?"));
    }

    #[test]
    fn a_new_speaker_or_a_long_pause_starts_a_new_sentence() {
        let words = [
            w("vâng", 0.0, 0.3, "s0"),
            w("được", 0.3, 0.6, "s0"),
            w("anh", 0.7, 0.9, "s1"),
            w("nói", 0.9, 1.1, "s1"),
            w("tiếp", 3.0, 3.3, "s1"),
        ];
        let out = sentences(&words);
        let got: Vec<(&str, Option<&str>)> = out.iter().map(|s| (s.text.as_str(), s.speaker.as_deref())).collect();
        assert_eq!(got, vec![("vâng được", Some("s0")), ("anh nói", Some("s1")), ("tiếp", Some("s1"))]);
    }

    #[test]
    fn long_runs_are_cut_after_the_last_comma_then_between_words() {
        let mut words = Vec::new();
        let mut t = 0.0;
        for i in 0..60 {
            let text = if i % 10 == 9 { "chữ,".to_string() } else { "chữ".to_string() };
            words.push(w(&text, t, t + 0.2, "s0"));
            t += 0.25;
        }
        let out = sentences(&words);
        assert!(out.len() >= 2);
        assert!(out.iter().all(|s| s.text.chars().count() <= MAX_SENTENCE_CHARS), "{out:?}");
        assert!(out[0].text.ends_with(','), "cắt sau dấu phẩy: {:?}", out[0].text);
        let total: usize = out.iter().map(|s| s.text.split_whitespace().count()).sum();
        assert_eq!(total, 60, "không mất từ nào");
        assert!(out.windows(2).all(|p| p[0].end <= p[1].start));

        let plain: Vec<Word> = (0..80).map(|i| w("chữ", f64::from(i) * 0.25, f64::from(i) * 0.25 + 0.2, "s0")).collect();
        let out = sentences(&plain);
        assert!(out.len() >= 3 && out.iter().all(|s| s.text.chars().count() <= MAX_SENTENCE_CHARS));
    }

    #[test]
    fn decimal_numbers_do_not_end_a_sentence() {
        let words = [w("giá", 0.0, 0.2, "s0"), w("3.5", 0.2, 0.5, "s0"), w("triệu.", 0.5, 0.9, "s0")];
        assert_eq!(sentences(&words).len(), 1);
    }

    #[test]
    fn speakers_are_renumbered_by_first_appearance() {
        let mut words = vec![
            Word { text: "a".into(), start: 0.0, end: 0.1, speaker: Some("speaker_3".into()) },
            Word { text: "b".into(), start: 0.1, end: 0.2, speaker: None },
            Word { text: "c".into(), start: 0.2, end: 0.3, speaker: Some("speaker_0".into()) },
            Word { text: "d".into(), start: 0.3, end: 0.4, speaker: Some("speaker_3".into()) },
        ];
        normalize_speakers(&mut words);
        let got: Vec<Option<&str>> = words.iter().map(|w| w.speaker.as_deref()).collect();
        assert_eq!(got, vec![Some("s0"), Some("s0"), Some("s1"), Some("s0")]);
    }

    #[test]
    fn merged_session_words_go_back_to_their_segment_with_local_times() {
        // Ba đoạn 60 + 60 + 30 giây ghép liền: mốc đoạn 0, 60, 120.
        let words = vec![
            w("Một.", 1.0, 1.5, "s0"),
            w("Hai", 59.6, 59.9, "s0"),
            w("ba.", 60.1, 60.6, "s0"),
            w("Bốn.", 125.0, 125.4, "s1"),
        ];
        let parts = split_by_offsets(words, &[0.0, 60.0, 120.0]);
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0].iter().map(|w| w.text.as_str()).collect::<Vec<_>>(), vec!["Một.", "Hai"]);
        assert_eq!(parts[1].len(), 1);
        assert!((parts[1][0].start - 0.1).abs() < 1e-9 && (parts[1][0].end - 0.6).abs() < 1e-9);
        assert!((parts[2][0].start - 5.0).abs() < 1e-9);
        // Câu không vắt qua đoạn: "Hai" kết thúc đoạn 0, "ba." mở đầu đoạn 1.
        let first = sentences(&parts[0]);
        assert_eq!(first.iter().map(|s| s.text.as_str()).collect::<Vec<_>>(), vec!["Một.", "Hai"]);
        assert_eq!(sentences(&parts[2])[0].speaker.as_deref(), Some("s1"));
        assert!(split_by_offsets(Vec::new(), &[]).is_empty());
    }

    #[test]
    fn joined_text_is_the_sentences_in_order() {
        let words = [w("Một.", 0.0, 0.5, "s0"), w("Hai.", 0.6, 1.0, "s1")];
        assert_eq!(joined_text(&sentences(&words)), "Một. Hai.");
    }
}
