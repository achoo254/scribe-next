//! Dịch lời chép của một phiên sang tiếng Anh hoặc tiếng Việt, từng câu, giữ mốc giây của câu; chỉ khi
//! người dùng bấm "Dịch". Bản dịch lưu ở `transcript_translations` (`models::translation`).
//!
//! Mỗi câu dịch gắn với câu nguồn bằng `(fileId, chỉ số câu trong đoạn)` và băm chữ nguồn lúc dịch
//! ([`source_hash`]): chữ đổi (sửa câu, chép lại đoạn) thì câu đó là bản dịch cũ. Một lượt chỉ gửi câu chưa
//! có bản dịch hoặc bản dịch cũ — vừa là "Dịch lại câu đã sửa", vừa là dịch tiếp khi lượt trước dừng giữa
//! chừng. Không còn câu nào cần dịch thì trả ngay, không tính lượt.
//!
//! Tiền: như tóm tắt (`summary::meter`) — lượt ngày riêng (`TRANSLATE_FREE_DAILY`, `TRANSLATE_PAID_DAILY`),
//! chung trần chi phí tháng của tài khoản và trần tổng tháng của chép lời. Một lượt giữ chỗ một lần cho mọi
//! câu cần dịch, gọi model theo từng lô ([`batch`]) tuần tự trong task nền, rồi quyết toán một lần theo tổng
//! token thật; lượt ngày chỉ trả lại khi OpenAI chắc chắn chưa xử lý lô nào. Lô lỗi (gọi lỗi, đầu ra không
//! khớp số câu) thử lại một lần; lỗi nữa thì dừng lượt, phần đã dịch giữ lại. Handler trả 202 sau khi giữ
//! chỗ xong; web hỏi tiến độ qua `…/translation/status`. Audit không ghi chữ.

pub mod batch;

use crate::audit;
use crate::auth::UserId;
use crate::error::AppError;
use crate::models::summary::Feature;
use crate::models::translation::{self, Item, Lang, NewItem, TranslationDoc};
use crate::models::{plan, user};
use crate::state::AppState;
use crate::summary::{self, meter, RunningGuard};
use crate::transcribe::docs::{self, check_key};
use crate::transcribe::openai::is_key_broken;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use mongodb::bson::{doc, DateTime};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::oneshot;

/// Chữ cần dịch dài nhất của một lượt (ký tự): ~3 giờ họp nói liên tục; chặn chi phí bất thường.
const MAX_INPUT_CHARS: usize = 400_000;
/// Một người bấm tối đa chừng này lượt mỗi phút (cộng với khoá một lượt mỗi phiên).
const PER_MINUTE: u32 = 6;
/// Số lần gọi model tối đa cho một lô (lần đầu và một lần thử lại).
const ATTEMPTS: u32 = 2;

const PAUSED: &str = "Dịch đang tạm ngưng, quản trị đã được báo. Thử lại sau.";
const FAILED: &str = "Không dịch được phần còn lại lúc này. Bấm dịch tiếp để thử lại.";
const SAVE_FAILED: &str = "Không lưu được bản dịch. Bấm dịch tiếp để thử lại.";
const INTERRUPTED: &str = "Lượt dịch bị ngắt giữa chừng. Bấm dịch tiếp để làm nốt.";

/// Băm chữ nguồn của một câu (16 ký tự hex đầu của SHA-256, khoảng trắng đã gộp): so với băm đã lưu để biết
/// bản dịch còn khớp câu không.
pub fn source_hash(text: &str) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    crate::session::hash_token(&line)[..16].to_string()
}

fn slot(key: &str) -> String {
    format!("tr:{key}")
}

/// Trạng thái cho client: doc ghi `running` mà tiến trình không còn chạy lượt đó (khởi động lại giữa chừng)
/// thì báo `failed` kèm lý do bị ngắt.
fn effective_status(state: &AppState, user_id: &str, key: &str, status: &str, error: Option<String>) -> (String, Option<String>) {
    if status == "running" && !state.summaries.is_running(user_id, &slot(key)) {
        return ("failed".into(), Some(INTERRUPTED.into()));
    }
    (status.to_string(), error)
}

