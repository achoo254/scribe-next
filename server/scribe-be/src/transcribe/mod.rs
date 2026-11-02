//! Chép lời bản ghi bằng key riêng của người dùng ("Key riêng", `user_keys`; chủ dự án chốt 02/11/2026: Scribe không trả
//! tiền AI thay người dùng): ElevenLabs Scribe v2 (cả tệp hoặc cả phiên iPhone trong một request, có người nói, xem
//! [`whole`]), OpenAI `gpt-transcribe` hoặc Google Gemini (cắt mảnh). Không có key hợp lệ cho chép lời ⟹ 403
//! `NO_USER_KEY`. Không có phút hay hạn mức nào; dung lượng, cỡ tệp, số tệp mỗi lần và rate limit vẫn áp.
//!
//! Lượt xếp hàng chỉ ghi nhà cung cấp; worker đọc và giải mã key của chủ phiên lúc chạy. Hàng đợi riêng (collection
//! `transcripts`, mỗi file một doc), chạy `TRANSCRIBE_CONCURRENCY` lượt cùng lúc — không chặn hàng đợi Cắt file. Backend
//! khởi động lại thì lượt dở chạy lại.

pub mod bookmarks;
pub mod chunking;
pub mod docs;
pub(crate) mod elevenlabs;
mod export;
pub(crate) mod gemini;
pub(crate) mod openai;
mod whole;
pub(crate) mod words;

use crate::audit;
use crate::auth::UserId;
use crate::config::{Provider, Vendor};
use crate::error::AppError;
use crate::files::{content_disposition_attachment, sanitize_filename};
use crate::models::translation::{self, Lang, TranslationDoc};
use crate::models::usage;
use crate::models::{file, transcript, transcript_doc, user};
use crate::state::AppState;
use axum::extract::{Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use chunking::Sentence;
use futures_util::{stream, StreamExt};
use mongodb::bson::{doc, oid::ObjectId, DateTime};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Notify};

/// Số mảnh gọi nhà cung cấp cùng lúc trong một file.
const PARALLEL: usize = 4;
/// Một lần bấm "Chép lời" tối đa bao nhiêu file (phiên ghi 3 giờ, đoạn 1 phút = 180 file).
const MAX_FILES: usize = 200;

/// Một file chép quá lâu thì dừng, khỏi chặn hàng đợi: 10 phút + thời lượng bản ghi.
const FILE_TIMEOUT_BASE_S: u64 = 600;

/// Giờ cho người dùng đọc (giờ Việt Nam).
pub(crate) fn vn_time(at: chrono::DateTime<chrono::Utc>) -> String {
    at.with_timezone(&usage::vietnam()).format("%H:%M ngày %d/%m").to_string()
}

/// Hàng đợi chép lời trong tiến trình: đánh thức vòng worker khi có lượt mới.
#[derive(Default)]
pub struct Transcriber {
    wake: Notify,
}

// MARK: API

#[derive(Deserialize)]
pub struct CreateReq {
    /// App cũ còn gửi kèm `apiKey`: serde bỏ qua trường lạ.
    #[serde(rename = "fileIds")]
    file_ids: Vec<String>,
}

