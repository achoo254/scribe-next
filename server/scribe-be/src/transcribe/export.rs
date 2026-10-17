//! Xuất transcript ra txt/srt/vtt (lời chép, phụ đề) và md/docx (tài liệu cuộc họp). Một phiên ghi
//! iPhone là nhiều file (mỗi đoạn một file): ghép theo thứ tự, mốc của file sau cộng dồn thời lượng các
//! file trước. Xuất bản dịch: người gọi thay chữ câu bằng bản dịch; md/docx dùng nhãn mục theo ngôn ngữ
//! ([`Labels`]), biên bản giữ nguyên ngôn ngữ gốc.
//!
//! Người nói: người gọi thay id người nói của câu (`Sentence::speaker`) bằng tên hiển thị. TXT/MD/DOCX thêm "Tên: "
//! ở chỗ đổi người nói; SRT thêm "Tên: " mỗi cue; VTT dùng thẻ giọng `<v Tên>`.

use super::chunking::Sentence;
use crate::models::transcript_doc::{Bookmark, StoredSummary};
use docx_rs::{Docx, Paragraph, Run, RunFonts, Style, StyleType};

/// Định dạng tệp xuất (`format=` của `GET /api/transcripts/export`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Txt,
    Srt,
    Vtt,
    Md,
    Docx,
}

impl Format {
    pub fn parse(raw: &str) -> Option<Format> {
        match raw {
            "txt" => Some(Format::Txt),
            "srt" => Some(Format::Srt),
            "vtt" => Some(Format::Vtt),
            "md" => Some(Format::Md),
            "docx" => Some(Format::Docx),
            _ => None,
        }
    }

    pub fn ext(self) -> &'static str {
        match self {
            Format::Txt => "txt",
            Format::Srt => "srt",
            Format::Vtt => "vtt",
            Format::Md => "md",
            Format::Docx => "docx",
        }
    }

    pub fn mime(self) -> &'static str {
        match self {
            Format::Txt => "text/plain; charset=utf-8",
            Format::Srt => "application/x-subrip; charset=utf-8",
            Format::Vtt => "text/vtt; charset=utf-8",
            Format::Md => "text/markdown; charset=utf-8",
            Format::Docx => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        }
    }
}

/// Chữ cố định của tài liệu cuộc họp theo ngôn ngữ.
#[derive(Debug, PartialEq)]
pub struct Labels {
    /// Tiêu đề khi phiên không có tên.
    pub untitled: &'static str,
    pub duration: &'static str,
    pub summary: &'static str,
    pub decisions: &'static str,
    pub actions: &'static str,
    pub owner: &'static str,
    pub due: &'static str,
    pub bookmarks: &'static str,
    /// Tên dấu mốc không có nhãn, kèm số thứ tự.
    pub bookmark: &'static str,
    pub transcript: &'static str,
}

pub const VI: Labels = Labels {
    untitled: "Bản chép lời",
    duration: "Thời lượng",
    summary: "Tóm tắt",
    decisions: "Quyết định",
    actions: "Việc cần làm",
    owner: "Người làm",
    due: "Hạn",
    bookmarks: "Dấu mốc",
    bookmark: "Dấu mốc",
    transcript: "Lời chép",
};

pub const EN: Labels = Labels {
    untitled: "Transcript",
    duration: "Duration",
    summary: "Summary",
    decisions: "Decisions",
    actions: "Action items",
    owner: "Owner",
    due: "Due",
    bookmarks: "Bookmarks",
    bookmark: "Bookmark",
    transcript: "Transcript",
};

/// Transcript của một file kèm mốc bắt đầu của file đó trong bản ghép.
pub struct Part<'a> {
    pub offset_s: f64,
    pub sentences: &'a [Sentence],
}

/// Phần đầu và phần người dùng tạo của tài liệu cuộc họp (md/docx).
pub struct Meeting<'a> {
    pub title: &'a str,
    /// Lúc bắt đầu ghi, đã định dạng giờ Việt Nam.
    pub started: Option<String>,
    pub duration_s: f64,
    pub summary: Option<&'a StoredSummary>,
    /// Theo thứ tự thời gian.
    pub bookmarks: &'a [Bookmark],
    pub labels: &'a Labels,
}

