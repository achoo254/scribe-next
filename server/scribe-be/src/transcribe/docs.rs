//! Transcript theo phiên ghi: gom các đoạn của một phiên (`file::session_key`), ghép kết quả máy
//! (`transcripts`, mỗi file một doc) với phần người dùng tạo ra (`transcript_docs`: tiêu đề, câu sửa).
//! Danh sách và chi tiết tính lúc đọc, không lưu trùng.
//!
//! Đọc/sửa/xoá transcript của chính mình chỉ cần đăng nhập (như `GET /api/transcripts`); chỉ việc
//! xếp chép lời cần quyền `TRANSCRIBE_EMAILS`. Audit không ghi tiêu đề hay câu (giữ 180 ngày).

use super::chunking::Sentence;
use crate::audit;
use crate::auth::UserId;
use crate::error::AppError;
use crate::models::file::{self, FileDoc};
use crate::models::transcript::{self, TranscriptDoc};
use crate::models::transcript_doc::{self, SegmentEdit};
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::{Extension, Json};
use mongodb::bson::{doc, oid::ObjectId};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

/// Tiêu đề dài nhất (ký tự).
const MAX_TITLE_CHARS: usize = 120;
/// Một đoạn 1 phút chỉ vài chục câu; trần này chặn body bất thường chứ không chặn người dùng.
const MAX_SENTENCES: usize = 5000;
const MAX_SENTENCE_CHARS: usize = 2000;
/// Tên đoạn do client đặt: index nhảy vọt không được làm phình danh sách "thiếu đoạn".
const MAX_MISSING_REPORTED: usize = 200;

fn check_key(key: &str) -> Result<(), AppError> {
    if file::is_valid_session_key(key) {
        Ok(())
    } else {
        Err(AppError::BadRequest("khoá transcript không hợp lệ".into()))
    }
}

/// Các đoạn của phiên; phiên không có bản ghi nào của user → 404.
async fn session_files(state: &AppState, user_id: &str, key: &str) -> Result<Vec<(u32, FileDoc)>, AppError> {
    check_key(key)?;
    let files = file::list_session(&state.db, user_id, key).await?;
    if files.is_empty() {
        return Err(AppError::NotFound("transcript không tồn tại".into()));
    }
    Ok(files)
}

fn file_ids(files: &[(u32, FileDoc)]) -> Vec<ObjectId> {
    files.iter().filter_map(|(_, f)| f.id).collect()
}

/// Thời lượng dùng để cộng dồn mốc: `durationS` của file; thiếu hoặc 0 thì lấy mốc cuối câu cuối.
pub(super) fn segment_duration(f: &FileDoc, sentences: &[Sentence]) -> f64 {
    f.duration_s
        .filter(|d| *d > 0.0)
        .or_else(|| sentences.last().map(|s| s.end))
        .unwrap_or(0.0)
}

/// Câu máy của một lượt chép đã xong, thay chữ bằng câu sửa khi còn hiệu lực.
/// Trả (câu, đã sửa?, số câu khác câu máy) — số sau là thứ chép lại sẽ làm mất.
pub(super) fn effective_sentences(t: &TranscriptDoc, edit: Option<&SegmentEdit>) -> (Vec<Sentence>, bool, usize) {
    match transcript_doc::effective_texts(edit, &t.run_id, t.sentences.len()) {
        Some(texts) => {
            let changed = t.sentences.iter().zip(texts).filter(|(s, text)| s.text != **text).count();
            let sentences = t
                .sentences
                .iter()
                .zip(texts)
                .map(|(s, text)| Sentence { start: s.start, end: s.end, text: text.clone() })
                .collect();
            (sentences, true, changed)
        }
        None => (t.sentences.clone(), false, 0),
    }
}

#[derive(Debug)]
struct SegmentView {
    file_id: String,
    index: u32,
    offset_s: f64,
    duration_s: f64,
    has_video: bool,
    /// none | queued | running | done | failed
    status: String,
    progress: f64,
    error: Option<String>,
    run_id: Option<String>,
    edited: bool,
    /// Số câu khác câu máy (chép lại sẽ làm mất).
    edited_sentences: usize,
    sentences: Vec<Sentence>,
}

impl SegmentView {
    fn to_json(&self) -> Value {
        json!({
            "fileId": self.file_id,
            "index": self.index,
            "offsetS": self.offset_s,
            "durationS": self.duration_s,
            "hasVideo": self.has_video,
            "status": self.status,
            "progress": self.progress,
            "error": self.error,
            "runId": self.run_id,
            "edited": self.edited,
            "editedSentences": self.edited_sentences,
            "sentences": self.sentences,
        })
    }
}

