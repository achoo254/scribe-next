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
use super::{elevenlabs, is_key_broken, record, report_key_broken, reserve_shortfall, words, Outcome, CANCELLED};
use super::{FILE_TIMEOUT_BASE_S, PAUSED};
use crate::audit;
use crate::models::{file, transcript, transcript_doc, usage, user};
use crate::state::AppState;
use mongodb::bson::{doc, oid::ObjectId, DateTime};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

/// Phiên còn đoạn đang tải lên: hoãn lượt chừng này giây rồi xét lại.
const DEFER_S: i64 = 20;
/// Mọi đoạn của lượt bị xoá hoặc chép lại trước khi gửi.
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

/// Kết quả một lượt: đoạn chép xong, đoạn hỏng riêng (không đọc được, không đủ phút), và lỗi chung áp cho mọi đoạn
/// chưa có kết quả.
#[derive(Default)]
struct Outcomes {
    /// `(chỉ số trong members, câu)`.
    done: Vec<(usize, Vec<Sentence>)>,
    failed: Vec<(usize, String)>,
    speakers: Vec<String>,
    error: Option<String>,
}

/// Chép quá lâu: dừng, khỏi chặn hàng đợi.
const TOO_LONG: &str = "chép quá lâu nên đã dừng, hãy thử lại";
/// Sổ phút hỏng giữa chừng.
const LEDGER_FAILED: &str = "không ghi được sổ phút, thử lại sau";

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
    let out = transcribe_unit(state, &members).await;
    let mut recorded = vec![false; members.len()];
    for (i, sentences) in &out.done {
        let m = &members[*i];
        let text = words::joined_text(sentences);
        // Đoạn bị xoá hay chép lại giữa chừng: ghi không khớp `runId` nên không đụng gì.
        record(db, m.t.id, &m.t.run_id, Outcome::Done { text: &text, sentences }).await;
        recorded[*i] = true;
    }
    for (i, e) in &out.failed {
        let m = &members[*i];
        tracing::warn!(file = %m.t.file_id(), "đoạn không chép được: {e}");
        record(db, m.t.id, &m.t.run_id, Outcome::Failed(e)).await;
        recorded[*i] = true;
    }
    if let Some(e) = &out.error {
        let shown = if is_key_broken(e) {
            report_key_broken(state, e);
            PAUSED
        } else if e == CANCELLED {
            PARTIAL
        } else {
            tracing::warn!(key = %key, "chép lời lỗi: {e}");
            audit::record(state, Some(user_id), "transcript.failed", doc! { "fileId": first.file_id(), "error": e });
            e.as_str()
        };
        for (m, _) in members.iter().zip(&recorded).filter(|(_, done)| !**done) {
            record(db, m.t.id, &m.t.run_id, Outcome::Failed(shown)).await;
        }
    }
    if !out.done.is_empty() {
        if !out.speakers.is_empty() {
            if let Err(e) = transcript_doc::reset_speakers(db, user_id, &key, &out.speakers).await {
                tracing::warn!(key = %key, "đặt nhãn người nói lỗi (đọc vẫn ra nhãn mặc định): {e:?}");
            }
        }
        let details = doc! {
            "fileId": first.file_id(),
            "files": out.done.len() as i64,
            "failedFiles": out.failed.len() as i64,
            "speakers": out.speakers.len() as i64,
            "seconds": started.elapsed().as_secs() as i64,
        };
        audit::record(state, Some(user_id), "transcript.done", details);
    }
    // Lượt kết thúc (xong hay lỗi): câu của các đoạn đổi ⟹ dựng lại index tìm kiếm của phiên.
    crate::search::index::schedule(db, user_id, &key);
}

/// File của lượt theo thứ tự đoạn. Phiên iPhone còn đoạn đang tải lên thì hoãn cả lượt; hết hoãn thì đoạn tải lên
/// sau lúc xếp hàng được thêm vào cùng lượt ([`add_late_segments`]).
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
    let mut files: HashMap<ObjectId, (u32, file::FileDoc)> = file::list_session(db, user_id, &key)
        .await
        .map_err(read_error)?
        .into_iter()
        .filter_map(|(index, f)| Some((f.id?, (index, f))))
        .collect();
    let session_ids: Vec<ObjectId> = files.keys().copied().collect();
    match add_late_segments(db, first, &session_ids).await {
        Ok(0) => {}
        Ok(added) => tracing::info!(key = %key, added, "thêm đoạn tải lên sau lúc xếp hàng vào lượt"),
        Err(e) => tracing::warn!(key = %key, "thêm đoạn tải lên muộn lỗi, chép phần đã xếp: {e:?}"),
    }
    let others = transcript::claim_run_members(db, user_id, &first.run_id, first.id).await.map_err(read_error)?;
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