/// Câu theo thứ tự kèm mốc bắt đầu của file chứa nó và tên người nói khi người nói đổi so với câu trước.
fn with_speaker_changes<'a>(parts: &'a [Part<'a>]) -> Vec<(f64, &'a Sentence, Option<&'a str>)> {
    let mut previous: Option<&str> = None;
    parts
        .iter()
        .flat_map(|p| p.sentences.iter().map(move |s| (p.offset_s, s)))
        .map(|(offset, s)| {
            let speaker = s.speaker.as_deref().map(str::trim).filter(|n| !n.is_empty());
            let changed = speaker.filter(|_| speaker != previous);
            previous = speaker;
            (offset, s, changed)
        })
        .collect()
}

/// `Tên: câu` khi có tên.
fn prefixed(name: Option<&str>, text: &str) -> String {
    match name {
        Some(name) => format!("{}: {text}", one_line(name)),
        None => text.to_string(),
    }
}

pub fn to_txt(parts: &[Part<'_>]) -> String {
    let lines: Vec<String> = with_speaker_changes(parts).into_iter().map(|(_, s, change)| prefixed(change, &s.text)).collect();
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
                cue_time(part.offset_s + s.start, ','),
                cue_time(part.offset_s + s.end, ','),
                prefixed(s.speaker.as_deref().map(str::trim).filter(|n| !n.is_empty()), &s.text)
            ));
        }
    }
    out
}

/// WebVTT: như SRT nhưng có dòng `WEBVTT` đầu tệp và mốc dùng dấu `.`. Câu rỗng bị bỏ (cue rỗng vô nghĩa,
/// và dòng trống trong cue sẽ kết thúc cue sớm).
pub fn to_vtt(parts: &[Part<'_>]) -> String {
    let mut out = String::from("WEBVTT\n\n");
    for part in parts {
        for s in part.sentences {
            let text = cue_text(&s.text);
            if text.is_empty() {
                continue;
            }
            let text = match s.speaker.as_deref().map(voice_name).filter(|n| !n.is_empty()) {
                Some(name) => format!("<v {name}>{text}"),
                None => text,
            };
            out.push_str(&format!(
                "{} --> {}\n{text}\n\n",
                cue_time(part.offset_s + s.start, '.'),
                cue_time(part.offset_s + s.end, '.'),
            ));
        }
    }
    out
}

/// Chữ một cue VTT: một dòng, và `-->` không được xuất hiện trong nội dung cue.
fn cue_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").replace("-->", "→")
}

/// Tên trong thẻ giọng VTT: một dòng, không có `<`, `>` (kết thúc thẻ), `&` viết thành thực thể.
fn voice_name(name: &str) -> String {
    one_line(name).replace(['<', '>'], "").replace('&', "&amp;").trim().to_string()
}

/// `HH:MM:SS,mmm` (SubRip) hoặc `HH:MM:SS.mmm` (WebVTT).
fn cue_time(seconds: f64, separator: char) -> String {
    let ms = (seconds.max(0.0) * 1000.0).round() as u64;
    format!(
        "{:02}:{:02}:{:02}{separator}{:03}",
        ms / 3_600_000,
        ms / 60_000 % 60,
        ms / 1000 % 60,
        ms % 1000
    )
}