/// Ghép các đoạn (đã theo thứ tự) với kết quả máy và câu sửa; mốc đoạn sau cộng dồn đoạn trước.
fn assemble(
    files: &[(u32, FileDoc)],
    transcripts: &HashMap<ObjectId, TranscriptDoc>,
    edits: &HashMap<String, SegmentEdit>,
) -> Vec<SegmentView> {
    let mut offset = 0.0;
    let mut out = Vec::with_capacity(files.len());
    for (index, f) in files {
        let Some(id) = f.id else { continue };
        let file_id = id.to_hex();
        let t = transcripts.get(&id);
        let (sentences, edited, edited_sentences) = match t.filter(|t| t.status == "done") {
            Some(t) => effective_sentences(t, edits.get(&file_id)),
            None => (Vec::new(), false, 0),
        };
        let duration_s = segment_duration(f, &sentences);
        out.push(SegmentView {
            file_id,
            index: *index,
            offset_s: offset,
            duration_s,
            has_video: f.has_video.unwrap_or(false),
            status: t.map_or_else(|| "none".to_string(), |t| t.status.clone()),
            progress: t.map_or(0.0, |t| t.progress),
            error: t.and_then(|t| t.error.clone()),
            run_id: t.map(|t| t.run_id.clone()),
            edited,
            edited_sentences,
            sentences,
        });
        offset += duration_s;
    }
    out
}

/// Index đoạn bị nhảy trong một phiên iPhone (đoạn chưa tải lên hoặc đã bị xoá riêng).
fn missing_indexes(key: &str, files: &[(u32, FileDoc)]) -> Vec<u32> {
    if !key.starts_with("s:") {
        return Vec::new();
    }
    let present: HashSet<u32> = files.iter().map(|(index, _)| *index).collect();
    let last = files.iter().map(|(index, _)| *index).max().unwrap_or(0);
    (0..last).filter(|i| !present.contains(i)).take(MAX_MISSING_REPORTED).collect()
}

fn created_at(files: &[(u32, FileDoc)]) -> Option<String> {
    files.iter().map(|(_, f)| f.created_at).min().and_then(|d| d.try_to_rfc3339_string().ok())
}

/// Tên file cho bản ghi một file (UI dùng làm tên mặc định); phiên iPhone thì UI tự đặt theo giờ.
fn single_name(key: &str, files: &[(u32, FileDoc)]) -> Option<String> {
    key.starts_with("f:").then(|| files.first().map(|(_, f)| f.name.clone())).flatten()
}

// MARK: Handlers

/// `GET /api/transcript-docs` — các phiên có ít nhất một đoạn đã xếp chép lời, mới nhất trước.
pub async fn list(State(state): State<AppState>, Extension(user): Extension<UserId>) -> Result<Json<Value>, AppError> {
    let previews: HashMap<ObjectId, transcript::Preview> = transcript::previews(&state.db, &user.0)
        .await?
        .into_iter()
        .map(|p| (p.run.file_id, p))
        .collect();

    // Bản ghi theo khoá phiên, giữ thứ tự gặp (mới nhất trước); chỉ phiên có transcript.
    let mut order: Vec<String> = Vec::new();
    let mut sessions: HashMap<String, Vec<(u32, FileDoc)>> = HashMap::new();
    for f in file::list_recordings(&state.db, &user.0).await? {
        let Some(id) = f.id else { continue };
        let key = file::session_key(&f.name, id);
        let index = file::parse_segment_name(&f.name).map_or(0, |(_, index)| index);
        let entry = sessions.entry(key.clone()).or_insert_with(|| {
            order.push(key);
            Vec::new()
        });
        entry.push((index, f));
    }
    order.retain(|key| sessions[key].iter().any(|(_, f)| f.id.is_some_and(|id| previews.contains_key(&id))));

    let docs: HashMap<String, transcript_doc::SessionDoc> = transcript_doc::get_many(&state.db, &user.0, &order)
        .await?
        .into_iter()
        .map(|d| (d.key.clone(), d))
        .collect();

    let mut items: Vec<(Option<String>, Value)> = Vec::with_capacity(order.len());
    for key in &order {
        let Some(mut files) = sessions.remove(key) else { continue };
        files.sort_by_key(|(index, _)| *index);
        let doc = docs.get(key);
        let (mut done, mut failed, mut pending, mut untranscribed) = (0, 0, 0, 0);
        let mut preview: Option<String> = None;
        for (_, f) in &files {
            let Some(p) = f.id.and_then(|id| previews.get(&id)) else {
                untranscribed += 1;
                continue;
            };
            match p.run.status.as_str() {
                "done" => done += 1,
                "failed" => failed += 1,
                _ => pending += 1,
            }
            if preview.is_none() && p.run.status == "done" && p.sentence_count > 0 {
                let edit = f.id.and_then(|id| doc.and_then(|d| d.edits.get(&id.to_hex())));
                let texts = match transcript_doc::effective_texts(edit, &p.run.run_id, p.sentence_count) {
                    Some(texts) => &texts[..texts.len().min(transcript::PREVIEW_SENTENCES as usize)],
                    None => &p.first_texts[..],
                };
                preview = Some(texts.join(" "));
            }
        }
        let created = created_at(&files);
        let item = json!({
            "key": key,
            "title": doc.and_then(|d| d.title.clone()),
            "name": single_name(key, &files),
            "createdAt": created,
            "durationS": files.iter().map(|(_, f)| f.duration_s.unwrap_or(0.0)).sum::<f64>(),
            "segments": files.len(),
            "done": done,
            "failed": failed,
            "pending": pending,
            "untranscribed": untranscribed,
            "preview": preview,
        });
        items.push((created, item));
    }
    // Mới nhất trước theo lúc bắt đầu phiên (RFC 3339 cùng múi giờ UTC nên so chuỗi được).
    items.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(Json(json!({ "items": items.into_iter().map(|(_, v)| v).collect::<Vec<_>>() })))
}

