//! Chép lời bằng ElevenLabs: một request cho cả tệp, hoặc cho cả phiên iPhone (`s:<phiên>`).
//!
//! Phiên iPhone là nhiều file `<phiên>#<index>`, xếp hàng chung một `runId` (`create`). Worker lấy một file ra thì nhận
//! luôn các file còn chờ của lượt, ghép audio theo thứ tự đoạn (mốc đoạn sau = tổng thời lượng các đoạn trước), gửi
//! một lần, rồi chia từ về lại từng đoạn ([`words::split_by_offsets`]) và ghi kết quả vào doc của từng file như trước:
//! câu sửa, index tìm kiếm, xuất tệp, dấu mốc, biên bản không phải đổi gì. Nhãn người nói nhất quán trong cả phiên.
//!
//! Trần giờ ElevenLabs của tháng (`ELEVENLABS_MONTHLY_HOURS`, sổ `el:<tháng>`) giữ chỗ trước khi gửi; ElevenLabs báo hết
//! quota thì đánh dấu tháng và báo chủ sản phẩm ([`report_quota_exhausted`]). Phiên còn đoạn đang tải lên thì hoãn
//! [`DEFER_S`] giây. Tiền: giây của từng đoạn cộng vào doc của đoạn đó TRƯỚC khi
//! gửi; ElevenLabs trả `audio_duration_secs` thì chỉnh lại theo đúng số đó ([`allocate_billed`]).

use super::chunking::{Sentence, SILENT_PEAK_DB};
use super::{elevenlabs, is_key_broken, record, report_key_broken, reserve_shortfall, words, Outcome, Progress, CANCELLED};
use super::{FILE_TIMEOUT_BASE_S, PAUSED};
use crate::audit;
use crate::models::{file, transcript, transcript_doc, usage, user};
use crate::state::AppState;
use mongodb::bson::{doc, DateTime};
use std::time::{Duration, Instant};

/// Phiên còn đoạn đang tải lên: hoãn lượt chừng này giây rồi xét lại.
const DEFER_S: i64 = 20;
/// Một đoạn của lượt bị xoá hoặc chép lại giữa chừng: các đoạn còn lại dừng với lỗi này.
const PARTIAL: &str = "Một đoạn của phiên vừa bị xoá hoặc thay đổi trong lúc chép. Hãy bấm Chép lời lại.";

/// Một file của lượt: doc hàng đợi và bản ghi.
struct Member {
    t: transcript::TranscriptDoc,
    f: file::FileDoc,
}

/// Kết quả gom lượt.
enum Gathered {
    Run { key: String, members: Vec<Member> },
    /// Phiên còn đoạn đang tải lên: đã hoãn.
    Deferred,
    /// File đã bị xoá (doc hàng đợi xoá theo): không ghi gì.
    Gone,
}

pub(super) async fn run(state: &AppState, first: &transcript::TranscriptDoc) {
    let started = Instant::now();
    let (key, members) = match gather(state, first).await {
        Ok(Gathered::Run { key, members }) => (key, members),
        Ok(Gathered::Deferred | Gathered::Gone) => return,
        Err(e) => {
            tracing::warn!(file = %first.file_id(), "gom lượt chép lỗi: {e}");
            record(&state.db, first.id, &first.run_id, Outcome::Failed(&e)).await;
            return;
        }
    };
    let db = &state.db;
    let user_id = first.user_id.as_str();
    match transcribe_unit(state, &members).await {
        Ok((per_file, speakers)) => {
            for (m, sentences) in members.iter().zip(&per_file) {
                let text = words::joined_text(sentences);
                record(db, m.t.id, &m.t.run_id, Outcome::Done { text: &text, sentences }).await;
            }
            if !speakers.is_empty() {
                if let Err(e) = transcript_doc::reset_speakers(db, user_id, &key, &speakers).await {
                    tracing::warn!(key = %key, "đặt nhãn người nói lỗi (đọc vẫn ra nhãn mặc định): {e:?}");
                }
            }
            let details = doc! {
                "fileId": first.file_id(),
                "files": members.len() as i64,
                "speakers": speakers.len() as i64,
                "seconds": started.elapsed().as_secs() as i64,
            };
            audit::record(state, Some(user_id), "transcript.done", details);
        }
        Err(e) if e == CANCELLED => {
            tracing::info!(key = %key, "một đoạn của lượt bị thay hoặc xoá, dừng cả lượt");
            for m in &members {
                record(db, m.t.id, &m.t.run_id, Outcome::Failed(PARTIAL)).await;
            }
        }
        Err(e) if is_key_broken(&e) => {
            report_key_broken(state, &e);
            for m in &members {
                record(db, m.t.id, &m.t.run_id, Outcome::Failed(PAUSED)).await;
            }
        }
        Err(e) => {
            tracing::warn!(key = %key, "chép lời lỗi: {e}");
            for m in &members {
                record(db, m.t.id, &m.t.run_id, Outcome::Failed(&e)).await;
            }
            audit::record(state, Some(user_id), "transcript.failed", doc! { "fileId": first.file_id(), "error": &e });
        }
    }
    // Lượt kết thúc (xong hay lỗi): câu của các đoạn đổi ⟹ dựng lại index tìm kiếm của phiên.
    crate::search::index::schedule(db, user_id, &key);
}