/// Trạng thái bản dịch của phiên cho `GET /api/transcript-docs/{key}`: `[{lang, status}]`.
pub(crate) async fn summaries_of(state: &AppState, user_id: &str, key: &str) -> Result<Vec<Value>, AppError> {
    Ok(translation::progress(&state.db, user_id, key)
        .await?
        .into_iter()
        .map(|p| {
            let (status, _) = effective_status(state, user_id, key, &p.status, p.error);
            json!({ "lang": p.lang.code(), "status": status })
        })
        .collect())
}

/// Một câu nguồn cần dịch.
#[derive(Debug, Clone, PartialEq)]
struct Source {
    file_id: String,
    idx: usize,
    text: String,
    hash: String,
}

/// Câu nguồn chưa có bản dịch hoặc có bản dịch cũ (theo thứ tự trong phiên). Câu rỗng không dịch.
fn pending(segments: &[(&str, Vec<&str>)], existing: Option<&TranslationDoc>) -> Vec<Source> {
    let mut out = Vec::new();
    for (file_id, texts) in segments {
        for (idx, text) in texts.iter().enumerate() {
            let text = text.trim();
            if text.is_empty() {
                continue;
            }
            let hash = source_hash(text);
            if existing.and_then(|d| d.item(file_id, idx)).is_some_and(|item| item.h == hash) {
                continue;
            }
            out.push(Source { file_id: file_id.to_string(), idx, text: text.to_string(), hash });
        }
    }
    out
}

/// Câu dịch của màn hình đọc, khớp chỉ số câu của từng đoạn: `{text, outdated}` hoặc `null` (chưa dịch, câu
/// rỗng). Trả kèm số câu bản dịch cũ và số câu chưa dịch.
fn aligned(segments: &[(&str, Vec<&str>)], doc: Option<&TranslationDoc>) -> (Vec<Value>, usize, usize) {
    let (mut outdated, mut missing) = (0, 0);
    let out = segments
        .iter()
        .map(|(file_id, texts)| {
            let sentences: Vec<Value> = texts
                .iter()
                .enumerate()
                .map(|(idx, text)| {
                    let text = text.trim();
                    if text.is_empty() {
                        return Value::Null;
                    }
                    match doc.and_then(|d| d.item(file_id, idx)) {
                        Some(item) => {
                            let old = item.h != source_hash(text);
                            outdated += usize::from(old);
                            json!({ "text": item.t, "outdated": old })
                        }
                        None => {
                            missing += 1;
                            Value::Null
                        }
                    }
                })
                .collect();
            json!({ "fileId": file_id, "sentences": sentences })
        })
        .collect();
    (out, outdated, missing)
}

#[derive(Deserialize)]
pub struct LangReq {
    lang: Option<String>,
}

fn parse_lang(raw: Option<&str>) -> Result<Lang, AppError> {
    raw.and_then(Lang::parse).ok_or_else(|| AppError::BadRequest("ngôn ngữ chỉ nhận en hoặc vi".into()))
}

/// `GET /api/transcript-docs/{key}/translation?lang=` — bản dịch khớp từng câu của màn hình đọc.
pub async fn get(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path(key): Path<String>,
    Query(q): Query<LangReq>,
) -> Result<Json<Value>, AppError> {
    let lang = parse_lang(q.lang.as_deref())?;
    let loaded = docs::load(&state, &user.0, &key).await?;
    let doc = translation::get(&state.db, &user.0, &key, lang).await?;
    let segment_texts = loaded.segment_texts();
    let (segments, outdated, missing) = aligned(&segment_texts, doc.as_ref());
    let Some(doc) = doc else {
        return Ok(Json(json!({
            "lang": lang.code(), "status": "none", "done": 0, "total": 0, "error": null, "model": null,
            "updatedAt": null, "outdated": 0, "missing": missing, "segments": segments,
        })));
    };
    let (status, error) = effective_status(&state, &user.0, &key, &doc.status, doc.error.clone());
    Ok(Json(json!({
        "lang": lang.code(),
        "status": status,
        "done": doc.done,
        "total": doc.total,
        "error": error,
        "model": doc.model,
        "updatedAt": doc.updated_at.try_to_rfc3339_string().ok(),
        "outdated": outdated,
        "missing": missing,
        "segments": segments,
    })))
}