/// `GET /api/transcript-docs/{key}` — toàn bộ transcript của phiên, đoạn theo thứ tự ghi.
pub async fn get(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path(key): Path<String>,
) -> Result<Json<Value>, AppError> {
    let files = session_files(&state, &user.0, &key).await?;
    let transcripts: HashMap<ObjectId, TranscriptDoc> = transcript::list_owned(&state.db, &user.0, &file_ids(&files))
        .await?
        .into_iter()
        .map(|t| (t.id, t))
        .collect();
    let doc = transcript_doc::get(&state.db, &user.0, &key).await?;
    let edits = doc.as_ref().map(|d| d.edits.clone()).unwrap_or_default();
    let segments = assemble(&files, &transcripts, &edits);
    Ok(Json(json!({
        "key": key,
        "title": doc.and_then(|d| d.title),
        "name": single_name(&key, &files),
        "createdAt": created_at(&files),
        "segments": segments.iter().map(SegmentView::to_json).collect::<Vec<_>>(),
        "missingIndexes": missing_indexes(&key, &files),
    })))
}

/// `GET /api/transcript-docs/{key}/status` — trạng thái từng đoạn, không kèm câu (poll khi đang chép).
pub async fn status(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path(key): Path<String>,
) -> Result<Json<Value>, AppError> {
    let files = session_files(&state, &user.0, &key).await?;
    let runs: HashMap<ObjectId, transcript::RunStatus> =
        transcript::statuses_for_files(&state.db, &user.0, &file_ids(&files))
            .await?
            .into_iter()
            .map(|r| (r.file_id, r))
            .collect();
    let items: Vec<Value> = files
        .iter()
        .filter_map(|(_, f)| f.id)
        .map(|id| match runs.get(&id) {
            Some(r) => json!({ "fileId": id.to_hex(), "status": r.status, "progress": r.progress, "runId": r.run_id }),
            None => json!({ "fileId": id.to_hex(), "status": "none", "progress": 0.0, "runId": null }),
        })
        .collect();
    Ok(Json(json!({ "items": items })))
}

#[derive(Deserialize)]
pub struct RenameReq {
    title: Option<String>,
}

/// Tiêu đề sau khi chuẩn hoá: cắt khoảng trắng, rỗng = về tên mặc định.
fn normalize_title(raw: Option<&str>) -> Result<Option<String>, AppError> {
    let title = raw.map(str::trim).filter(|t| !t.is_empty());
    if title.is_some_and(|t| t.chars().count() > MAX_TITLE_CHARS) {
        return Err(AppError::BadRequest(format!("tên tối đa {MAX_TITLE_CHARS} ký tự")));
    }
    Ok(title.map(str::to_string))
}

/// `PATCH /api/transcript-docs/{key}` — đổi tên; `null` hoặc chuỗi rỗng = về tên mặc định.
pub async fn rename(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path(key): Path<String>,
    Json(req): Json<RenameReq>,
) -> Result<Json<Value>, AppError> {
    let title = normalize_title(req.title.as_deref())?;
    session_files(&state, &user.0, &key).await?;
    transcript_doc::set_title(&state.db, &user.0, &key, title.as_deref()).await?;
    let title_len = title.as_ref().map_or(0, |t| t.chars().count() as i64);
    audit::record(&state, Some(&user.0), "transcript.rename", doc! { "key": &key, "titleLen": title_len });
    Ok(Json(json!({ "title": title })))
}

