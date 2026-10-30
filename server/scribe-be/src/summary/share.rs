//! Gửi biên bản tới chat riêng của người dùng với bot Scribe trên chat.hoandat.com (markdown của Bot API,
//! xem `chat-next/spec/bot-api/format.md`). Chỉ chat đã liên kết qua deep link (`chat_link`), không bao
//! giờ vào nhóm. Tài khoản bị khoá không gửi được. Biên bản đã cũ (chữ đã đổi sau lúc tóm tắt) vẫn gửi
//! được, kèm dòng nhắc. Đầu đề và chữ cố định của tin theo ngôn ngữ của biên bản ([`StoredSummary::lang`]), để cả
//! tin một ngôn ngữ. Audit không ghi nội dung.

use crate::audit;
use crate::auth::UserId;
use crate::chat_link::bot::{BotApi, BotError};
use crate::error::AppError;
use crate::lang::Lang;
use crate::models::transcript_doc::StoredSummary;
use crate::models::{chat_link, user};
use crate::notify;
use crate::state::AppState;
use crate::transcribe::docs;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use mongodb::bson::doc;
use serde_json::{json, Value};
use std::time::Duration;

/// Mỗi người gửi tối đa chừng này lần mỗi giờ.
const SHARES_PER_HOUR: u32 = 10;
/// Bot API nhận tối đa 4096 ký tự sau khi đọc markdown; đếm trên chữ thô (dài hơn chữ sau khi đọc) và
/// chừa phần dư.
const MAX_MESSAGE_CHARS: usize = 3_900;
const MAX_SUMMARY_CHARS: usize = 1_800;

/// Thoát mọi ký tự có nghĩa trong markdown của Bot API (`\c` luôn là chính ký tự `c`).
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '*' | '_' | '~' | '|' | '[' | ']' | '(' | ')' | '`' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Mốc `m:ss` hoặc `h:mm:ss`.
pub fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0).floor() as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// Đường dẫn mở bản chép lời trên web (cùng dạng `transcriptPath()` của web: khoá mã hoá URI).
pub fn transcript_url(public_url: &str, key: &str) -> String {
    format!("{}/transcripts/{}", public_url.trim_end_matches('/'), key.replace(':', "%3A"))
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", cut.trim_end())
}

/// Chữ cố định của tin biên bản trong một ngôn ngữ.
struct Words {
    summary: &'static str,
    decisions: &'static str,
    actions: &'static str,
    /// Đứng trước hạn của việc ("hạn thứ Sáu").
    due: &'static str,
    open: &'static str,
    /// Dòng nhắc khi biên bản viết từ chữ cũ hơn chữ hiện tại.
    stale: &'static str,
    /// Tên bản ghi khi phiên không có tiêu đề (kèm giờ nếu biết).
    recording: &'static str,
    /// Giờ bắt đầu phiên (chrono), giờ Việt Nam.
    when: &'static str,
}

fn words(lang: Lang) -> &'static Words {
    const VI: Words = Words {
        summary: "Tóm tắt",
        decisions: "Quyết định",
        actions: "Việc cần làm",
        due: "hạn",
        open: "Mở trên Scribe",
        stale: "(biên bản có thể đã cũ)",
        recording: "Bản ghi",
        when: "%H:%M ngày %d/%m/%Y",
    };
    const EN: Words = Words {
        summary: "Summary",
        decisions: "Decisions",
        actions: "Action items",
        due: "due",
        open: "Open in Scribe",
        stale: "(these minutes may be out of date)",
        recording: "Recording",
        when: "%H:%M, %b %-d, %Y",
    };
    // Thuật ngữ khớp bản tiếng Trung của web (`web/src/i18n/messages/zh-CN`).
    const ZH: Words = Words {
        summary: "摘要",
        decisions: "决定",
        actions: "待办事项",
        due: "截止",
        open: "在 Scribe 中打开",
        stale: "（会议纪要可能已过时）",
        recording: "录音",
        when: "%Y年%-m月%-d日 %H:%M",
    };
    match lang {
        Lang::Vi => &VI,
        Lang::En => &EN,
        Lang::Zh => &ZH,
    }
}

/// Dòng cuối khi phải bớt `n` mục cho vừa tin.
fn more_items(lang: Lang, n: usize) -> String {
    match lang {
        Lang::Vi => format!("… còn {n} mục, xem đủ trên Scribe."),
        Lang::En => format!("… {n} more, see them all in Scribe."),
        Lang::Zh => format!("… 还有 {n} 项，请在 Scribe 中查看全部。"),
    }
}

