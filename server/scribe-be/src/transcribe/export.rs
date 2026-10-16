//! Xuất transcript ra txt/srt. Một phiên ghi iPhone là nhiều file (mỗi đoạn một file): ghép theo thứ
//! tự, mốc của file sau cộng dồn thời lượng các file trước.

use super::chunking::Sentence;

/// Transcript của một file kèm mốc bắt đầu của file đó trong bản ghép.
pub struct Part<'a> {
    pub offset_s: f64,
    pub sentences: &'a [Sentence],
}

pub fn to_txt(parts: &[Part<'_>]) -> String {
    let lines: Vec<&str> = parts
        .iter()
        .flat_map(|p| p.sentences.iter().map(|s| s.text.as_str()))
        .collect();
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

pub fn to_srt(parts: &[Part<'_>]) -> String {
    let mut out = String::new();
    let mut index = 0;
    for part in parts {
        for s in part.sentences {
            index += 1;
            out.push_str(&format!(
                "{index}\n{} --> {}\n{}\n\n",
                srt_time(part.offset_s + s.start),
                srt_time(part.offset_s + s.end),
                s.text
            ));
        }
    }
    out
}

/// `HH:MM:SS,mmm` — định dạng mốc của SubRip.
fn srt_time(seconds: f64) -> String {
    let ms = (seconds.max(0.0) * 1000.0).round() as u64;
    format!(
        "{:02}:{:02}:{:02},{:03}",
        ms / 3_600_000,
        ms / 60_000 % 60,
        ms / 1000 % 60,
        ms % 1000
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(start: f64, end: f64, text: &str) -> Sentence {
        Sentence { start, end, text: text.into() }
    }

    #[test]
    fn srt_offsets_later_files() {
        let first = [s(0.0, 2.5, "Xin chào.")];
        let second = [s(1.0, 3.0, "Bắt đầu họp.")];
        let parts = [
            Part { offset_s: 0.0, sentences: &first },
            Part { offset_s: 60.0, sentences: &second },
        ];
        assert_eq!(
            to_srt(&parts),
            "1\n00:00:00,000 --> 00:00:02,500\nXin chào.\n\n\
             2\n00:01:01,000 --> 00:01:03,000\nBắt đầu họp.\n\n"
        );
        assert_eq!(to_txt(&parts), "Xin chào.\nBắt đầu họp.\n");
    }

    #[test]
    fn srt_time_handles_hours() {
        assert_eq!(srt_time(3725.0456), "01:02:05,046");
    }
}