/// `POST /api/transcripts` — xếp hàng chép các bản ghi của chính người dùng bằng key riêng của họ.
pub async fn create(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Json(req): Json<CreateReq>,
) -> Result<Json<Value>, AppError> {
    let (own, _) = crate::user_keys::required(&state, &user.0, crate::user_keys::Use::Transcribe).await?;
    let profile = user::find_profile(&state.db, &user.0)
        .await?
        .ok_or_else(|| AppError::Unauthorized("user không tồn tại".into()))?;
    if profile.blocked {
        return Err(AppError::Forbidden("tài khoản đã bị khoá".into()));
    }
    if req.file_ids.is_empty() || req.file_ids.len() > MAX_FILES {
        return Err(AppError::BadRequest(format!("chọn từ 1 đến {MAX_FILES} bản ghi")));
    }
    // Kiểm hết trước khi xếp hàng: một id sai thì không xếp file nào.
    let requested = parse_ids(&req.file_ids.join(","))?;
    let owned = file::list_owned_by_ids(&state.db, &user.0, &requested).await?;
    if owned.len() != requested.len() {
        return Err(AppError::NotFound("bản ghi không tồn tại".into()));
    }
    if owned.iter().any(|f| !f.in_history()) {
        return Err(AppError::BadRequest("chỉ chép lời được bản ghi hoặc tệp tải lên ở trang Ghi âm".into()));
    }
    // Cả phiên trong một request chỉ khi chép bằng ElevenLabs (có người nói); OpenAI và Gemini chép từng tệp.
    let whole_sessions = own == Vendor::ElevenLabs;
    let units = queue_units(&state, &user.0, owned, whole_sessions).await?;
    let files: Vec<ObjectId> = units.iter().flat_map(|u| u.files.iter().copied()).collect();
    let seconds: f64 = units.iter().flat_map(|u| u.docs.iter()).filter_map(|f| f.duration_s).filter(|d| *d > 0.0).sum();
    let rerun = transcript::statuses_for_files(&state.db, &user.0, &files).await?.len();

    let model = own_model(own);
    let mut queued = 0;
    for unit in &units {
        // Một lượt mỗi đơn vị: cả phiên iPhone chung một `runId` để worker gom lại thành một request. Xếp ở trạng thái
        // chờ bật rồi bật cùng lúc: worker không nhận lượt khi mới xếp được một phần (phiên bị tách làm nhiều request).
        let run_id = uuid::Uuid::new_v4().to_string();
        for &oid in &unit.files {
            let ok = match transcript::enqueue_held(&state.db, oid, &user.0, &run_id, model, own).await {
                Ok(ok) => ok,
                // Mongo standalone có thể báo lỗi dù đã ghi: đọc lại theo `runId`.
                Err(e) => {
                    tracing::warn!(file = %oid, "xếp hàng lỗi, kiểm lại: {e:?}");
                    transcript::has_run(&state.db, oid, &run_id).await.unwrap_or(false)
                }
            };
            if ok {
                queued += 1;
            }
        }
        // Bật lỗi thì lượt vẫn ở trạng thái chờ bật: lần khởi động sau bật hộ (`requeue_interrupted`).
        if let Err(e) = transcript::release_held(&state.db, &user.0, &run_id).await {
            tracing::error!(run = %run_id, "bật lượt chép lời lỗi: {e:?}");
        }
    }
    if queued == 0 {
        return Err(AppError::Conflict("các bản ghi này đang được chép".into()));
    }
    state.transcriber.wake.notify_one();
    let details = doc! { "files": queued as i64, "seconds": seconds.ceil() as i64, "byok": own.code() };
    audit::record(&state, Some(&user.0), "transcript.create", details);
    if rerun > 0 {
        audit::record(&state, Some(&user.0), "transcript.rerun", doc! { "files": rerun as i64 });
    }
    // `files`: số file đã xếp (gồm các đoạn kéo theo khi chép cả phiên), `queued` giữ cho app cũ (cùng số).
    Ok(Json(json!({ "queued": queued, "files": queued, "skipped": requested.len().saturating_sub(queued) })))
}

/// Model ghi vào lượt chép bằng key riêng (Claude không chép lời; `Use::Transcribe` không bao giờ chọn nó).
fn own_model(own: Vendor) -> &'static str {
    match own {
        Vendor::ElevenLabs => Provider::ElevenLabs.model(),
        Vendor::Gemini => gemini::MODEL,
        _ => openai::USER_MODEL,
    }
}

/// Một đơn vị xếp hàng: một file, hoặc mọi đoạn của một phiên iPhone khi chép cả phiên trong một request.
struct QueueUnit {
    /// Theo thứ tự đoạn.
    files: Vec<ObjectId>,
    docs: Vec<file::FileDoc>,
}

/// Gom file cần xếp thành đơn vị. Chép cả phiên (`whole_sessions`): một đoạn bất kỳ của phiên iPhone kéo theo mọi
/// đoạn hiện có của phiên — đoạn tới muộn làm chép lại cả phiên, để nhãn người nói nhất quán. Phiên còn đoạn đang
/// chờ/chạy thì bỏ cả phiên (lượt đang chạy chưa gồm đoạn mới).
async fn queue_units(
    state: &AppState,
    user_id: &str,
    owned: Vec<file::FileDoc>,
    whole_sessions: bool,
) -> Result<Vec<QueueUnit>, AppError> {
    let mut units: Vec<QueueUnit> = Vec::new();
    let mut seen_sessions = std::collections::HashSet::new();
    for f in owned {
        let Some(id) = f.id else { continue };
        let key = f.session_key().unwrap_or_default();
        if !whole_sessions || !key.starts_with("s:") {
            units.push(QueueUnit { files: vec![id], docs: vec![f] });
            continue;
        }
        if !seen_sessions.insert(key.clone()) {
            continue;
        }
        let parts: Vec<file::FileDoc> = file::list_session(&state.db, user_id, &key).await?.into_iter().map(|(_, f)| f).collect();
        let ids: Vec<ObjectId> = parts.iter().filter_map(|p| p.id).collect();
        let busy = transcript::statuses_for_files(&state.db, user_id, &ids)
            .await?
            .iter()
            .any(|r| transcript::ACTIVE.contains(&r.status.as_str()));
        if !busy && !ids.is_empty() {
            units.push(QueueUnit { files: ids, docs: parts });
        }
    }
    Ok(units)
}

#[derive(Deserialize)]
pub struct IdsQuery {
    /// Id file, cách nhau bằng dấu phẩy, theo thứ tự ghép.
    ids: String,
}