/// `GET /api/transcript-docs/{key}/translation/status?lang=` — tiến độ lượt gần nhất, không kèm câu.
pub async fn status(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path(key): Path<String>,
    Query(q): Query<LangReq>,
) -> Result<Json<Value>, AppError> {
    let lang = parse_lang(q.lang.as_deref())?;
    check_key(&key)?;
    let found = translation::progress(&state.db, &user.0, &key).await?.into_iter().find(|p| p.lang == lang);
    let Some(p) = found else {
        return Ok(Json(json!({ "lang": lang.code(), "status": "none", "done": 0, "total": 0, "error": null })));
    };
    let (status, error) = effective_status(&state, &user.0, &key, &p.status, p.error);
    Ok(Json(json!({ "lang": lang.code(), "status": status, "done": p.done, "total": p.total, "error": error })))
}

/// Phần đã kiểm xong của một lượt, đủ để task nền chạy tiếp mà không cần request.
struct Job {
    user_id: String,
    key: String,
    lang: Lang,
    plan: plan::Plan,
    sources: Vec<Source>,
    api_key: String,
}

/// `POST /api/transcript-docs/{key}/translation {lang}` — dịch các câu chưa có bản dịch hoặc có bản dịch cũ.
pub async fn create(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path(key): Path<String>,
    Json(req): Json<LangReq>,
) -> Result<Response, AppError> {
    let lang = parse_lang(req.lang.as_deref())?;
    check_key(&key)?;
    let profile = user::find_profile(&state.db, &user.0)
        .await?
        .ok_or_else(|| AppError::Unauthorized("user không tồn tại".into()))?;
    if profile.blocked {
        return Err(AppError::Forbidden("tài khoản đã bị khoá".into()));
    }
    if !state.config.transcribe_allowed(&profile.email) {
        return Err(AppError::Forbidden("tài khoản này chưa được dùng dịch".into()));
    }
    let api_key = state
        .transcriber
        .api_key()
        .filter(|_| !state.summaries.paused())
        .ok_or_else(|| AppError::Unavailable(PAUSED.into()))?
        .to_string();
    if !state.rate.allow(&format!("translate:{}", user.0), PER_MINUTE) {
        return Err(AppError::TooManyRequests("bấm dịch quá nhanh, thử lại sau một phút".into()));
    }
    let running = state
        .summaries
        .start(&user.0, &slot(&key))
        .ok_or_else(|| AppError::TooManyRequests("bản này đang được dịch, chờ một chút".into()))?;

    let loaded = docs::load(&state, &user.0, &key).await?;
    let segment_texts = loaded.segment_texts();
    if segment_texts.iter().all(|(_, texts)| texts.iter().all(|t| t.trim().is_empty())) {
        return Err(AppError::Coded {
            status: StatusCode::CONFLICT,
            code: "NOT_TRANSCRIBED",
            message: "Bản ghi chưa có đoạn nào chép lời xong. Chép lời trước rồi dịch.".into(),
        });
    }
    let existing = translation::get(&state.db, &user.0, &key, lang).await?;
    let sources = pending(&segment_texts, existing.as_ref());
    if sources.is_empty() {
        // Mọi câu đã có bản dịch khớp (lượt trước bị ngắt ngay sau lô cuối): ghi lại là xong.
        if existing.as_ref().is_some_and(|d| d.status != "done") {
            translation::finish(&state.db, &user.0, &key, lang, None).await?;
        }
        return Ok(Json(json!({ "lang": lang.code(), "status": "done", "done": 0, "total": 0 })).into_response());
    }
    if sources.iter().map(|s| s.text.chars().count()).sum::<usize>() > MAX_INPUT_CHARS {
        return Err(AppError::TooLarge("bản chép lời quá dài để dịch".into()));
    }
    let total = sources.len() as i64;
    let job = Job {
        user_id: user.0.clone(),
        key,
        lang,
        plan: plan::effective(profile.plan.as_deref(), profile.plan_until, DateTime::now()),
        sources,
        api_key,
    };
    // Task giữ `running` tới khi dịch xong; handler bị huỷ thì task vẫn chạy hết và quyết toán.
    let (started_tx, started_rx) = oneshot::channel();
    tokio::spawn(run(state, job, running, started_tx));
    match started_rx.await {
        Ok(Ok(())) => {
            let body = json!({ "lang": lang.code(), "status": "running", "done": 0, "total": total });
            Ok((StatusCode::ACCEPTED, Json(body)).into_response())
        }
        Ok(Err(e)) => Err(e),
        Err(_) => {
            tracing::error!("task dịch dừng bất thường trước khi bắt đầu");
            Err(AppError::Internal("dịch dừng bất thường".into()))
        }
    }
}