/// File của lượt theo thứ tự đoạn. Phiên iPhone còn đoạn đang tải lên thì hoãn cả lượt.
async fn gather(state: &AppState, first: &transcript::TranscriptDoc) -> Result<Gathered, String> {
    let db = &state.db;
    let user_id = first.user_id.as_str();
    let read_error = |e: crate::error::AppError| {
        tracing::warn!(file = %first.file_id(), "đọc lượt chép lỗi: {e:?}");
        "không đọc được bản ghi, thử lại sau".to_string()
    };
    let Some(f) = file::get_owned(db, &first.file_id(), user_id).await.map_err(read_error)? else {
        return Ok(Gathered::Gone);
    };
    let key = f.session_key().unwrap_or_else(|| format!("f:{}", first.file_id()));
    let Some(session) = key.strip_prefix("s:") else {
        return Ok(Gathered::Run { key, members: vec![Member { t: first.clone(), f }] });
    };
    match crate::recordings::segment_uploading(state, user_id, session).await {
        Ok(true) => {
            let until = DateTime::from_millis(DateTime::now().timestamp_millis() + DEFER_S * 1000);
            transcript::defer_run(db, user_id, &first.run_id, until).await.map_err(read_error)?;
            tracing::info!(key = %key, "phiên còn đoạn đang tải lên, hoãn {DEFER_S} giây");
            return Ok(Gathered::Deferred);
        }
        Ok(false) => {}
        Err(e) => tracing::warn!(key = %key, "tra đoạn đang tải lên lỗi, chép luôn: {e:?}"),
    }
    let others = transcript::claim_run_members(db, user_id, &first.run_id, first.id).await.map_err(read_error)?;
    let mut files: std::collections::HashMap<_, _> = file::list_session(db, user_id, &key)
        .await
        .map_err(read_error)?
        .into_iter()
        .filter_map(|(index, f)| Some((f.id?, (index, f))))
        .collect();
    let mut members: Vec<(u32, Member)> = std::iter::once(first.clone())
        .chain(others)
        // File không còn (vừa bị xoá): doc hàng đợi của nó xoá theo file.
        .filter_map(|t| files.remove(&t.id).map(|(index, f)| (index, Member { t, f })))
        .collect();
    members.sort_by_key(|(index, _)| *index);
    if members.is_empty() {
        return Ok(Gathered::Gone);
    }
    Ok(Gathered::Run { key, members: members.into_iter().map(|(_, m)| m).collect() })
}

/// Thời lượng thật của tệp (đo lại; `durationS` của bản ghi có thể do client báo).
async fn measure(state: &AppState, f: &file::FileDoc) -> Result<f64, String> {
    match core_split::probe_file(&state.config.ffprobe_path, &f.path).await {
        Ok(info) if info.duration_s > 0.0 => Ok(info.duration_s),
        _ => f.duration_s.filter(|d| *d > 0.0).ok_or_else(|| "không đọc được thời lượng bản ghi".to_string()),
    }
}

/// Câu của từng file (cùng thứ tự với `members`) và id người nói theo thứ tự xuất hiện.
async fn transcribe_unit(state: &AppState, members: &[Member]) -> Result<(Vec<Vec<Sentence>>, Vec<String>), String> {
    let Some(first) = members.first() else { return Ok((Vec::new(), Vec::new())) };
    // Khoá sau khi đã xếp hàng: không gửi audio nữa; lượt đóng với 0 giây đã gửi nên được hoàn hết.
    if user::find_profile(&state.db, &first.t.user_id).await.ok().flatten().is_some_and(|p| p.blocked) {
        return Err("tài khoản đã bị khoá".into());
    }
    let Some(api_key) = state.transcriber.transcribe_key() else { return Err(PAUSED.into()) };
    let mut durations = Vec::with_capacity(members.len());
    for m in members {
        let duration = measure(state, &m.f).await?;
        reserve_shortfall(state, &m.t, duration).await?;
        durations.push(duration);
    }
    let total: f64 = durations.iter().sum();
    let limit = Duration::from_secs(FILE_TIMEOUT_BASE_S + total as u64);
    tokio::time::timeout(limit, send(state, members, &durations, &api_key.0))
        .await
        .map_err(|_| "chép quá lâu nên đã dừng, hãy thử lại".to_string())?
}