/// Đoạn của phiên chưa từng được xếp chép (tải lên sau lúc bấm, ví dụ trong lúc lượt bị hoãn): xếp vào lượt của
/// `first` (cùng `runId`, cùng sổ phút; phút giữ lúc chạy như mọi đoạn đo lại dài hơn phần đã giữ), để cả phiên đi
/// trong một request và lần bấm sau không phải chép lại cả phiên. Trả số đoạn đã thêm.
async fn add_late_segments(
    db: &mongodb::Database,
    first: &transcript::TranscriptDoc,
    session_ids: &[ObjectId],
) -> Result<usize, crate::error::AppError> {
    let Some(charge) = first.charge() else { return Ok(0) };
    let in_run: HashSet<ObjectId> = transcript::run_file_ids(db, &first.user_id, &first.run_id).await?;
    let missing: Vec<ObjectId> = session_ids.iter().copied().filter(|id| !in_run.contains(id)).collect();
    if missing.is_empty() {
        return Ok(0);
    }
    let known: HashSet<ObjectId> = transcript::statuses_for_files(db, &first.user_id, &missing)
        .await?
        .into_iter()
        .map(|r| r.file_id)
        .collect();
    let mut added = 0;
    for id in missing.into_iter().filter(|id| !known.contains(id)) {
        if transcript::enqueue(db, id, &first.user_id, &first.run_id, &first.model, &charge, 0).await? {
            added += 1;
        }
    }
    Ok(added)
}

/// Thời lượng thật của tệp (đo lại; `durationS` của bản ghi có thể do client báo, dùng khi không đo được).
async fn measure_file(ffprobe: &str, path: &str, stored: Option<f64>) -> Result<f64, String> {
    match core_split::probe_file(ffprobe, path).await {
        Ok(info) if info.duration_s > 0.0 => Ok(info.duration_s),
        _ => stored.filter(|d| *d > 0.0).ok_or_else(|| "không đọc được thời lượng bản ghi".to_string()),
    }
}

/// Đo và giữ phút từng đoạn (đoạn hỏng thì chỉ đoạn đó lỗi), rồi ghép và gửi các đoạn còn lại.
async fn transcribe_unit(state: &AppState, members: &[Member]) -> Outcomes {
    let mut out = Outcomes::default();
    let Some(first) = members.first() else { return out };
    // Khoá sau khi đã xếp hàng: không gửi audio nữa; lượt đóng với 0 giây đã gửi nên được hoàn hết.
    if user::find_profile(&state.db, &first.t.user_id).await.ok().flatten().is_some_and(|p| p.blocked) {
        out.error = Some("tài khoản đã bị khoá".into());
        return out;
    }
    let Some(api_key) = state.transcriber.transcribe_key() else {
        out.error = Some(PAUSED.into());
        return out;
    };
    let mut live: Vec<(usize, f64)> = Vec::with_capacity(members.len());
    for (i, m) in members.iter().enumerate() {
        let duration = match measure_file(&state.config.ffprobe_path, &m.f.path, m.f.duration_s).await {
            Ok(d) => d,
            Err(e) => {
                out.failed.push((i, e));
                continue;
            }
        };
        match reserve_shortfall(state, &m.t, duration).await {
            Ok(()) => live.push((i, duration)),
            // Đoạn vừa bị xoá hay chép lại: bỏ khỏi lượt.
            Err(e) if e == CANCELLED => {}
            Err(e) => out.failed.push((i, e)),
        }
    }
    if live.is_empty() {
        return out;
    }
    let total: f64 = live.iter().map(|(_, d)| d).sum();
    let limit = Duration::from_secs(FILE_TIMEOUT_BASE_S + total as u64);
    let run_id = first.t.run_id.clone();
    if tokio::time::timeout(limit, send(state, members, live, &api_key.0, &mut out)).await.is_err() {
        // Có thể ElevenLabs đã xử lý: giữ nguyên giây đã tính, chỉ bỏ ghi chú giữ chỗ.
        usage::release_provider_hold(&state.db, &run_id).await;
        out.error.get_or_insert_with(|| TOO_LONG.to_string());
    }
    out
}