#[derive(Deserialize)]
pub struct ExportQuery {
    /// Id file theo thứ tự ghép; hoặc `session` thay cho danh sách này.
    ids: Option<String>,
    /// Phiên ghi iPhone: mọi đoạn `<session>#<index>` của người dùng, theo `index` (không giới hạn số đoạn).
    session: Option<String>,
    format: Option<String>,
    name: Option<String>,
    /// Xuất bản dịch (`en` | `vi` | `zh-CN`) thay cho chữ gốc; chỉ khi mọi file cùng một phiên.
    lang: Option<String>,
    /// Ngôn ngữ giao diện của người tải (mặc định `vi`): nhãn mục md/docx khi xuất chữ gốc.
    ui: Option<String>,
}

fn parse_ids(raw: &str) -> Result<Vec<ObjectId>, AppError> {
    let ids: Vec<ObjectId> = raw
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| ObjectId::parse_str(s.trim()).map_err(|_| AppError::BadRequest("id không hợp lệ".into())))
        .collect::<Result<_, _>>()?;
    if ids.is_empty() || ids.len() > MAX_FILES {
        return Err(AppError::BadRequest(format!("cần từ 1 đến {MAX_FILES} id")));
    }
    Ok(ids)
}

/// `GET /api/transcripts?ids=a,b` — trạng thái và nội dung chép lời của từng file.
pub async fn get(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Query(q): Query<IdsQuery>,
) -> Result<Json<Value>, AppError> {
    let ids = parse_ids(&q.ids)?;
    let docs = transcript::list_owned(&state.db, &user.0, &ids).await?;
    let items: Vec<Value> = docs
        .iter()
        .map(|t| {
            // Không trả `text` (trùng nội dung với `sentences`): trang hỏi lại mỗi 2 giây khi đang chép.
            json!({
                "fileId": t.file_id(),
                "status": transcript::public_status(&t.status),
                "progress": t.progress,
                "error": t.error,
                "sentences": t.sentences,
            })
        })
        .collect();
    Ok(Json(json!({ "items": items })))
}