/// Đặt tiến độ cho mọi file của lượt; một file không còn thuộc lượt ⟹ [`CANCELLED`].
async fn set_progress(state: &AppState, members: &[Member], pct: f64) -> Result<(), String> {
    for m in members {
        if let Ok(false) = transcript::set_progress(&state.db, m.t.id, &m.t.run_id, pct).await {
            return Err(CANCELLED.into());
        }
    }
    Ok(())
}

async fn send(
    state: &AppState,
    members: &[Member],
    durations: &[f64],
    api_key: &str,
) -> Result<(Vec<Vec<Sentence>>, Vec<String>), String> {
    set_progress(state, members, 5.0).await?;
    let paths: Vec<&str> = members.iter().map(|m| m.f.path.as_str()).collect();
    let (audio, peak) = elevenlabs::encode(&state.config.ffmpeg_path, &paths).await?;
    // Hoàn toàn im lặng: không gửi, không tính phút (im lặng bên trong một bản có lời thì vẫn gửi).
    if peak.is_some_and(|p| p < SILENT_PEAK_DB) {
        return Ok((vec![Vec::new(); members.len()], Vec::new()));
    }
    // Trần giờ ElevenLabs của tháng: giữ chỗ nguyên tử trước khi gửi, quyết toán theo số ElevenLabs tính.
    let now = chrono::Utc::now();
    let month = usage::month_key(now);
    let need: i64 = durations.iter().map(|d| d.ceil() as i64).sum();
    let ledger = |e: crate::error::AppError| {
        tracing::warn!("sổ giờ ElevenLabs lỗi: {e:?}");
        "không ghi được sổ phút, thử lại sau".to_string()
    };
    if usage::provider_exhausted(&state.db, &month).await.map_err(ledger)?
        || !usage::reserve_provider(&state.db, &month, need, state.config.elevenlabs_monthly_seconds).await.map_err(ledger)?
    {
        return Err(super::hours_exhausted_message(now));
    }
    let refund = || async {
        if let Err(e) = usage::settle_provider(&state.db, &month, -need).await {
            tracing::error!(need, "hoàn sổ giờ ElevenLabs lỗi: {e:?}");
        }
    };
    // Ghi giây sắp gửi TRƯỚC khi gửi: tiền đã chi luôn nằm trong sổ, kể cả khi tiến trình chết.
    for (m, d) in members.iter().zip(durations) {
        if let Err(e) = count_sent(state, m, d.ceil() as i64).await {
            refund().await;
            return Err(e);
        }
    }
    if let Err(e) = set_progress(state, members, 10.0).await {
        refund().await;
        return Err(e);
    }
    let total: f64 = durations.iter().sum();
    let result = match elevenlabs::transcribe(&state.http, elevenlabs::API_URL, api_key, &audio, total).await {
        Ok(result) => result,
        Err(e) => {
            // ElevenLabs đã từ chối: không tính tiền ⟹ hoàn sổ giờ. Hết thời gian chờ thì có thể đã xử lý: giữ nguyên.
            if !e.contains("quá thời gian") {
                refund().await;
            }
            if e == elevenlabs::QUOTA_EXHAUSTED {
                report_quota_exhausted(state, &month).await;
                return Err(super::hours_exhausted_message(now));
            }
            return Err(e);
        }
    };
    if let Some(billed) = result.duration_s {
        if let Err(e) = usage::settle_provider(&state.db, &month, billed.ceil() as i64 - need).await {
            tracing::error!("chỉnh sổ giờ ElevenLabs lỗi: {e:?}");
        }
        for ((m, d), share) in members.iter().zip(durations).zip(allocate_billed(billed, durations)) {
            let delta = share - d.ceil() as i64;
            if delta != 0 {
                count_sent(state, m, delta).await?;
            }
        }
    }
    Ok(split_result(result, durations))
}

/// ElevenLabs báo hết quota tháng: đánh dấu tháng (chặn gửi tới tháng sau hoặc tới khi quản trị gỡ bằng
/// `admin elevenlabs-quota --clear`), ghi audit và báo chủ sản phẩm một lần. Không phải key hỏng: không ngắt chép lời.
async fn report_quota_exhausted(state: &AppState, month: &str) {
    if usage::mark_provider_exhausted(&state.db, month).await {
        tracing::error!(month, "ElevenLabs báo hết quota tháng — dừng gửi tới tháng sau");
        audit::record(state, None, "transcript.provider_quota", doc! { "month": month });
        crate::notify::send(
            state,
            format!(
                "ElevenLabs báo hết quota tháng {month}; chép lời dừng tới tháng sau. Nạp thêm credit rồi chạy \
                 `scribe-be admin elevenlabs-quota --clear` để mở lại."
            ),
        );
    }
}