/// Ghép các đoạn `live` (`(chỉ số, thời lượng)` theo thứ tự), giữ giờ ElevenLabs, ghi giây đã gửi, gửi một lần rồi
/// chia câu về từng đoạn. ElevenLabs từ chối chắc chắn ⟹ hoàn hết: giây đã ghi của từng đoạn (sổ phút người dùng và
/// trần USD quyết toán theo đó) và giờ ElevenLabs.
async fn send(state: &AppState, members: &[Member], live: Vec<(usize, f64)>, api_key: &str, out: &mut Outcomes) {
    let db = &state.db;
    // Đoạn bị xoá hay chép lại từ lúc nhận: bỏ khỏi bản ghép.
    let mut kept = Vec::with_capacity(live.len());
    for (i, d) in live {
        let m = &members[i];
        if !matches!(transcript::set_progress(db, m.t.id, &m.t.run_id, 5.0).await, Ok(false)) {
            kept.push((i, d));
        }
    }
    let live = kept;
    let Some(&(lead, _)) = live.first() else { return };
    let run_id = members[lead].t.run_id.as_str();
    let inputs: Vec<(&str, f64)> = live.iter().map(|(i, d)| (members[*i].f.path.as_str(), *d)).collect();
    let (audio, peak) = match elevenlabs::encode(&state.config.ffmpeg_path, &inputs).await {
        Ok(encoded) => encoded,
        Err(e) => {
            out.error = Some(e);
            return;
        }
    };
    // Hoàn toàn im lặng: không gửi, không tính phút (im lặng bên trong một bản có lời thì vẫn gửi).
    if peak.is_some_and(|p| p < SILENT_PEAK_DB) {
        out.done.extend(live.iter().map(|(i, _)| (*i, Vec::new())));
        return;
    }
    // Trần giờ ElevenLabs của tháng: giữ chỗ nguyên tử trước khi gửi, quyết toán theo số ElevenLabs tính.
    let now = chrono::Utc::now();
    let month = usage::month_key(now);
    let need: i64 = live.iter().map(|(_, d)| d.ceil() as i64).sum();
    let limit = state.config.elevenlabs_monthly_seconds;
    let reserved = match usage::provider_exhausted(db, &month).await {
        Ok(true) => Ok(false),
        Ok(false) => usage::reserve_provider(db, &month, need, limit).await,
        Err(e) => Err(e),
    };
    match reserved {
        Ok(true) => {}
        Ok(false) => {
            out.error = Some(super::hours_exhausted_message(now));
            return;
        }
        Err(e) => {
            tracing::warn!("sổ giờ ElevenLabs lỗi: {e:?}");
            out.error = Some(LEDGER_FAILED.into());
            return;
        }
    }
    if let Err(e) = usage::hold_provider(db, run_id, &month, need).await {
        tracing::warn!("ghi phần giữ giờ ElevenLabs lỗi (khởi động lại sẽ không hoàn được): {e:?}");
    }
    // Ghi giây sắp gửi TRƯỚC khi gửi: tiền đã chi luôn nằm trong sổ, kể cả khi tiến trình chết.
    let counted = match count_all(db, members, &live).await {
        Ok(counted) if !counted.is_empty() => counted,
        // Mọi đoạn vừa bị xoá: không gửi.
        Ok(_) => {
            refund_provider(db, &month, need, run_id).await;
            return;
        }
        Err(e) => {
            refund_provider(db, &month, need, run_id).await;
            out.error = Some(e);
            return;
        }
    };
    for (i, _) in &live {
        let m = &members[*i];
        let _ = transcript::set_progress(db, m.t.id, &m.t.run_id, 10.0).await;
    }
    let durations: Vec<f64> = live.iter().map(|(_, d)| *d).collect();
    let total: f64 = durations.iter().sum();
    let result = match elevenlabs::transcribe(&state.http, elevenlabs::API_URL, api_key, &audio, total).await {
        Ok(result) => result,
        Err(e) => {
            if elevenlabs::maybe_billed(&e) {
                usage::release_provider_hold(db, run_id).await;
            } else {
                uncount(db, members, &counted).await;
                refund_provider(db, &month, need, run_id).await;
            }
            out.error = Some(if e == elevenlabs::QUOTA_EXHAUSTED {
                report_quota_exhausted(state, &month).await;
                super::hours_exhausted_message(now)
            } else {
                e
            });
            return;
        }
    };
    if let Some(billed) = result.duration_s {
        if let Err(e) = usage::settle_provider(db, &month, billed.ceil() as i64 - need).await {
            tracing::error!("chỉnh sổ giờ ElevenLabs lỗi: {e:?}");
        }
        let shares = allocate_billed(billed, &durations);
        for ((i, d), share) in live.iter().zip(shares) {
            let delta = share - d.ceil() as i64;
            let m = &members[*i];
            if delta != 0 && counted.iter().any(|(c, _)| c == i) {
                // Đoạn vừa bị xoá thì bỏ qua: sổ của nó đã quyết toán lúc xoá.
                if let Err(e) = transcript::add_sent(db, m.t.id, &m.t.run_id, delta).await {
                    tracing::warn!(file = %m.t.file_id(), delta, "chỉnh giây theo số ElevenLabs tính lỗi: {e:?}");
                }
            }
        }
    }
    usage::release_provider_hold(db, run_id).await;
    let (per_file, speakers) = split_result(result, &durations);
    out.done.extend(live.iter().map(|(i, _)| *i).zip(per_file));
    out.speakers = speakers;
}