/// Kết quả cộng dồn các lần gọi model của một lượt.
#[derive(Debug, Default, PartialEq)]
struct Tally {
    input_tokens: i64,
    output_tokens: i64,
    /// Giây giữ nguyên theo ước lượng của các lần gọi có thể đã bị tính tiền mà không biết token.
    unknown_s: i64,
    /// OpenAI có thể đã xử lý ít nhất một lần gọi.
    billed: bool,
}

impl Tally {
    fn add<T>(&mut self, call: &summary::llm::Call<T>, estimate_s: i64) {
        match &call.result {
            Ok(c) => {
                self.input_tokens += c.input_tokens;
                self.output_tokens += c.output_tokens;
                self.billed = true;
            }
            Err(_) if call.maybe_billed => {
                self.unknown_s += estimate_s;
                self.billed = true;
            }
            Err(_) => {}
        }
    }

    fn outcome(&self, ok: bool) -> meter::Outcome {
        meter::Outcome {
            cost_s: meter::cost_seconds(self.input_tokens, self.output_tokens) + self.unknown_s,
            refund: !self.billed,
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            ok,
        }
    }
}

/// Lô dịch xong của một lượt, hoặc lý do dừng.
enum BatchEnd {
    Done(Vec<String>),
    /// Key/credit hỏng: tạm ngưng các tính năng gọi model chữ.
    KeyBroken(String),
    Failed(String),
}

/// Dịch một lô, thử lại một lần khi lỗi gọi model (trừ key/credit hỏng). Model trả lệch số dòng (hay gộp
/// các câu ngắn liền nhau như "À." "Vâng.") thì không gửi lại nguyên lô mà chia đôi và dịch từng nửa: lô
/// nhỏ hiếm khi bị gộp, mỗi câu vẫn có đúng một bản dịch của riêng nó.
fn translate_batch<'a>(
    state: &'a AppState,
    job: &'a Job,
    lines: &'a [&'a str],
    tally: &'a mut Tally,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = BatchEnd> + Send + 'a>> {
    Box::pin(async move {
        let chars: usize = lines.iter().map(|l| l.chars().count()).sum();
        let estimate = batch::estimate_seconds(chars, lines.len());
        let model = state.config.summary_model.as_str();
        let mut last = String::new();
        for attempt in 1..=ATTEMPTS {
            let call = batch::translate(&state.http, &job.api_key, model, job.lang, lines).await;
            tally.add(&call, estimate);
            let draft = match call.result.and_then(|c| c.draft) {
                Ok(draft) => draft,
                Err(e) if is_key_broken(&e) => return BatchEnd::KeyBroken(e),
                Err(e) => {
                    tracing::warn!(key = %job.key, attempt, lines = lines.len(), "lô dịch lỗi: {e}");
                    last = e;
                    continue;
                }
            };
            match batch::map_output(lines, draft) {
                Ok(texts) => return BatchEnd::Done(texts),
                Err(e) if lines.len() > 1 => {
                    tracing::warn!(key = %job.key, lines = lines.len(), "lô dịch lệch dòng, chia đôi: {e}");
                    return translate_halves(state, job, lines, tally).await;
                }
                Err(e) => {
                    tracing::warn!(key = %job.key, attempt, "câu dịch lỗi: {e}");
                    last = e;
                }
            }
        }
        BatchEnd::Failed(last)
    })
}