/// Từ của cả bản ghép → câu của từng file (mốc trong file) và id người nói theo thứ tự xuất hiện.
fn split_result(result: elevenlabs::Transcript, durations: &[f64]) -> (Vec<Vec<Sentence>>, Vec<String>) {
    let mut all_words = result.words;
    words::normalize_speakers(&mut all_words);
    let speakers = speaker_ids(&all_words);
    let per_file = words::split_by_offsets(all_words, &offsets(durations)).iter().map(|w| words::sentences(w)).collect();
    (per_file, speakers)
}

/// Cộng giây đã gửi vào doc của đoạn `m`; đoạn không còn thuộc lượt ⟹ [`CANCELLED`].
async fn count_sent(state: &AppState, m: &Member, seconds: i64) -> Result<(), String> {
    Progress { db: &state.db, file_id: m.t.id, run_id: &m.t.run_id }.count_sent(seconds).await
}

/// Mốc bắt đầu của từng đoạn trong bản ghép: tổng thời lượng các đoạn trước.
fn offsets(durations: &[f64]) -> Vec<f64> {
    durations
        .iter()
        .scan(0.0, |acc, d| {
            let start = *acc;
            *acc += d.max(0.0);
            Some(start)
        })
        .collect()
}

/// Id người nói theo thứ tự xuất hiện (sau [`words::normalize_speakers`]).
fn speaker_ids(all_words: &[words::Word]) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for id in all_words.iter().filter_map(|w| w.speaker.as_ref()) {
        if !ids.contains(id) {
            ids.push(id.clone());
        }
    }
    ids
}