#[derive(Deserialize)]
pub struct EditReq {
    #[serde(rename = "runId")]
    run_id: String,
    texts: Vec<String>,
}

fn check_texts(texts: &[String]) -> Result<(), AppError> {
    if texts.len() > MAX_SENTENCES {
        return Err(AppError::BadRequest(format!("tối đa {MAX_SENTENCES} câu")));
    }
    if texts.iter().any(|t| t.chars().count() > MAX_SENTENCE_CHARS) {
        return Err(AppError::BadRequest(format!("mỗi câu tối đa {MAX_SENTENCE_CHARS} ký tự")));
    }
    Ok(())
}

const STALE_RUN: &str = "đoạn này vừa được chép lại, tải lại để sửa tiếp";

/// `PUT /api/transcript-docs/{key}/segments/{fileId}` — lưu câu sửa của một đoạn.
///
/// Ghi có điều kiện: đoạn thuộc phiên, đã chép xong, `runId` là lượt hiện tại và số câu khớp. Sau khi
/// ghi đọc lại `runId`: nếu một lượt chép lại vừa chen vào thì bỏ phần vừa ghi và trả 409.
pub async fn save_segment(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path((key, file_id)): Path<(String, String)>,
    Json(req): Json<EditReq>,
) -> Result<Json<Value>, AppError> {
    let oid = ObjectId::parse_str(&file_id).map_err(|_| AppError::BadRequest("id đoạn không hợp lệ".into()))?;
    check_texts(&req.texts)?;
    let files = session_files(&state, &user.0, &key).await?;
    if !files.iter().any(|(_, f)| f.id == Some(oid)) {
        return Err(AppError::NotFound("đoạn không thuộc transcript này".into()));
    }
    let current = transcript::list_owned(&state.db, &user.0, &[oid]).await?.into_iter().next();
    let Some(t) = current.filter(|t| t.status == "done" && t.run_id == req.run_id) else {
        return Err(AppError::Conflict(STALE_RUN.into()));
    };
    if t.sentences.len() != req.texts.len() {
        return Err(AppError::Conflict("số câu không khớp bản chép hiện tại, tải lại để sửa tiếp".into()));
    }
    let edit = SegmentEdit { run_id: req.run_id, texts: req.texts };
    transcript_doc::set_edits(&state.db, &user.0, &key, oid, &edit).await?;

    let after = transcript::statuses_for_files(&state.db, &user.0, &[oid]).await?;
    if after.first().is_none_or(|r| r.run_id != edit.run_id) {
        // Chỉ gỡ đúng phần vừa ghi: phần sửa của lượt mới (nếu đã kịp lưu) giữ nguyên.
        transcript_doc::unset_edits_of_run(&state.db, &user.0, &key, oid, &edit.run_id).await?;
        return Err(AppError::Conflict(STALE_RUN.into()));
    }
    let details = doc! { "key": &key, "fileId": &file_id, "sentences": edit.texts.len() as i64 };
    audit::record(&state, Some(&user.0), "transcript.edit", details);
    Ok(Json(json!({ "ok": true })))
}