/// `GET /api/transcripts/export?ids=a,b&format=txt|srt|vtt|md|docx&name=…` — một tệp ghép các file theo
/// thứ tự, mốc của file sau cộng dồn thời lượng các file trước. File chưa chép xong thì bỏ qua (mốc vẫn cộng).
/// `md`/`docx` là tài liệu cuộc họp: kèm biên bản và dấu mốc khi mọi file thuộc cùng một phiên.
/// `lang=en|vi|zh-CN`: câu thay bằng bản dịch đã lưu của phiên (câu chưa dịch giữ chữ gốc, câu có bản dịch cũ vẫn
/// dùng bản dịch đó như màn hình đọc), nhãn mục md/docx theo ngôn ngữ, biên bản giữ nguyên ngôn ngữ gốc. Không có
/// `lang` thì nhãn mục theo `ui=` (ngôn ngữ giao diện, mặc định tiếng Việt).
pub async fn export(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Query(q): Query<ExportQuery>,
) -> Result<Response, AppError> {
    let format = export::Format::parse(q.format.as_deref().unwrap_or("txt"))
        .ok_or_else(|| AppError::BadRequest("định dạng chỉ nhận txt, srt, vtt, md hoặc docx".into()))?;
    let lang = match q.lang.as_deref().filter(|l| !l.is_empty()) {
        Some(raw) => Some(Lang::parse(raw).ok_or_else(|| AppError::BadRequest(crate::lang::UNKNOWN.into()))?),
        None => None,
    };
    let ui = crate::lang::from_request(q.ui.as_deref())?;
    let files = export_files(&state, &user.0, &q).await?;
    let ids: Vec<ObjectId> = files.iter().filter_map(|f| f.id).collect();
    let docs = transcript::list_owned(&state.db, &user.0, &ids).await?;
    let by_id: HashMap<ObjectId, &transcript::TranscriptDoc> = docs.iter().map(|t| (t.id, t)).collect();
    // Câu sửa theo phiên của từng file (một truy vấn), áp theo luật `runId` như màn hình đọc.
    let keys: Vec<String> = files.iter().filter_map(file::FileDoc::session_key).collect();
    let edits: HashMap<String, transcript_doc::SessionDoc> = transcript_doc::get_many(&state.db, &user.0, &keys)
        .await?
        .into_iter()
        .map(|d| (d.key.clone(), d))
        .collect();

    // Biên bản, dấu mốc, tiêu đề và bản dịch thuộc một phiên: chỉ dùng khi mọi file cùng phiên.
    let session_key = match keys.first() {
        Some(first) if keys.iter().all(|k| k == first) => Some(first.as_str()),
        _ => None,
    };
    let translated = match lang {
        Some(lang) => {
            let key = session_key.ok_or_else(|| AppError::BadRequest("chỉ xuất bản dịch cho một bản ghi".into()))?;
            let doc = translation::get(&state.db, &user.0, key, lang)
                .await?
                .ok_or_else(|| AppError::NotFound("bản ghi này chưa có bản dịch sang ngôn ngữ đó".into()))?;
            Some(doc)
        }
        None => None,
    };

    let mut segments: Vec<(f64, Vec<Sentence>)> = Vec::new();
    let mut offset = 0.0;
    for f in &files {
        let Some(id) = f.id else { continue };
        let done = by_id.get(&id).filter(|t| t.status == "done");
        let sentences = match done {
            Some(t) => {
                let session = edits.get(&file::session_key(&f.kind, &f.name, id));
                let edit = session.and_then(|d| d.edits.get(&id.to_hex()));
                let sentences = docs::name_speakers(docs::effective_sentences(t, edit).0, session.map(|d| &d.speakers));
                match &translated {
                    Some(doc) => translate_sentences(sentences, doc, &id.to_hex()),
                    None => sentences,
                }
            }
            None => Vec::new(),
        };
        let duration = docs::segment_duration(f, &sentences);
        if done.is_some() {
            segments.push((offset, sentences));
        }
        offset += duration;
    }
    let parts: Vec<export::Part<'_>> =
        segments.iter().map(|(offset_s, sentences)| export::Part { offset_s: *offset_s, sentences }).collect();
    if parts.is_empty() {
        return Err(AppError::NotFound("chưa có bản chép lời nào xong".into()));
    }
    let session_doc = session_key.and_then(|key| edits.get(key));
    let labels = export::labels(lang.unwrap_or(ui));
    // Tiêu đề người dùng đặt cho phiên, rồi tên client gửi (tên file ở dòng lịch sử ghi).
    let title = session_doc
        .and_then(|d| d.title.clone())
        .or_else(|| q.name.clone())
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty());
    let body: Vec<u8> = match format {
        export::Format::Txt => export::to_txt(&parts).into_bytes(),
        export::Format::Srt => export::to_srt(&parts).into_bytes(),
        export::Format::Vtt => export::to_vtt(&parts).into_bytes(),
        export::Format::Md | export::Format::Docx => {
            let mut bookmarks = session_doc.map(|d| d.bookmarks.clone()).unwrap_or_default();
            bookmarks.sort_by(|a, b| a.at.total_cmp(&b.at));
            let meeting = export::Meeting {
                title: title.as_deref().unwrap_or(labels.untitled),
                started: files.iter().map(|f| f.created_at).min().and_then(|at| {
                    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(at.timestamp_millis())
                        .map(|at| at.with_timezone(&usage::vietnam()).format("%H:%M %d/%m/%Y").to_string())
                }),
                duration_s: offset,
                summary: session_doc.and_then(|d| d.summary.as_ref()),
                bookmarks: &bookmarks,
                labels,
            };
            if format == export::Format::Md {
                export::to_md(&meeting, &parts).into_bytes()
            } else {
                export::to_docx(&meeting, &parts).map_err(AppError::Internal)?
            }
        }
    };
    let base = sanitize_filename(title.as_deref().unwrap_or("chep-loi"));
    let base = match lang {
        Some(lang) => format!("{base}-{}", lang.code()),
        None => base,
    };
    Ok((
        [
            (header::CONTENT_TYPE, header::HeaderValue::from_static(format.mime())),
            (header::CONTENT_DISPOSITION, content_disposition_attachment(&format!("{base}.{}", format.ext()))),
        ],
        body,
    )
        .into_response())
}

/// Câu của một đoạn với chữ thay bằng bản dịch đã lưu (cùng chỉ số câu); câu chưa dịch giữ chữ gốc.
fn translate_sentences(sentences: Vec<Sentence>, doc: &TranslationDoc, file_id: &str) -> Vec<Sentence> {
    sentences
        .into_iter()
        .enumerate()
        .map(|(idx, s)| match doc.item(file_id, idx).filter(|_| !s.text.trim().is_empty()) {
            Some(item) => Sentence { text: item.t.clone(), ..s },
            None => s,
        })
        .collect()
}

/// File cần xuất, theo thứ tự ghép.
async fn export_files(state: &AppState, user_id: &str, q: &ExportQuery) -> Result<Vec<file::FileDoc>, AppError> {
    if let Some(session) = q.session.as_deref().filter(|s| !s.is_empty()) {
        // Gom phiên cùng một nguồn với `/api/transcript-docs` (`file::session_key`).
        let key = format!("s:{session}");
        if !file::is_valid_session_key(&key) {
            return Err(AppError::BadRequest("phiên ghi không hợp lệ".into()));
        }
        let parts = file::list_session(&state.db, user_id, &key).await?;
        return Ok(parts.into_iter().map(|(_, f)| f).collect());
    }
    let ids = parse_ids(q.ids.as_deref().unwrap_or(""))?;
    let mut by_id: HashMap<ObjectId, file::FileDoc> = file::list_owned_by_ids(&state.db, user_id, &ids)
        .await?
        .into_iter()
        .filter_map(|f| Some((f.id?, f)))
        .collect();
    Ok(ids.iter().filter_map(|id| by_id.remove(id)).collect())
}