/// Chia số giây ElevenLabs tính (`billed`, làm tròn lên) cho từng đoạn theo thời lượng: tổng đúng bằng số đã tính,
/// phần dư chia cho các đoạn có phần lẻ lớn nhất.
fn allocate_billed(billed: f64, durations: &[f64]) -> Vec<i64> {
    let target = billed.max(0.0).ceil() as i64;
    let total: f64 = durations.iter().map(|d| d.max(0.0)).sum();
    if durations.is_empty() || total <= 0.0 {
        return vec![0; durations.len()];
    }
    let exact: Vec<f64> = durations.iter().map(|d| target as f64 * d.max(0.0) / total).collect();
    let mut shares: Vec<i64> = exact.iter().map(|e| e.floor() as i64).collect();
    let mut order: Vec<usize> = (0..exact.len()).collect();
    order.sort_by(|&a, &b| (exact[b] - exact[b].floor()).total_cmp(&(exact[a] - exact[a].floor())));
    let left = target - shares.iter().sum::<i64>();
    for &i in order.iter().cycle().take(left.max(0) as usize) {
        shares[i] += 1;
    }
    shares
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_add_up_earlier_segments() {
        assert_eq!(offsets(&[60.0, 59.5, 30.0]), vec![0.0, 60.0, 119.5]);
        assert!(offsets(&[]).is_empty());
    }

    #[test]
    fn billed_seconds_are_shared_by_duration_and_add_up() {
        // ElevenLabs tính 149,2 giây cho ba đoạn 60 + 59,5 + 30 (đã gửi 60 + 60 + 30 = 150): tổng 150.
        let shares = allocate_billed(149.2, &[60.0, 59.5, 30.0]);
        assert_eq!(shares.iter().sum::<i64>(), 150);
        assert!(shares.iter().all(|s| *s >= 0));
        // Phiên 180 đoạn ~1 phút: làm tròn từng đoạn lên cộng dư tới 180 giây; chia lại theo số tính thật.
        let durations = vec![59.98; 180];
        let shares = allocate_billed(10_796.4, &durations);
        assert_eq!(shares.iter().sum::<i64>(), 10_797);
        let rounded: i64 = durations.iter().map(|d| d.ceil() as i64).sum();
        assert_eq!(rounded - shares.iter().sum::<i64>(), 3, "{rounded}");
        assert_eq!(allocate_billed(10.0, &[]), Vec::<i64>::new());
        assert_eq!(allocate_billed(10.0, &[0.0]), vec![0]);
        assert_eq!(allocate_billed(12.3, &[12.3]), vec![13]);
    }

    #[test]
    fn a_merged_session_is_split_back_into_segment_sentences() {
        let w = |text: &str, start: f64, speaker: &str| words::Word {
            text: text.into(),
            start,
            end: start + 0.4,
            speaker: Some(speaker.into()),
        };
        // Hai đoạn 60 giây + 30 giây; người nói thứ hai của nhà cung cấp là `speaker_1` nhưng xuất hiện trước.
        let result = elevenlabs::Transcript {
            words: vec![
                w("Chào", 0.5, "speaker_1"),
                w("anh.", 0.9, "speaker_1"),
                w("Chào", 2.0, "speaker_0"),
                w("em.", 2.4, "speaker_0"),
                w("Sang", 61.0, "speaker_1"),
                w("đoạn", 61.4, "speaker_1"),
                w("hai.", 61.8, "speaker_1"),
            ],
            duration_s: Some(90.0),
        };
        let (per_file, speakers) = split_result(result, &[60.0, 30.0]);
        assert_eq!(speakers, vec!["s0", "s1"]);
        assert_eq!(per_file.len(), 2);
        let first: Vec<(&str, Option<&str>)> = per_file[0].iter().map(|s| (s.text.as_str(), s.speaker.as_deref())).collect();
        assert_eq!(first, vec![("Chào anh.", Some("s0")), ("Chào em.", Some("s1"))]);
        assert_eq!(per_file[1].len(), 1);
        let second = &per_file[1][0];
        assert_eq!((second.text.as_str(), second.speaker.as_deref()), ("Sang đoạn hai.", Some("s0")));
        assert!((second.start - 1.0).abs() < 1e-9 && (second.end - 2.2).abs() < 1e-9, "mốc tính trong đoạn: {second:?}");
    }

    /// Chép thật qua ElevenLabs bằng đúng đường của server (ghép đoạn → nén → gửi → chia câu về đoạn), tốn tiền và cần
    /// mạng: `ELEVENLABS_API_KEY_FILE=<tệp key> TRANSCRIBE_SEGMENTS=<đoạn1>,<đoạn2> cargo test live_elevenlabs -- --ignored
    /// --nocapture`. Chỉ in số câu, số người nói, giây tính tiền; không in chữ hay key.
    #[tokio::test]
    #[ignore]
    async fn live_elevenlabs_transcribes_a_two_segment_session() {
        let key_file = std::env::var("ELEVENLABS_API_KEY_FILE").expect("ELEVENLABS_API_KEY_FILE");
        let key = std::fs::read_to_string(key_file).expect("đọc key");
        let paths: Vec<String> = std::env::var("TRANSCRIBE_SEGMENTS").expect("TRANSCRIBE_SEGMENTS").split(',').map(str::to_string).collect();
        let mut durations = Vec::new();
        for p in &paths {
            durations.push(core_split::probe_file("ffprobe", p).await.expect("ffprobe").duration_s);
        }
        let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
        let (audio, peak) = elevenlabs::encode("ffmpeg", &refs).await.expect("nén");
        let total: f64 = durations.iter().sum();
        let started = Instant::now();
        let result = elevenlabs::transcribe(&reqwest::Client::new(), elevenlabs::API_URL, key.trim(), &audio, total)
            .await
            .expect("chép lời");
        let billed = result.duration_s;
        let (per_file, speakers) = split_result(result, &durations);
        let counts: Vec<usize> = per_file.iter().map(Vec::len).collect();
        let usd = billed.unwrap_or(total) / 60.0 * crate::config::Provider::ElevenLabs.usd_per_minute();
        println!(
            "đoạn {:?}s, đỉnh {peak:?} dB, {} KB, câu theo đoạn {counts:?}, {} người nói, tính {billed:?}s ≈ ${usd:.4}, {:.1}s",
            durations,
            audio.len() / 1024,
            speakers.len(),
            started.elapsed().as_secs_f64()
        );
        assert!(per_file.iter().all(|s| !s.is_empty()), "mỗi đoạn có câu");
        for (sentences, d) in per_file.iter().zip(&durations) {
            assert!(sentences.iter().all(|s| s.start >= 0.0 && s.start <= *d + 0.5 && s.speaker.is_some()));
        }
    }

    #[test]
    fn speaker_ids_follow_first_appearance() {
        let w = |speaker: Option<&str>| words::Word { text: "a".into(), start: 0.0, end: 0.1, speaker: speaker.map(str::to_string) };
        assert_eq!(speaker_ids(&[w(Some("s0")), w(None), w(Some("s1")), w(Some("s0"))]), vec!["s0", "s1"]);
        assert!(speaker_ids(&[w(None)]).is_empty());
    }
}