/// Ghi giây sắp gửi cho từng đoạn. Đoạn không còn thuộc lượt (vừa bị xoá/chép lại) thì bỏ qua; sổ lỗi thì hoàn các
/// đoạn đã ghi và dừng. Trả `(chỉ số, giây đã ghi)`.
async fn count_all(
    db: &mongodb::Database,
    members: &[Member],
    live: &[(usize, f64)],
) -> Result<Vec<(usize, i64)>, String> {
    let mut counted = Vec::with_capacity(live.len());
    for &(i, d) in live {
        let m = &members[i];
        let seconds = d.ceil() as i64;
        match transcript::add_sent(db, m.t.id, &m.t.run_id, seconds).await {
            Ok(true) => counted.push((i, seconds)),
            Ok(false) => {}
            Err(e) => {
                tracing::warn!(file = %m.t.file_id(), "ghi giây đã gửi lỗi: {e:?}");
                uncount(db, members, &counted).await;
                return Err(LEDGER_FAILED.into());
            }
        }
    }
    Ok(counted)
}

/// ElevenLabs chắc chắn không xử lý: trả lại giây đã ghi; lượt đóng sau đó quyết toán sổ phút và trần USD theo số này.
async fn uncount(db: &mongodb::Database, members: &[Member], counted: &[(usize, i64)]) {
    for &(i, seconds) in counted {
        let m = &members[i];
        if let Err(e) = transcript::add_sent(db, m.t.id, &m.t.run_id, -seconds).await {
            tracing::error!(file = %m.t.file_id(), seconds, "hoàn giây đã gửi lỗi: {e:?}");
        }
    }
}