/// `DELETE /api/transcript-docs/{key}` — xoá kết quả chép lời của mọi đoạn và phần người dùng sửa;
/// audio giữ nguyên.
pub async fn delete(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path(key): Path<String>,
) -> Result<Json<Value>, AppError> {
    let files = session_files(&state, &user.0, &key).await?;
    let removed = transcript::delete_owned(&state.db, &user.0, &file_ids(&files)).await?;
    transcript_doc::delete(&state.db, &user.0, &key).await?;
    let details = doc! { "key": &key, "segments": files.len() as i64, "transcripts": removed as i64 };
    audit::record(&state, Some(&user.0), "transcript.delete", details);
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mongodb::bson::DateTime;

    fn recording(name: &str, duration_s: Option<f64>) -> FileDoc {
        FileDoc::new_recording("u", name, "/tmp/x", 1, duration_s.unwrap_or(0.0), false, ObjectId::new())
    }

    fn done(f: &FileDoc, run_id: &str, sentences: Vec<Sentence>) -> TranscriptDoc {
        TranscriptDoc {
            id: f.id.expect("id"),
            user_id: "u".into(),
            status: "done".into(),
            progress: 100.0,
            run_id: run_id.into(),
            error: None,
            text: String::new(),
            sentences,
            model: "m".into(),
            updated_at: DateTime::now(),
            charged_s: 0,
            sent_s: 0,
            month: None,
            free_pool: false,
            day: None,
            open: false,
            settled: true,
            attempts: 0,
        }
    }

    fn s(start: f64, end: f64, text: &str) -> Sentence {
        Sentence { start, end, text: text.into() }
    }

    #[test]
    fn segments_are_offset_by_earlier_durations_and_edits_follow_the_run() {
        let a = recording("aaaaaaaa-1111#0", Some(60.0));
        let b = recording("aaaaaaaa-1111#1", None); // durationS thiếu: lấy mốc cuối câu cuối
        let c = recording("aaaaaaaa-1111#3", Some(30.0));
        let mut transcripts = HashMap::new();
        transcripts.insert(a.id.expect("a"), done(&a, "run-a", vec![s(0.0, 2.0, "Một."), s(2.0, 4.0, "Hai.")]));
        transcripts.insert(b.id.expect("b"), done(&b, "run-b", vec![s(0.0, 45.5, "Ba.")]));
        let mut edits = HashMap::new();
        let a_hex = a.id.expect("a").to_hex();
        let b_hex = b.id.expect("b").to_hex();
        edits.insert(a_hex.clone(), SegmentEdit { run_id: "run-a".into(), texts: vec!["Một!".into(), "Hai!".into()] });
        // Sửa của lượt chép cũ: bỏ qua.
        edits.insert(b_hex, SegmentEdit { run_id: "run-old".into(), texts: vec!["Cũ.".into()] });

        let files = vec![(0, a), (1, b), (3, c)];
        let segments = assemble(&files, &transcripts, &edits);
        let offsets: Vec<f64> = segments.iter().map(|s| s.offset_s).collect();
        assert_eq!(offsets, vec![0.0, 60.0, 105.5]);
        assert!(segments[0].edited);
        assert_eq!(segments[0].edited_sentences, 2);
        assert_eq!(segments[0].sentences[1], s(2.0, 4.0, "Hai!"));
        assert!(!segments[1].edited);
        assert_eq!(segments[1].sentences[0].text, "Ba.");
        assert_eq!((segments[2].status.as_str(), segments[2].sentences.len()), ("none", 0));
        assert_eq!(segments[2].run_id, None);
        assert_eq!(missing_indexes("s:aaaaaaaa-1111", &files), vec![2]);
        assert!(missing_indexes("f:65f0a1b2c3d4e5f60718293a", &files).is_empty());
    }

    #[test]
    fn unfinished_segments_have_no_sentences_and_zero_duration_falls_back() {
        let a = recording("aaaaaaaa-1111#0", Some(0.0));
        let mut running = done(&a, "run-a", vec![s(0.0, 5.0, "Dở.")]);
        running.status = "running".into();
        running.progress = 40.0;
        let mut transcripts = HashMap::new();
        transcripts.insert(a.id.expect("a"), running);
        let segments = assemble(&[(0, a)], &transcripts, &HashMap::new());
        assert_eq!(segments[0].status, "running");
        assert!(segments[0].sentences.is_empty());
        assert_eq!(segments[0].duration_s, 0.0);
    }

    #[test]
    fn edits_with_a_different_sentence_count_are_ignored() {
        let a = recording("x", Some(10.0));
        let t = done(&a, "run-a", vec![s(0.0, 1.0, "Một."), s(1.0, 2.0, "Hai.")]);
        let edit = SegmentEdit { run_id: "run-a".into(), texts: vec!["Chỉ một.".into()] };
        let (sentences, edited, changed) = effective_sentences(&t, Some(&edit));
        assert!(!edited && changed == 0);
        assert_eq!(sentences, t.sentences);
    }

    #[test]
    fn titles_are_trimmed_and_capped() {
        assert_eq!(normalize_title(Some("  Họp tuần  ")).expect("ok").as_deref(), Some("Họp tuần"));
        assert_eq!(normalize_title(Some("   ")).expect("ok"), None);
        assert_eq!(normalize_title(None).expect("ok"), None);
        assert!(normalize_title(Some(&"ữ".repeat(120))).is_ok());
        assert!(normalize_title(Some(&"ữ".repeat(121))).is_err());
        assert!(check_texts(&["a".repeat(2000)]).is_ok());
        assert!(check_texts(&["a".repeat(2001)]).is_err());
        assert!(check_texts(&vec![String::new(); 5001]).is_err());
        assert!(check_key("s:../etc").is_err());
    }
}