/// Tin nhắn biên bản, chữ cố định theo ngôn ngữ của biên bản. Quá dài thì rút gọn tóm tắt và bớt mục cuối, kèm dòng
/// "còn N mục" trỏ về Scribe.
pub fn format_message(title: &str, when: Option<&str>, stored: &StoredSummary, url: &str, stale: bool) -> String {
    let lang = stored.language();
    let w = words(lang);
    let mut head = format!("*{}*\n", escape(title));
    if let Some(when) = when {
        head.push_str(&format!("{}\n", escape(when)));
    }
    if stale {
        head.push_str(&format!("_{}_\n", escape(w.stale)));
    }
    head.push_str(&format!("\n*{}*\n{}\n", w.summary, escape(&truncate(&stored.summary, MAX_SUMMARY_CHARS))));
    let footer = format!("\n[{}]({url})", w.open);

    let decisions: Vec<String> =
        stored.decisions.iter().map(|d| format!("• \\[{}\\] {}", clock(d.at), escape(&d.text))).collect();
    let actions: Vec<String> = stored
        .actions
        .iter()
        .map(|a| {
            let mut line = format!("• \\[{}\\] {}", clock(a.at), escape(&a.text));
            let who = [a.owner.as_deref().map(escape), a.due.as_deref().map(|d| format!("{} {}", w.due, escape(d)))];
            let who: Vec<String> = who.into_iter().flatten().collect();
            if !who.is_empty() {
                line.push_str(&format!(" — {}", who.join(", ")));
            }
            line
        })
        .collect();

    // Chừa chỗ cho dòng "còn N mục" (~60 ký tự) cùng phần đầu và chân.
    let budget = MAX_MESSAGE_CHARS.saturating_sub(head.chars().count() + footer.chars().count() + 60);
    let mut body = String::new();
    let mut used = 0;
    let mut omitted = 0;
    for (heading, lines) in [(w.decisions, &decisions), (w.actions, &actions)] {
        if lines.is_empty() {
            continue;
        }
        let heading = format!("\n*{heading}*\n");
        let mut section = String::new();
        for line in lines.iter() {
            let cost = line.chars().count() + 1 + if section.is_empty() { heading.chars().count() } else { 0 };
            if omitted > 0 || used + cost > budget {
                omitted += 1;
                continue;
            }
            if section.is_empty() {
                section.push_str(&heading);
            }
            section.push_str(line);
            section.push('\n');
            used += cost;
        }
        body.push_str(&section);
    }
    if omitted > 0 {
        body.push_str(&format!("\n{}\n", more_items(lang, omitted)));
    }
    format!("{head}{body}{footer}")
}

/// Thời điểm bắt đầu phiên theo giờ Việt Nam, viết theo `lang` để đọc trong tin.
fn vn_when(created_at: Option<&str>, lang: Lang) -> Option<String> {
    let at = chrono::DateTime::parse_from_rfc3339(created_at?).ok()?;
    Some(at.with_timezone(&crate::models::usage::vietnam()).format(words(lang).when).to_string())
}

fn conflict(code: &'static str, message: &str) -> AppError {
    AppError::Coded { status: StatusCode::CONFLICT, code, message: message.into() }
}

/// Kết quả gửi tới bot: lỗi trả client và mã `error.code` của bot khi phải gỡ liên kết (bot bị chặn hoặc
/// chat không còn: liên kết không còn dùng được).
fn after_send(sent: Result<(), BotError>) -> (Result<(), AppError>, Option<String>) {
    match sent {
        Ok(()) => (Ok(()), None),
        Err(BotError::Forbidden(code)) => (
            Err(conflict("CHAT_BLOCKED", "Không gửi được vì bot Scribe đã bị chặn trên chat. Đã gỡ liên kết, hãy liên kết lại.")),
            Some(code),
        ),
        Err(BotError::RetryAfter(_)) => (Err(AppError::TooManyRequests("chat đang bận, thử lại sau ít phút".into())), None),
        Err(e) => {
            tracing::warn!("gửi biên bản lỗi: {e}");
            (Err(AppError::Unavailable("Không gửi được sang chat, thử lại sau.".into())), None)
        }
    }
}