/// Hoàn phần giờ ElevenLabs đã giữ của lượt.
async fn refund_provider(db: &mongodb::Database, month: &str, need: i64, run_id: &str) {
    if let Err(e) = usage::settle_provider(db, month, -need).await {
        tracing::error!(need, "hoàn sổ giờ ElevenLabs lỗi: {e:?}");
    }
    usage::release_provider_hold(db, run_id).await;
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
        let refs: Vec<(&str, f64)> = paths.iter().map(String::as_str).zip(durations.iter().copied()).collect();
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

    #[tokio::test]
    async fn an_unreadable_segment_fails_alone_with_its_stored_length_as_fallback() {
        let missing = "/nonexistent/scribe-test.m4a";
        assert!(measure_file("ffprobe", missing, None).await.is_err());
        assert!(measure_file("ffprobe", missing, Some(0.0)).await.is_err());
        assert_eq!(measure_file("ffprobe", missing, Some(12.5)).await, Ok(12.5));
    }

    /// Tiền và hàng đợi của lượt cả phiên trên Mongo thật (DB dev): `TRANSCRIPT_TEST_MONGO_URI=<uri>
    /// TRANSCRIPT_TEST_DB_NAME=<db> cargo test whole_session_money -- --ignored`. User và tháng giả riêng, tự dọn.
    #[tokio::test]
    #[ignore]
    async fn whole_session_money_and_queue_on_real_mongo() {
        use crate::config::TranscribeLimits;
        use crate::models::usage::Charge;
        let uri = std::env::var("TRANSCRIPT_TEST_MONGO_URI").expect("TRANSCRIPT_TEST_MONGO_URI");
        let db_name = std::env::var("TRANSCRIPT_TEST_DB_NAME").expect("TRANSCRIPT_TEST_DB_NAME");
        let db = crate::db::connect(&uri, &db_name).await.expect("kết nối Mongo");
        let user = format!("selftest-{}", uuid::Uuid::new_v4());
        let month = format!("test-{}", uuid::Uuid::new_v4().simple());
        let session = format!("selftest-{}", &uuid::Uuid::new_v4().simple().to_string()[..12]);
        let key = format!("s:{session}");
        let limits = TranscribeLimits {
            free_seconds: 10_000,
            plus_seconds: 10_000,
            free_pool_seconds: 10_000,
            monthly_cap_seconds: 10_000,
            free_daily_seconds: 10_000,
            free_delay_hours: 0,
        };
        let charge = Charge { user_id: user.clone(), month: month.clone(), free_pool: false, day: None };
        let used = || async { usage::month_usage(&db, &user, &month).await.expect("sổ") };
        let ids = [ObjectId::new(), ObjectId::new(), ObjectId::new(), ObjectId::new()];
        for (index, id) in ids.iter().enumerate() {
            let owner = file::Owner { user_id: &user, plan: crate::models::plan::Plan::Plus };
            let f = file::FileDoc::new_recording(owner, &format!("{session}#{index}"), "/nonexistent", 1, 60.0, false, *id);
            file::create(&db, f).await.expect("tạo file");
        }
        let docs = |ids: Vec<ObjectId>| {
            let (db, user) = (db.clone(), user.clone());
            async move { transcript::list_owned(&db, &user, &ids).await.expect("đọc") }
        };

        // Xếp ở trạng thái chờ bật: worker chưa nhận được file nào cho tới khi bật cả lượt.
        let run = format!("run-{}", uuid::Uuid::new_v4());
        usage::reserve(&db, &charge, 10_000, 120, &limits).await.expect("giữ").expect("đủ");
        for id in &ids[..2] {
            assert!(transcript::enqueue_held(&db, *id, &user, &run, "scribe_v2", &charge, 60).await.expect("xếp"));
        }
        let coll = db.collection::<mongodb::bson::Document>("transcripts");
        let queued = |coll: mongodb::Collection<mongodb::bson::Document>, user: String| async move {
            coll.count_documents(doc! { "userId": user, "status": "queued" }).await.expect("đếm")
        };
        assert_eq!(queued(coll.clone(), user.clone()).await, 0, "chưa bật thì chưa nhận được");
        let held = transcript::statuses_for_files(&db, &user, &ids[..2]).await.expect("trạng thái");
        assert!(held.iter().all(|r| r.status == "queued"), "client thấy trạng thái chờ");
        assert_eq!(transcript::release_held(&db, &user, &run).await.expect("bật"), 2);
        assert_eq!(queued(coll.clone(), user.clone()).await, 2);

        // Đoạn 3 tải lên sau lúc bấm (chưa có lượt nào), đoạn 4 đã chép ở lượt khác: chỉ đoạn 3 được thêm vào lượt.
        assert!(transcript::enqueue(&db, ids[3], &user, "run-cu", "scribe_v2", &charge, 0).await.expect("lượt khác"));
        transcript::finish_failed(&db, ids[3], "run-cu", "lỗi cũ").await.expect("đóng lượt khác");
        coll.update_one(doc! { "_id": ids[0] }, doc! { "$set": { "status": "running" } }).await.expect("nhận đầu");
        let first = docs(vec![ids[0]]).await.remove(0);
        assert_eq!(add_late_segments(&db, &first, &ids).await.expect("thêm"), 1);
        assert_eq!(add_late_segments(&db, &first, &ids).await.expect("thêm lại"), 0, "không thêm hai lần");
        let others = transcript::claim_run_members(&db, &user, &run, ids[0]).await.expect("nhận lượt");
        let mut got: Vec<ObjectId> = others.iter().map(|t| t.id).collect();
        got.sort();
        let mut want = vec![ids[1], ids[2]];
        want.sort();
        assert_eq!(got, want);

        // Ba đoạn của lượt; đoạn 2 bị xoá trước khi ghi giây: chỉ đoạn đó bỏ qua, lượt vẫn chạy.
        let mut files: HashMap<ObjectId, file::FileDoc> = file::list_session(&db, &user, &key)
            .await
            .expect("phiên")
            .into_iter()
            .filter_map(|(_, f)| Some((f.id?, f)))
            .collect();
        let members: Vec<Member> = docs(vec![ids[0], ids[1], ids[2]])
            .await
            .into_iter()
            .filter_map(|t| Some(Member { f: files.remove(&t.id)?, t }))
            .collect();
        let index_of = |id: ObjectId| members.iter().position(|m| m.t.id == id).expect("có");
        transcript::delete(&db, ids[1]).await.expect("xoá đoạn 2");
        let live: Vec<(usize, f64)> = members.iter().enumerate().map(|(i, _)| (i, 59.6)).collect();
        let counted = count_all(&db, &members, &live).await.expect("ghi giây");
        let mut counted_ids: Vec<ObjectId> = counted.iter().map(|(i, _)| members[*i].t.id).collect();
        counted_ids.sort();
        let mut want = vec![ids[0], ids[2]];
        want.sort();
        assert_eq!(counted_ids, want);
        assert!(counted.iter().all(|(_, s)| *s == 60));
        assert!(index_of(ids[1]) < members.len());

        // ElevenLabs từ chối chắc chắn: hoàn giây đã ghi, đóng lượt ⟹ sổ người dùng và trần tổng về phần còn lại.
        // Đoạn 1 giữ 60, đoạn 3 giữ 0 (thêm muộn); đoạn 2 đã xoá với 0 giây gửi ⟹ hoàn 60 lúc xoá.
        uncount(&db, &members, &counted).await;
        for id in [ids[0], ids[2]] {
            assert!(transcript::finish_failed(&db, id, &run, "từ chối").await.expect("đóng"));
        }
        let after = used().await;
        assert_eq!((after.user_seconds, after.total_seconds), (0, 0), "không tính phút cho lượt bị từ chối");

        // Sổ giờ ElevenLabs: giữ rồi hoàn khi bị từ chối, ghi chú giữ chỗ được bỏ.
        assert!(usage::reserve_provider(&db, &month, 120, 1_000).await.expect("giữ giờ"));
        usage::hold_provider(&db, &run, &month, 120).await.expect("ghi giữ");
        refund_provider(&db, &month, 120, &run).await;
        assert_eq!(usage::provider_seconds(&db, &month).await.expect("giờ"), 0);
        assert_eq!(usage::refund_provider_holds(&db).await.expect("không còn giữ"), 0);

        // Panic giữa lượt: mọi file còn mở của lượt đóng, không file nào kẹt `running`.
        let run2 = format!("run-{}", uuid::Uuid::new_v4());
        for id in [ids[0], ids[2]] {
            assert!(transcript::enqueue(&db, id, &user, &run2, "scribe_v2", &charge, 0).await.expect("xếp lại"));
        }
        assert_eq!(transcript::fail_open_run(&db, &user, &run2, "lỗi nội bộ").await.expect("đóng"), 2);
        let open = coll.count_documents(doc! { "userId": &user, "open": true }).await.expect("đếm");
        assert_eq!(open, 0);

        for id in ids {
            let _ = db.collection::<mongodb::bson::Document>("files").delete_one(doc! { "_id": id }).await;
            transcript::delete(&db, id).await.expect("dọn");
        }
        db.collection::<mongodb::bson::Document>(usage::RUNS_COLL).delete_many(doc! { "userId": &user }).await.expect("dọn");
        db.collection::<mongodb::bson::Document>("transcribe_usage")
            .delete_many(doc! { "_id": { "$regex": format!("{month}$") } })
            .await
            .expect("dọn");
    }

    #[test]
    fn speaker_ids_follow_first_appearance() {
        let w = |speaker: Option<&str>| words::Word { text: "a".into(), start: 0.0, end: 0.1, speaker: speaker.map(str::to_string) };
        assert_eq!(speaker_ids(&[w(Some("s0")), w(None), w(Some("s1")), w(Some("s0"))]), vec!["s0", "s1"]);
        assert!(speaker_ids(&[w(None)]).is_empty());
    }
}