/// Mốc trong tài liệu: `mm:ss`, từ một giờ trở lên `h:mm:ss`.
fn clock(seconds: f64) -> String {
    let total = seconds.max(0.0).floor() as u64;
    let (h, m, s) = (total / 3600, total / 60 % 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

/// Một khối của tài liệu cuộc họp; md và docx cùng dựng từ danh sách này nên luôn cùng nội dung.
#[derive(Debug, PartialEq)]
enum Block {
    Title(String),
    Meta(String),
    Heading(&'static str),
    Paragraph(String),
    /// Mục danh sách (quyết định, việc cần làm, dấu mốc), mốc giây tính từ đầu phiên.
    Item { at: f64, text: String },
    /// Một câu lời chép; `speaker` chỉ có ở câu mà người nói đổi.
    Line { at: f64, speaker: Option<String>, text: String },
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Thứ tự: tiêu đề, giờ và thời lượng, tóm tắt, quyết định, việc cần làm, dấu mốc, lời chép. Mục trống bị bỏ.
fn outline(meeting: &Meeting<'_>, parts: &[Part<'_>]) -> Vec<Block> {
    let labels = meeting.labels;
    let mut blocks = vec![Block::Title(one_line(meeting.title))];
    let mut meta = Vec::new();
    if let Some(started) = &meeting.started {
        meta.push(started.clone());
    }
    if meeting.duration_s > 0.0 {
        meta.push(format!("{} {}", labels.duration, clock(meeting.duration_s)));
    }
    if !meta.is_empty() {
        blocks.push(Block::Meta(meta.join(" · ")));
    }

    if let Some(summary) = meeting.summary {
        let paragraphs: Vec<Block> = summary
            .summary
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(|l| Block::Paragraph(l.to_string()))
            .collect();
        if !paragraphs.is_empty() {
            blocks.push(Block::Heading(labels.summary));
            blocks.extend(paragraphs);
        }
        let decisions: Vec<Block> = summary
            .decisions
            .iter()
            .filter(|d| !d.text.trim().is_empty())
            .map(|d| Block::Item { at: d.at, text: one_line(&d.text) })
            .collect();
        if !decisions.is_empty() {
            blocks.push(Block::Heading(labels.decisions));
            blocks.extend(decisions);
        }
        let actions: Vec<Block> = summary
            .actions
            .iter()
            .filter(|a| !a.text.trim().is_empty())
            .map(|a| {
                let mut details = Vec::new();
                if let Some(owner) = a.owner.as_deref().map(one_line).filter(|o| !o.is_empty()) {
                    details.push(format!("{}: {owner}", labels.owner));
                }
                if let Some(due) = a.due.as_deref().map(one_line).filter(|d| !d.is_empty()) {
                    details.push(format!("{}: {due}", labels.due));
                }
                let text = one_line(&a.text);
                let text = if details.is_empty() { text } else { format!("{text} ({})", details.join("; ")) };
                Block::Item { at: a.at, text }
            })
            .collect();
        if !actions.is_empty() {
            blocks.push(Block::Heading(labels.actions));
            blocks.extend(actions);
        }
    }

    if !meeting.bookmarks.is_empty() {
        blocks.push(Block::Heading(labels.bookmarks));
        blocks.extend(meeting.bookmarks.iter().enumerate().map(|(i, b)| {
            let label = b.label.as_deref().map(one_line).filter(|l| !l.is_empty());
            Block::Item { at: b.at, text: label.unwrap_or_else(|| format!("{} {}", labels.bookmark, i + 1)) }
        }));
    }

    let lines: Vec<Block> = with_speaker_changes(parts)
        .into_iter()
        .map(|(offset, s, change)| (offset + s.start, change.map(one_line), one_line(&s.text)))
        .filter(|(_, _, text)| !text.is_empty())
        .map(|(at, speaker, text)| Block::Line { at, speaker, text })
        .collect();
    if !lines.is_empty() {
        blocks.push(Block::Heading(labels.transcript));
        blocks.extend(lines);
    }
    blocks
}

pub fn to_md(meeting: &Meeting<'_>, parts: &[Part<'_>]) -> String {
    let rendered: Vec<String> = outline(meeting, parts)
        .into_iter()
        .map(|block| match block {
            Block::Title(text) => format!("# {text}"),
            Block::Meta(text) => format!("_{text}_"),
            Block::Heading(text) => format!("## {text}"),
            Block::Paragraph(text) => text,
            Block::Item { at, text } => format!("- [{}] {text}", clock(at)),
            Block::Line { at, speaker, text } => format!("[{}] {}", clock(at), prefixed(speaker.as_deref(), &text)),
        })
        .collect();
    let mut out = rendered.join("\n\n");
    out.push('\n');
    out
}

/// Phông hiển thị đủ dấu tiếng Việt trên Word, LibreOffice và Pages.
const DOCX_FONT: &str = "Arial";
/// Màu chữ mốc thời gian và dòng giờ/thời lượng (xám).
const MUTED: &str = "6B7280";

pub fn to_docx(meeting: &Meeting<'_>, parts: &[Part<'_>]) -> Result<Vec<u8>, String> {
    let fonts = || RunFonts::new().ascii(DOCX_FONT).hi_ansi(DOCX_FONT).cs(DOCX_FONT).east_asia(DOCX_FONT);
    // Cỡ chữ docx tính theo nửa point: 22 = 11pt.
    let mut docx = Docx::new()
        .default_fonts(fonts())
        .default_size(22)
        .add_style(Style::new("Title", StyleType::Paragraph).name("Title").size(36).bold())
        .add_style(Style::new("Heading1", StyleType::Paragraph).name("heading 1").size(28).bold().outline_lvl(0));
    let stamp = |at: f64| Run::new().add_text(format!("[{}] ", clock(at))).color(MUTED);
    for block in outline(meeting, parts) {
        let paragraph = match block {
            Block::Title(text) => Paragraph::new().style("Title").add_run(Run::new().add_text(text)),
            Block::Meta(text) => Paragraph::new().add_run(Run::new().add_text(text).color(MUTED)),
            Block::Heading(text) => Paragraph::new().style("Heading1").add_run(Run::new().add_text(text)),
            Block::Paragraph(text) => Paragraph::new().add_run(Run::new().add_text(text)),
            Block::Item { at, text } => Paragraph::new()
                .add_run(Run::new().add_text("• "))
                .add_run(stamp(at))
                .add_run(Run::new().add_text(text)),
            Block::Line { at, speaker, text } => {
                let line = Paragraph::new().add_run(stamp(at));
                let line = match speaker {
                    Some(name) => line.add_run(Run::new().add_text(format!("{name}: ")).bold()),
                    None => line,
                };
                line.add_run(Run::new().add_text(text))
            }
        };
        docx = docx.add_paragraph(paragraph);
    }
    let mut out = std::io::Cursor::new(Vec::new());
    docx.pack(&mut out).map_err(|e| format!("tạo docx: {e}"))?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::transcript_doc::{Action, Decision};
    use mongodb::bson::DateTime;
    use std::io::Read;

    fn s(start: f64, end: f64, text: &str) -> Sentence {
        Sentence { start, end, text: text.into(), speaker: None }
    }

    fn summary() -> StoredSummary {
        StoredSummary {
            model: "m".into(),
            created_at: DateTime::now(),
            source_hash: "h".into(),
            summary: "Chốt giá và lịch giao hàng.\n\nCòn chờ báo giá vận chuyển.".into(),
            decisions: vec![Decision { text: "Giá 5 triệu".into(), at: 70.0 }],
            actions: vec![
                Action { text: "Gửi hợp đồng".into(), owner: Some("Lan".into()), due: Some("thứ Sáu".into()), at: 3725.0 },
                Action { text: "Gọi kho".into(), owner: None, due: None, at: 5.0 },
            ],
        }
    }

    fn bookmarks() -> Vec<Bookmark> {
        vec![
            Bookmark { id: "a".into(), at: 12.0, label: Some("Ý quan trọng".into()) },
            Bookmark { id: "b".into(), at: 30.0, label: None },
        ]
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

    fn said(start: f64, text: &str, speaker: &str) -> Sentence {
        Sentence { speaker: Some(speaker.into()), ..s(start, start + 2.0, text) }
    }

    #[test]
    fn speaker_names_mark_changes_in_text_and_every_subtitle_cue() {
        let first = [said(0.0, "Chào anh.", "Lan"), said(2.0, "Bắt đầu nhé.", "Lan")];
        let second = [said(1.0, "Vâng.", "Người 2"), said(3.0, "Em nói tiếp.", "Lan")];
        let parts = [Part { offset_s: 0.0, sentences: &first }, Part { offset_s: 60.0, sentences: &second }];
        assert_eq!(to_txt(&parts), "Lan: Chào anh.\nBắt đầu nhé.\nNgười 2: Vâng.\nLan: Em nói tiếp.\n");
        let srt = to_srt(&parts);
        assert!(srt.starts_with("1\n00:00:00,000 --> 00:00:02,000\nLan: Chào anh.\n\n2\n00:00:02,000 --> 00:00:04,000\nLan: Bắt đầu nhé."));
        assert!(srt.contains("00:01:01,000 --> 00:01:03,000\nNgười 2: Vâng."));
        let vtt = to_vtt(&parts);
        assert!(vtt.contains("00:00:00.000 --> 00:00:02.000\n<v Lan>Chào anh.\n"));
        assert!(vtt.contains("<v Người 2>Vâng."));
        // Câu không có người nói (bản OpenAI) giữ nguyên.
        let plain = [s(0.0, 1.0, "Xin chào.")];
        let parts = [Part { offset_s: 0.0, sentences: &plain }];
        assert_eq!(to_txt(&parts), "Xin chào.\n");
        assert!(to_vtt(&parts).contains("\nXin chào.\n"));
        assert_eq!(voice_name("A<b>&c\nd"), "Ab&amp;c d");
    }

    #[test]
    fn meeting_documents_name_the_speaker_at_each_change() {
        let sentences = [said(1.0, "Mở đầu.", "Lan"), said(3.0, "Tiếp.", "Lan"), said(70.0, "Đồng ý.", "Anh Minh")];
        let parts = [Part { offset_s: 0.0, sentences: &sentences }];
        let meeting = Meeting { title: "Họp", started: None, duration_s: 0.0, summary: None, bookmarks: &[], labels: &VI };
        assert_eq!(
            to_md(&meeting, &parts),
            "# Họp\n\n## Lời chép\n\n[00:01] Lan: Mở đầu.\n\n[00:03] Tiếp.\n\n[01:10] Anh Minh: Đồng ý.\n"
        );
        let bytes = to_docx(&meeting, &parts).expect("tạo docx");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("zip");
        let mut xml = String::new();
        archive.by_name("word/document.xml").expect("document.xml").read_to_string(&mut xml).expect("đọc");
        assert!(xml.contains("Anh Minh: ") && xml.contains("Lan: ") && xml.contains("Tiếp."));
    }

    #[test]
    fn formats_parse_with_their_extension_and_type() {
        for raw in ["txt", "srt", "vtt", "md", "docx"] {
            assert_eq!(Format::parse(raw).map(Format::ext), Some(raw));
        }
        assert_eq!(Format::parse("pdf"), None);
        assert_eq!(Format::Vtt.mime(), "text/vtt; charset=utf-8");
        assert_eq!(
            Format::Docx.mime(),
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        );
    }

    #[test]
    fn cue_time_handles_hours() {
        assert_eq!(cue_time(3725.0456, ','), "01:02:05,046");
        assert_eq!(cue_time(3725.0456, '.'), "01:02:05.046");
    }

    #[test]
    fn vtt_has_header_dot_timestamps_and_skips_empty_cues() {
        let first = [s(0.0, 2.5, "Xin chào."), s(2.5, 3.0, "  ")];
        let second = [s(1.0, 3.0, "Một\n\ndòng --> hai")];
        let parts = [
            Part { offset_s: 0.0, sentences: &first },
            Part { offset_s: 3600.0, sentences: &second },
        ];
        assert_eq!(
            to_vtt(&parts),
            "WEBVTT\n\n\
             00:00:00.000 --> 00:00:02.500\nXin chào.\n\n\
             01:00:01.000 --> 01:00:03.000\nMột dòng → hai\n\n"
        );
    }

    #[test]
    fn clock_switches_to_hours() {
        assert_eq!(clock(5.9), "00:05");
        assert_eq!(clock(754.0), "12:34");
        assert_eq!(clock(3725.0), "1:02:05");
    }

    #[test]
    fn md_lists_sections_in_order() {
        let first = [s(1.0, 2.0, "Mở đầu."), s(2.0, 3.0, " ")];
        let second = [s(10.0, 12.0, "Chốt giá 5 triệu.")];
        let parts = [
            Part { offset_s: 0.0, sentences: &first },
            Part { offset_s: 60.0, sentences: &second },
        ];
        let stored = summary();
        let marks = bookmarks();
        let meeting = Meeting {
            title: "Họp\nkhách hàng",
            started: Some("09:30 17/10/2026".into()),
            duration_s: 3730.0,
            summary: Some(&stored),
            bookmarks: &marks,
            labels: &VI,
        };
        assert_eq!(
            to_md(&meeting, &parts),
            "# Họp khách hàng\n\n\
             _09:30 17/10/2026 · Thời lượng 1:02:10_\n\n\
             ## Tóm tắt\n\n\
             Chốt giá và lịch giao hàng.\n\n\
             Còn chờ báo giá vận chuyển.\n\n\
             ## Quyết định\n\n\
             - [01:10] Giá 5 triệu\n\n\
             ## Việc cần làm\n\n\
             - [1:02:05] Gửi hợp đồng (Người làm: Lan; Hạn: thứ Sáu)\n\n\
             - [00:05] Gọi kho\n\n\
             ## Dấu mốc\n\n\
             - [00:12] Ý quan trọng\n\n\
             - [00:30] Dấu mốc 2\n\n\
             ## Lời chép\n\n\
             [00:01] Mở đầu.\n\n\
             [01:10] Chốt giá 5 triệu.\n"
        );
    }

    #[test]
    fn md_omits_empty_sections() {
        let sentences = [s(0.0, 1.0, "Xin chào.")];
        let parts = [Part { offset_s: 0.0, sentences: &sentences }];
        let meeting = Meeting { title: "Ghi âm", started: None, duration_s: 0.0, summary: None, bookmarks: &[], labels: &VI };
        assert_eq!(to_md(&meeting, &parts), "# Ghi âm\n\n## Lời chép\n\n[00:00] Xin chào.\n");
    }

    #[test]
    fn md_of_a_translation_uses_english_headings_and_keeps_the_minutes() {
        let sentences = [s(1.0, 2.0, "Opening."), s(70.0, 72.0, "Price agreed at 5 million.")];
        let parts = [Part { offset_s: 0.0, sentences: &sentences }];
        let stored = summary();
        let marks = bookmarks();
        let meeting = Meeting {
            title: "Họp khách hàng",
            started: Some("09:30 17/10/2026".into()),
            duration_s: 75.0,
            summary: Some(&stored),
            bookmarks: &marks,
            labels: &EN,
        };
        let md = to_md(&meeting, &parts);
        for heading in ["## Summary", "## Decisions", "## Action items", "## Bookmarks", "## Transcript", "Duration 01:15"] {
            assert!(md.contains(heading), "thiếu {heading:?}");
        }
        // Biên bản giữ nguyên ngôn ngữ gốc; chỉ nhãn quanh nó theo ngôn ngữ xuất.
        assert!(md.contains("- [1:02:05] Gửi hợp đồng (Owner: Lan; Due: thứ Sáu)"));
        assert!(md.contains("- [00:30] Bookmark 2"));
        assert!(md.contains("[01:10] Price agreed at 5 million."));
        assert!(!md.contains("Lời chép") && !md.contains("Tóm tắt"));
    }

    #[test]
    fn docx_is_a_zip_with_the_vietnamese_text() {
        let sentences = [s(65.0, 70.0, "Chúng ta thống nhất phương án ưu tiên.")];
        let parts = [Part { offset_s: 0.0, sentences: &sentences }];
        let stored = summary();
        let marks = bookmarks();
        let meeting = Meeting {
            title: "Họp đầu tuần",
            started: Some("09:30 17/10/2026".into()),
            duration_s: 70.0,
            summary: Some(&stored),
            bookmarks: &marks,
            labels: &VI,
        };
        let bytes = to_docx(&meeting, &parts).expect("tạo docx");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("docx là zip hợp lệ");
        let mut xml = String::new();
        archive
            .by_name("word/document.xml")
            .expect("có word/document.xml")
            .read_to_string(&mut xml)
            .expect("đọc document.xml");
        for text in ["Họp đầu tuần", "Chúng ta thống nhất phương án ưu tiên.", "[01:05] ", "Ý quan trọng", "Việc cần làm"] {
            assert!(xml.contains(text), "thiếu {text:?}");
        }
        assert!(xml.contains(MUTED), "mốc thời gian phải có màu xám");
        assert!(archive.by_name("[Content_Types].xml").is_ok());
    }
}