/// Dịch hai nửa của một lô lần lượt rồi ghép lại theo thứ tự.
async fn translate_halves(state: &AppState, job: &Job, lines: &[&str], tally: &mut Tally) -> BatchEnd {
    let (left, right) = lines.split_at(lines.len() / 2);
    let mut texts = match translate_batch(state, job, left, tally).await {
        BatchEnd::Done(texts) => texts,
        other => return other,
    };
    match translate_batch(state, job, right, tally).await {
        BatchEnd::Done(rest) => {
            texts.extend(rest);
            BatchEnd::Done(texts)
        }
        other => other,
    }
}

/// Phần có tiền của một lượt: giữ chỗ (báo cho handler), dịch từng lô, quyết toán ở mọi đường ra.
async fn run(state: AppState, job: Job, _running: RunningGuard, started: oneshot::Sender<Result<(), AppError>>) {
    let (db, user_id, key, lang) = (&state.db, job.user_id.as_str(), job.key.as_str(), job.lang);
    let chars: Vec<usize> = job.sources.iter().map(|s| s.text.chars().count()).collect();
    let ranges = batch::split(&chars, batch::MAX_LINES, batch::MAX_CHARS);
    let estimate: i64 = ranges.iter().map(|r| batch::estimate_seconds(chars[r.clone()].iter().sum(), r.len())).sum();
    let reservation = match meter::reserve(&state, Feature::Translate, user_id, Some(key), job.plan, estimate).await {
        Ok(r) => r,
        Err(e) => {
            let _ = started.send(Err(e));
            return;
        }
    };
    let model = state.config.summary_model.as_str();
    if let Err(e) = translation::start_run(db, user_id, key, lang, model, job.sources.len() as i64).await {
        meter::settle_outcome(&state, &reservation, &Tally::default().outcome(false)).await;
        let _ = started.send(Err(e));
        return;
    }
    // Handler có thể đã bị huỷ (client ngắt): lượt vẫn chạy hết.
    let _ = started.send(Ok(()));

    let mut tally = Tally::default();
    let mut failure: Option<(String, String)> = None;
    let mut translated = 0usize;
    for range in &ranges {
        let sources = &job.sources[range.clone()];
        let lines: Vec<&str> = sources.iter().map(|s| s.text.as_str()).collect();
        let texts = match translate_batch(&state, &job, &lines, &mut tally).await {
            BatchEnd::Done(texts) => texts,
            BatchEnd::KeyBroken(e) => {
                summary::report_key_broken(&state, &e);
                failure = Some((PAUSED.into(), e));
                break;
            }
            BatchEnd::Failed(e) => {
                failure = Some((FAILED.into(), e));
                break;
            }
        };
        let items: Vec<NewItem<'_>> = sources
            .iter()
            .zip(texts)
            .map(|(s, t)| NewItem { file_id: &s.file_id, idx: s.idx, item: Item { h: s.hash.clone(), t } })
            .collect();
        if let Err(e) = translation::save_items(db, user_id, key, lang, &items).await {
            failure = Some((SAVE_FAILED.into(), format!("{e:?}")));
            break;
        }
        translated += items.len();
    }

    let cost_s = meter::settle_outcome(&state, &reservation, &tally.outcome(failure.is_none())).await;
    let details = doc! {
        "key": key,
        "lang": lang.code(),
        "sentences": translated as i64,
        "pending": job.sources.len() as i64,
        "batches": ranges.len() as i64,
        "inputTokens": tally.input_tokens,
        "outputTokens": tally.output_tokens,
        "costS": cost_s,
    };
    let finished = match &failure {
        None => {
            audit::record(&state, Some(user_id), "translate.create", details);
            translation::finish(db, user_id, key, lang, None).await
        }
        Some((message, error)) => {
            let mut details = details;
            details.insert("error", error.as_str());
            audit::record(&state, Some(user_id), "translate.failed", details);
            translation::finish(db, user_id, key, lang, Some(message)).await
        }
    };
    if let Err(e) = finished {
        tracing::error!(key = %key, "ghi kết thúc lượt dịch lỗi: {e:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn doc_with(items: &[(&str, usize, &str, &str)]) -> TranslationDoc {
        let mut map: HashMap<String, HashMap<String, Item>> = HashMap::new();
        for (file, idx, source, t) in items {
            map.entry(file.to_string()).or_default().insert(idx.to_string(), Item { h: source_hash(source), t: t.to_string() });
        }
        TranslationDoc {
            id: mongodb::bson::oid::ObjectId::new(),
            lang: "en".into(),
            model: "m".into(),
            status: "done".into(),
            done: 0,
            total: 0,
            error: None,
            updated_at: DateTime::from_millis(0),
            items: map,
        }
    }

    #[test]
    fn source_hash_ignores_spacing_only() {
        assert_eq!(source_hash(" Chốt  giá\n5 triệu. "), source_hash("Chốt giá 5 triệu."));
        assert_ne!(source_hash("Chốt giá 5 triệu."), source_hash("Chốt giá 6 triệu."));
        assert_eq!(source_hash("x").len(), 16);
    }

    #[test]
    fn only_untranslated_or_edited_sentences_are_pending() {
        let segments = vec![("a", vec!["Một.", "  ", "Hai đã sửa."]), ("b", vec!["Ba."]), ("c", vec![])];
        assert_eq!(pending(&segments, None).iter().map(|s| (s.file_id.as_str(), s.idx)).collect::<Vec<_>>(), vec![
            ("a", 0),
            ("a", 2),
            ("b", 0)
        ]);
        let doc = doc_with(&[("a", 0, "Một.", "One."), ("a", 2, "Hai.", "Two."), ("b", 0, "Ba.", "Three.")]);
        let left = pending(&segments, Some(&doc));
        assert_eq!(left, vec![Source { file_id: "a".into(), idx: 2, text: "Hai đã sửa.".into(), hash: source_hash("Hai đã sửa.") }]);
    }

    #[test]
    fn reader_view_marks_outdated_and_missing_sentences() {
        let segments = vec![("a", vec!["Một.", "", "Hai đã sửa."]), ("b", vec!["Ba.", "Bốn."])];
        let doc = doc_with(&[("a", 0, "Một.", "One."), ("a", 2, "Hai.", "Two."), ("b", 0, "Ba.", "Three.")]);
        let (out, outdated, missing) = aligned(&segments, Some(&doc));
        assert_eq!((outdated, missing), (1, 1));
        assert_eq!(out[0], json!({ "fileId": "a", "sentences": [
            { "text": "One.", "outdated": false },
            null,
            { "text": "Two.", "outdated": true },
        ] }));
        assert_eq!(out[1]["sentences"], json!([{ "text": "Three.", "outdated": false }, null]));
        let (_, outdated, missing) = aligned(&segments, None);
        assert_eq!((outdated, missing), (0, 4));
    }

    #[test]
    fn tally_refunds_the_daily_turn_only_when_nothing_was_billed() {
        use summary::llm::{Call, Completion};
        let ok = |i, o| Call::<batch::Draft> {
            result: Ok(Completion { input_tokens: i, output_tokens: o, draft: Ok(batch::Draft { lines: vec![] }) }),
            maybe_billed: true,
        };
        let unreachable = Call::<batch::Draft> { result: Err("không kết nối được".into()), maybe_billed: false };
        let timed_out = Call::<batch::Draft> { result: Err("quá giờ".into()), maybe_billed: true };

        let mut none = Tally::default();
        none.add(&unreachable, 50);
        none.add(&unreachable, 50);
        let outcome = none.outcome(false);
        assert_eq!((outcome.cost_s, outcome.refund), (0, true));

        let mut some = Tally::default();
        some.add(&ok(20_000, 1_500), 300);
        some.add(&ok(1_000_000, 0), 300);
        some.add(&timed_out, 70);
        some.add(&unreachable, 70);
        let outcome = some.outcome(true);
        assert_eq!((outcome.input_tokens, outcome.output_tokens), (1_020_000, 1_500));
        assert_eq!(outcome.cost_s, meter::cost_seconds(1_020_000, 1_500) + 70);
        assert!(!outcome.refund && outcome.ok);
    }
}