/// `POST /api/transcript-docs/{key}/share` — gửi biên bản tới chat riêng đã liên kết.
pub async fn share(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path(key): Path<String>,
) -> Result<Json<Value>, AppError> {
    let profile = user::find_profile(&state.db, &user.0)
        .await?
        .ok_or_else(|| AppError::Unauthorized("user không tồn tại".into()))?;
    if profile.blocked {
        return Err(AppError::Forbidden("tài khoản đã bị khoá".into()));
    }
    if !state.rate.allow_in(&format!("share:{}", user.0), SHARES_PER_HOUR, Duration::from_secs(3600)) {
        return Err(AppError::TooManyRequests(format!("mỗi giờ gửi tối đa {SHARES_PER_HOUR} lần, thử lại sau")));
    }
    let loaded = docs::load(&state, &user.0, &key).await?;
    let Some(stored) = loaded.doc.as_ref().and_then(|d| d.summary.as_ref()) else {
        return Err(conflict("NO_SUMMARY", "Chưa có biên bản. Bấm Tóm tắt trước rồi gửi."));
    };
    let Some(chat_id) = chat_link::linked_chat(&state.db, &user.0).await? else {
        return Err(conflict("CHAT_NOT_LINKED", "Chưa liên kết chat.hoandat.com."));
    };
    let token = notify::read_bot_token(&state.config).await.map_err(|e| {
        tracing::warn!("gửi biên bản: {e}");
        AppError::Unavailable("Gửi sang chat chưa được bật trên máy chủ này.".into())
    })?;
    let recording = words(stored.language()).recording;
    let when = vn_when(loaded.created_at().as_deref(), stored.language());
    let title = loaded.title(&key).or_else(|| when.as_ref().map(|w| format!("{recording} {w}"))).unwrap_or_else(|| recording.into());
    let stale = stored.source_hash != super::source_hash(&super::build_input(&loaded.timed_sentences()));
    let text = format_message(&title, when.as_deref(), stored, &transcript_url(&state.config.public_url, &key), stale);

    let sent = BotApi::new(&state.http, &state.config.notify_bot_api, &token).send(chat_id, &text, true).await;
    let (result, unlink_code) = after_send(sent);
    if let Some(code) = unlink_code {
        // Chỉ gỡ khi vẫn là chat vừa gửi: không gỡ liên kết mới vừa chen vào.
        if chat_link::unlink(&state.db, &user.0, Some(chat_id)).await? {
            audit::record(&state, Some(&user.0), "chat.unlink", doc! { "reason": "bot_forbidden", "code": &code });
        }
    }
    result?;
    audit::record(&state, Some(&user.0), "transcript.share", doc! { "key": &key, "chatId": chat_id, "stale": stale });
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::transcript_doc::{Action, Decision};
    use mongodb::bson::DateTime;

    fn stored(decisions: usize, actions: usize) -> StoredSummary {
        StoredSummary {
            model: "gpt-4.1-mini".into(),
            created_at: DateTime::now(),
            source_hash: "h".into(),
            lang: "vi".into(),
            summary: "Bàn kế hoạch *quý 4* [nháp].".into(),
            decisions: (0..decisions).map(|i| Decision { text: format!("Quyết định {i}"), at: 61.0 + i as f64 }).collect(),
            actions: (0..actions)
                .map(|i| Action { text: format!("Việc {i}_x"), owner: Some("Lan".into()), due: (i == 0).then(|| "thứ Sáu".into()), at: 3_725.0 })
                .collect(),
        }
    }

    #[test]
    fn markdown_specials_are_escaped() {
        assert_eq!(escape("a*b_c [x](y) `z` \\ ~|"), "a\\*b\\_c \\[x\\]\\(y\\) \\`z\\` \\\\ \\~\\|");
        assert_eq!(escape("Họp tuần 5/10"), "Họp tuần 5/10");
    }

    #[test]
    fn clock_and_link_formats() {
        assert_eq!((clock(0.0), clock(61.9), clock(3725.0)), ("0:00".into(), "1:01".into(), "1:02:05".into()));
        assert_eq!(transcript_url("https://scribe.hoandat.com/", "s:abc-1"), "https://scribe.hoandat.com/transcripts/s%3Aabc-1");
    }

    #[test]
    fn message_lists_summary_decisions_and_actions() {
        let text = format_message("Họp *tuần*", Some("09:00 ngày 16/10/2026"), &stored(1, 2), "https://x/transcripts/f%3A1", false);
        assert_eq!(
            text,
            "*Họp \\*tuần\\**\n09:00 ngày 16/10/2026\n\n*Tóm tắt*\nBàn kế hoạch \\*quý 4\\* \\[nháp\\].\n\
             \n*Quyết định*\n• \\[1:01\\] Quyết định 0\n\
             \n*Việc cần làm*\n• \\[1:02:05\\] Việc 0\\_x — Lan, hạn thứ Sáu\n• \\[1:02:05\\] Việc 1\\_x — Lan\n\
             \n[Mở trên Scribe](https://x/transcripts/f%3A1)"
        );
        let empty = format_message("T", None, &stored(0, 0), "u", false);
        assert!(!empty.contains("Quyết định") && !empty.contains("Việc cần làm"));
        assert!(!empty.contains("đã cũ"));
    }

    #[test]
    fn stale_minutes_carry_a_note() {
        let text = format_message("T", Some("09:00"), &stored(0, 0), "u", true);
        assert!(text.starts_with("*T*\n09:00\n_\\(biên bản có thể đã cũ\\)_\n"), "{text}");
    }

    #[test]
    fn chinese_minutes_get_chinese_headings() {
        let mut s = stored(1, 1);
        s.lang = "zh-CN".into();
        let text = format_message("周会", Some("2026年10月16日 09:00"), &s, "https://x/t", true);
        assert_eq!(
            text,
            "*周会*\n2026年10月16日 09:00\n_（会议纪要可能已过时）_\n\n*摘要*\nBàn kế hoạch \\*quý 4\\* \\[nháp\\].\n\
             \n*决定*\n• \\[1:01\\] Quyết định 0\n\
             \n*待办事项*\n• \\[1:02:05\\] Việc 0\\_x — Lan, 截止 thứ Sáu\n\
             \n[在 Scribe 中打开](https://x/t)"
        );
        s.lang = "en".into();
        let en = format_message("T", None, &s, "u", false);
        assert!(en.contains("*Summary*") && en.contains("*Action items*") && en.contains("due thứ Sáu"));
        assert!(en.ends_with("[Open in Scribe](u)"));
    }

    #[test]
    fn a_bot_refusal_unlinks_and_reports_chat_blocked() {
        let (result, unlink) = after_send(Err(BotError::Forbidden("forbidden".into())));
        assert_eq!(unlink.as_deref(), Some("forbidden"));
        let Err(AppError::Coded { status, code, .. }) = result else { panic!("phải là lỗi có mã") };
        assert_eq!((status, code), (StatusCode::CONFLICT, "CHAT_BLOCKED"));

        let (result, unlink) = after_send(Err(BotError::RetryAfter(5)));
        assert!(matches!(result, Err(AppError::TooManyRequests(_))) && unlink.is_none());
        let (result, unlink) = after_send(Err(BotError::Status(400, "bad_request".into())));
        assert!(matches!(result, Err(AppError::Unavailable(_))) && unlink.is_none());
        let (result, unlink) = after_send(Ok(()));
        assert!(result.is_ok() && unlink.is_none());
    }

    #[test]
    fn long_minutes_fit_the_bot_limit() {
        let mut s = stored(50, 50);
        s.summary = "chữ ".repeat(2_000);
        for d in &mut s.decisions {
            d.text = "quyết định dài ".repeat(10);
        }
        let text = format_message("T", None, &s, "https://x", true);
        assert!(text.chars().count() <= 4_000, "{}", text.chars().count());
        assert!(text.contains("… còn ") && text.ends_with("[Mở trên Scribe](https://x)"));
        s.lang = "zh-CN".into();
        assert!(format_message("T", None, &s, "https://x", true).contains("… 还有 "));
    }

    #[test]
    fn session_time_reads_in_vietnam_time() {
        assert_eq!(vn_when(Some("2026-10-16T02:00:00Z"), Lang::Vi).as_deref(), Some("09:00 ngày 16/10/2026"));
        assert_eq!(vn_when(Some("2026-10-16T02:00:00Z"), Lang::En).as_deref(), Some("09:00, Oct 16, 2026"));
        assert_eq!(vn_when(Some("2026-10-06T02:00:00Z"), Lang::Zh).as_deref(), Some("2026年10月6日 09:00"));
        assert_eq!(vn_when(None, Lang::Vi), None);
    }
}