// MARK: Worker

/// Khởi động: lượt bị cắt ngang được xếp lại (lượt đã bị cắt hai lần thì dừng); rồi chạy hàng đợi nền.
pub async fn start_worker(state: AppState) {
    match transcript::requeue_interrupted(&state.db).await {
        Ok((requeued, stopped)) if requeued + stopped > 0 => {
            tracing::info!("chép lời dở sau khởi động lại: {requeued} lượt chạy lại, {stopped} lượt dừng")
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("xếp lại chép lời dở lỗi: {e:?}"),
    }
    let running = Arc::new(Mutex::new(HashSet::new()));
    for _ in 0..state.config.transcribe_concurrency {
        tokio::spawn(worker_loop(state.clone(), running.clone()));
    }
}

/// Lượt (`runId`) đang chạy ở các vòng worker của tiến trình này.
type RunningRuns = Arc<Mutex<HashSet<String>>>;

/// Thời gian một file chờ trước khi được nhận lại, khi lượt của nó đang chạy ở vòng khác.
const BUSY_RUN_RETRY_MS: i64 = 5_000;

/// Một vòng hàng đợi; [`start_worker`] chạy `TRANSCRIBE_CONCURRENCY` vòng song song.
///
/// Một lượt chỉ chạy ở một vòng. Lượt cả phiên ElevenLabs tự gom các file khác của nó
/// ([`transcript::claim_run_members`]); với OpenAI các file của phiên chép lần lượt. Vì vậy vòng nhận phải file
/// của lượt đang chạy thì trả file đó về hàng đợi. Nhận lượt và kiểm danh sách nằm dưới cùng một khoá, để hai vòng
/// không cùng bắt đầu một lượt.
async fn worker_loop(state: AppState, running: RunningRuns) {
    loop {
        let claimed = {
            let mut busy = running.lock().await;
            match transcript::claim_next(&state.db).await {
                Ok(Some(t)) if busy.contains(&t.run_id) => Claim::Busy(t),
                Ok(Some(t)) => {
                    busy.insert(t.run_id.clone());
                    Claim::Run(t)
                }
                Ok(None) => Claim::Empty,
                Err(e) => Claim::Error(e),
            }
        };
        match claimed {
            Claim::Run(t) => {
                // Còn lượt chờ thì đánh thức một vòng đang rảnh, khỏi đợi hết 5 giây.
                state.transcriber.wake.notify_one();
                let (file_id, run_id, user_id) = (t.id, t.run_id.clone(), t.user_id.clone());
                let st = state.clone();
                // Panic trong một lượt không được giết vòng hàng đợi. Lượt cả phiên có nhiều file: đóng hết, không
                // chỉ file worker đã nhận (các file khác sẽ kẹt `running` tới lần khởi động sau).
                if let Err(e) = tokio::spawn(async move { run(&st, &t).await }).await {
                    tracing::error!(file = %file_id, "chép lời panic: {e}");
                    match transcript::fail_open_run(&state.db, &user_id, &run_id, INTERNAL_ERROR).await {
                        Ok(_) => {}
                        Err(e) => {
                            tracing::error!(file = %file_id, "đóng lượt sau panic lỗi: {e:?}");
                            record(&state.db, file_id, &run_id, Outcome::Failed(INTERNAL_ERROR)).await;
                        }
                    }
                }
                running.lock().await.remove(&run_id);
            }
            Claim::Busy(t) => {
                let until = DateTime::from_millis(DateTime::now().timestamp_millis() + BUSY_RUN_RETRY_MS);
                if let Err(e) = transcript::requeue_claimed(&state.db, t.id, &t.run_id, until).await {
                    tracing::error!(file = %t.id, "trả file của lượt đang chạy về hàng đợi lỗi: {e:?}");
                }
            }
            Claim::Empty => {
                let _ = tokio::time::timeout(Duration::from_secs(5), state.transcriber.wake.notified()).await;
            }
            Claim::Error(e) => {
                tracing::error!("lấy lượt chép lời lỗi: {e}");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}

enum Claim {
    Run(transcript::TranscriptDoc),
    /// File thuộc lượt đang chạy ở vòng khác.
    Busy(transcript::TranscriptDoc),
    Empty,
    Error(mongodb::error::Error),
}

/// Người dùng thấy câu này khi một lượt dừng vì lỗi lập trình (panic).
const INTERNAL_ERROR: &str = "lỗi nội bộ khi chép";

enum Outcome<'a> {
    Done { text: &'a str, sentences: &'a [Sentence] },
    Failed(&'a str),
}

/// Ghi kết quả lượt chép; Mongo lỗi chốc lát thì thử lại, kẻo lượt kẹt `running` và trang hỏi mãi.
async fn record(db: &mongodb::Database, file_id: ObjectId, run_id: &str, outcome: Outcome<'_>) {
    for attempt in 1..=3 {
        let written = match outcome {
            Outcome::Done { text, sentences } => transcript::finish_done(db, file_id, run_id, text, sentences).await,
            Outcome::Failed(error) => transcript::finish_failed(db, file_id, run_id, error).await,
        };
        match written {
            Ok(_) => return,
            Err(e) if attempt < 3 => {
                tracing::warn!(file = %file_id, "ghi kết quả chép lời lỗi, thử lại: {e:?}");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(e) => tracing::error!(file = %file_id, "ghi kết quả chép lời lỗi: {e:?}"),
        }
    }
}

/// Câu cho lượt cũ thời key server còn dở lúc nâng cấp: không còn key server để chạy.
pub(crate) const NEEDS_OWN_KEY: &str = "Chép lời giờ chạy bằng key riêng của bạn. Thêm key ở Key riêng rồi bấm Chép lời lại.";

async fn run(state: &AppState, t: &transcript::TranscriptDoc) {
    match t.own_provider() {
        Some(Vendor::ElevenLabs) => whole::run(state, t).await,
        Some(_) => run_chunked(state, t).await,
        // Lượt cũ thời key server (xếp trước khi nâng cấp): dừng, người dùng thêm key rồi chép lại.
        None => {
            record(&state.db, t.id, &t.run_id, Outcome::Failed(NEEDS_OWN_KEY)).await;
            crate::search::index::schedule_file(&state.db, &t.user_id, t.id);
        }
    }
}

/// Cách chép từng mảnh của một tệp.
#[derive(Clone, Copy)]
enum Engine<'a> {
    /// Key riêng OpenAI: `gpt-transcribe`.
    OwnOpenAi(&'a str),
    /// Key riêng Gemini: câu kèm mốc ước lượng.
    OwnGemini(&'a str),
}

/// Lượt chép theo mảnh (OpenAI hoặc Gemini). Key riêng đọc và giải mã lúc chạy; lỗi key chỉ đánh dấu key đó.
async fn run_chunked(state: &AppState, t: &transcript::TranscriptDoc) {
    let started = Instant::now();
    let Some(provider) = t.own_provider() else { return };
    let key = match crate::user_keys::open(state, &t.user_id, provider).await {
        Ok(key) => key,
        Err(message) => {
            record(&state.db, t.id, &t.run_id, Outcome::Failed(&message)).await;
            crate::search::index::schedule_file(&state.db, &t.user_id, t.id);
            return;
        }
    };
    let engine = if key.provider == Vendor::Gemini { Engine::OwnGemini(key.value()) } else { Engine::OwnOpenAi(key.value()) };
    let result = match transcribe_file(state, t, engine).await {
        Err(e) => match crate::user_keys::problem_of(&e) {
            Some(problem) => Err(crate::user_keys::report(state, &key, problem).await),
            None => Err(e),
        },
        result => result,
    };
    drop(key);
    let db = &state.db;
    match result {
        Err(e) if e == CANCELLED => {
            tracing::info!(file = %t.file_id(), "lượt chép lời bị thay hoặc file bị xoá, dừng");
            return;
        }
        Ok((text, sentences, chunks)) => {
            record(db, t.id, &t.run_id, Outcome::Done { text: &text, sentences: &sentences }).await;
            let details = doc! {
                "fileId": t.file_id(),
                "chunks": chunks as i64,
                "seconds": started.elapsed().as_secs() as i64,
            };
            audit::record(state, Some(&t.user_id), "transcript.done", details);
        }
        Err(e) => {
            tracing::warn!(file = %t.file_id(), "chép lời lỗi: {e}");
            record(db, t.id, &t.run_id, Outcome::Failed(&e)).await;
            audit::record(state, Some(&t.user_id), "transcript.failed", doc! { "fileId": t.file_id(), "error": &e });
        }
    }
    // Lượt kết thúc (xong hay lỗi): câu của đoạn đổi ⟹ dựng lại index tìm kiếm của phiên.
    crate::search::index::schedule_file(db, &t.user_id, t.id);
}

async fn transcribe_file(
    state: &AppState,
    t: &transcript::TranscriptDoc,
    engine: Engine<'_>,
) -> Result<(String, Vec<Sentence>, usize), String> {
    // Khoá sau khi đã xếp hàng: không gửi audio nữa; lượt đóng với 0 giây đã gửi nên được hoàn hết.
    if user::find_profile(&state.db, &t.user_id).await.ok().flatten().is_some_and(|p| p.blocked || p.deleting) {
        return Err("tài khoản đã bị khoá".into());
    }
    let f = file::get_owned(&state.db, &t.file_id(), &t.user_id)
        .await
        .map_err(|e| {
            tracing::warn!(file = %t.file_id(), "đọc bản ghi để chép lỗi: {e:?}");
            "không đọc được bản ghi, thử lại sau".to_string()
        })?
        .ok_or(CANCELLED)?;
    // Đo lại thời lượng từ chính tệp: `durationS` của bản ghi có thể do client báo, sai thì mảnh
    // rơi ra ngoài tệp (OpenAI trả 400) hoặc bỏ sót phần cuối.
    let duration = match core_split::probe_file(&state.config.ffprobe_path, &f.path).await {
        Ok(info) if info.duration_s > 0.0 => info.duration_s,
        _ => f.duration_s.filter(|d| *d > 0.0).ok_or("không đọc được thời lượng bản ghi")?,
    };
    let progress = Progress { db: &state.db, file_id: t.id, run_id: &t.run_id };
    let limit = Duration::from_secs(FILE_TIMEOUT_BASE_S + duration as u64);
    let work = transcribe_path(&state.http, &state.config.ffmpeg_path, &f.path, duration, engine, Some(progress));
    tokio::time::timeout(limit, work)
        .await
        .map_err(|_| "chép quá lâu nên đã dừng, hãy thử lại".to_string())?
}

/// Lượt không còn hiệu lực (xếp lại, hoặc file bị xoá): dừng, không ghi gì.
const CANCELLED: &str = "lượt chép lời đã bị thay";

/// Nơi ghi tiến độ và số giây đã gửi của một lượt chép.
struct Progress<'a> {
    db: &'a mongodb::Database,
    file_id: ObjectId,
    run_id: &'a str,
}

impl Progress<'_> {
    /// Cộng giây đã gửi; lượt đã bị thay hoặc file bị xoá thì dừng (khỏi tốn tiền thêm).
    async fn count_sent(&self, seconds: i64) -> Result<(), String> {
        match transcript::add_sent(self.db, self.file_id, self.run_id, seconds).await {
            Ok(true) => Ok(()),
            Ok(false) => Err(CANCELLED.into()),
            Err(e) => {
                tracing::warn!(file = %self.file_id, "ghi giây đã gửi lỗi: {e:?}");
                Err("không ghi được sổ phút, thử lại sau".into())
            }
        }
    }
}

/// Chép một tệp audio/video: tìm khoảng lặng, cắt mảnh, gọi nhà cung cấp song song, ghép câu có mốc. Mảnh hoàn toàn im
/// lặng (đỉnh < `SILENT_PEAK_DB`) không gửi và không tính vào giây đã gửi. Trả (chữ liền, các câu, số mảnh đã gửi).
async fn transcribe_path(
    http: &reqwest::Client,
    ffmpeg: &str,
    path: &str,
    duration: f64,
    engine: Engine<'_>,
    progress: Option<Progress<'_>>,
) -> Result<(String, Vec<Sentence>, usize), String> {
    let speech = openai::speech_spans(ffmpeg, path, duration).await?;
    let chunks: Vec<chunking::Span> = chunking::plan_chunks(&speech, duration)
        .into_iter()
        .filter(|&c| chunking::has_speech(c, &speech))
        .collect();

    let total = chunks.len();
    let mut texts = Vec::with_capacity(total);
    let progress = progress.as_ref();
    // `buffered` giữ thứ tự; lỗi một mảnh thì dừng và bỏ các mảnh đang chờ (khỏi tốn tiền vô ích).
    let mut results = stream::iter(chunks.iter().copied())
        .map(|chunk| async move {
            let (audio, peak) = openai::encode_chunk(ffmpeg, path, chunk).await?;
            if peak.is_some_and(|p| p < chunking::SILENT_PEAK_DB) {
                return Ok::<_, String>((Chunk::Text(String::new()), false));
            }
            let seconds = (chunk.1 - chunk.0).ceil() as i64;
            // Ghi giây sắp gửi TRƯỚC khi gửi: số phút người dùng thấy gồm cả phần đã gửi khi tiến trình chết.
            if let Some(p) = progress {
                p.count_sent(seconds).await?;
            }
            let text = match engine {
                Engine::OwnOpenAi(key) => Chunk::Text(openai::transcribe_once(http, key, &audio).await?),
                Engine::OwnGemini(key) => Chunk::Timed(gemini::transcribe_chunk(http, key, &audio, chunk).await?),
            };
            Ok((text, true))
        })
        .buffered(PARALLEL);
    let mut sent = 0;
    while let Some(result) = results.next().await {
        let (text, was_sent) = result?;
        sent += usize::from(was_sent);
        texts.push(text);
        if let Some(p) = progress {
            let pct = (texts.len() as f64 / total as f64 * 100.0).min(99.0).floor();
            // Lượt đã bị thay hoặc file bị xoá: dừng để khỏi tốn tiền của người dùng.
            if let Ok(false) = transcript::set_progress(p.db, p.file_id, p.run_id, pct).await {
                return Err(CANCELLED.into());
            }
        }
    }

    let sentences: Vec<Sentence> = chunks
        .iter()
        .zip(texts)
        .flat_map(|(&chunk, text)| match text {
            Chunk::Text(text) => chunking::sentences(&text, chunk, &speech),
            Chunk::Timed(sentences) => sentences,
        })
        .collect();
    let text = words::joined_text(&sentences);
    Ok((text, sentences, sent))
}

/// Kết quả chép một mảnh: chữ liền (mốc câu ước lượng theo độ dài chữ) hoặc câu đã có mốc (Gemini).
enum Chunk {
    Text(String),
    Timed(Vec<Sentence>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_of_a_translation_replaces_text_by_sentence_index() {
        use crate::models::translation::Item;
        let s = |start: f64, text: &str| Sentence { start, end: start + 1.0, text: text.into(), speaker: None };
        let mut items = HashMap::new();
        items.insert(
            "0".to_string(),
            Item { h: crate::translate::source_hash("Một."), t: "One.".into() },
        );
        // Bản dịch cũ (câu nguồn đã sửa) vẫn được xuất, như màn hình đọc.
        items.insert("2".to_string(), Item { h: "khác".into(), t: "Three (old).".into() });
        let doc = TranslationDoc {
            id: ObjectId::new(),
            lang: "en".into(),
            model: "m".into(),
            status: "done".into(),
            done: 2,
            total: 2,
            error: None,
            updated_at: mongodb::bson::DateTime::from_millis(0),
            items: HashMap::from([("abc".to_string(), items)]),
        };
        let out = translate_sentences(vec![s(0.0, "Một."), s(2.0, "Hai."), s(4.0, "Ba đã sửa.")], &doc, "abc");
        assert_eq!(out.iter().map(|x| x.text.as_str()).collect::<Vec<_>>(), vec!["One.", "Hai.", "Three (old)."]);
        assert_eq!(out[2].start, 4.0, "giữ mốc câu");
        let parts = [export::Part { offset_s: 60.0, sentences: &out }];
        assert_eq!(
            export::to_srt(&parts).lines().take(3).collect::<Vec<_>>(),
            vec!["1", "00:01:00,000 --> 00:01:01,000", "One."]
        );
        // Đoạn khác chưa dịch: giữ chữ gốc.
        assert_eq!(translate_sentences(vec![s(0.0, "Một.")], &doc, "other")[0].text, "Một.");
    }

    /// Không có key riêng cho chép lời (tính năng Key riêng tắt ⟹ không đọc Mongo): 403 `NO_USER_KEY`, không xếp gì.
    #[tokio::test]
    async fn without_an_own_transcribe_key_nothing_is_queued() {
        let db = mongodb::Client::with_uri_str("mongodb://127.0.0.1:9/?serverSelectionTimeoutMS=200&connectTimeoutMS=200")
            .await
            .expect("uri")
            .database("scribe_transcribe_test");
        let state = AppState::for_tests(crate::config::Config::for_tests("http://localhost", "http://localhost"), db);
        let req = CreateReq { file_ids: vec![ObjectId::new().to_hex()] };
        let Err(AppError::Coded { status, code, message }) = create(State(state), Extension(UserId("u1".into())), Json(req)).await
        else {
            panic!("phải từ chối");
        };
        assert_eq!((status, code), (axum::http::StatusCode::FORBIDDEN, "NO_USER_KEY"));
        assert!(message.contains("Key riêng"), "{message}");
    }

    /// Chép thật qua OpenAI bằng key của người chạy test (tốn tiền, cần mạng): `OPENAI_API_KEY_FILE=<tệp chứa key>
    /// TRANSCRIBE_SAMPLE=<tệp audio> TRANSCRIBE_OUT=<tệp txt> cargo test live_openai -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn live_openai_transcribes_sample() {
        let key_file = std::env::var("OPENAI_API_KEY_FILE").expect("OPENAI_API_KEY_FILE");
        let key = std::fs::read_to_string(key_file).expect("đọc key");
        let sample = std::env::var("TRANSCRIBE_SAMPLE").expect("TRANSCRIBE_SAMPLE");
        let duration = core_split::probe_file("ffprobe", &sample).await.expect("ffprobe").duration_s;
        let started = std::time::Instant::now();
        let (text, sentences, chunks) =
            super::transcribe_path(&reqwest::Client::new(), "ffmpeg", &sample, duration, super::Engine::OwnOpenAi(key.trim()), None)
                .await
                .expect("chép lời");
        println!("{chunks} mảnh, {} câu, {:.1}s", sentences.len(), started.elapsed().as_secs_f64());
        if let Ok(out) = std::env::var("TRANSCRIBE_OUT") {
            std::fs::write(&out, &text).expect("ghi txt");
            let parts = [super::export::Part { offset_s: 0.0, sentences: &sentences }];
            std::fs::write(format!("{out}.srt"), super::export::to_srt(&parts)).expect("ghi srt");
        }
        assert!(!text.is_empty());
    }
}
