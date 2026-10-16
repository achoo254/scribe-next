//! Chép lời bản ghi bằng OpenAI `gpt-4o-mini-transcribe`, key do người dùng tự nhập.
//!
//! Key đi kèm request `POST /api/transcripts` và chỉ nằm trong RAM ([`PendingKeys`]) tới khi lượt chép
//! của file đó xong: không ghi Mongo, không ghi log/audit. Backend khởi động lại thì các lượt dở báo
//! lỗi và người dùng bấm lại. Chỉ email trong `TRANSCRIBE_EMAILS` được dùng (tiền là của họ, nhưng
//! tính năng mở cho ai do chủ sở hữu quyết).
//!
//! Hàng đợi riêng (collection `transcripts`), chạy tuần tự từng file, mỗi file gọi song song
//! [`PARALLEL`] mảnh — không chặn hàng đợi Cắt file.

pub mod chunking;
pub mod docs;
mod export;
mod openai;

use crate::audit;
use crate::auth::UserId;
use crate::error::AppError;
use crate::files::{content_disposition_attachment, sanitize_filename};
use crate::models::{file, transcript, transcript_doc, user};
use crate::state::AppState;
use axum::extract::{Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use chunking::Sentence;
use futures_util::{stream, StreamExt};
use mongodb::bson::{doc, oid::ObjectId};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};
use tokio::sync::Notify;

/// Số mảnh gọi OpenAI cùng lúc trong một file.
const PARALLEL: usize = 4;
/// Một lần bấm "Chép lời" tối đa bao nhiêu file (phiên ghi 3 giờ, đoạn 1 phút = 180 file).
const MAX_FILES: usize = 200;

/// Hàng đợi rỗng mà key còn nằm lâu hơn mức này thì lượt của nó đã mất (file bị xoá, xếp lại): bỏ key.
const STALE_KEY: Duration = Duration::from_secs(60);
/// Một file chép quá lâu thì dừng, khỏi chặn hàng đợi: 10 phút + thời lượng bản ghi.
const FILE_TIMEOUT_BASE_S: u64 = 600;

/// API key của người dùng. Không có `Debug`/`Display` để khỏi lỡ in ra log.
struct ApiKey(String);

struct PendingKey {
    run_id: String,
    key: Arc<ApiKey>,
    stored_at: Instant,
}

/// Key đang chờ dùng, theo file + lượt chép. Mất khi tiến trình dừng (cố ý).
#[derive(Default)]
pub struct PendingKeys {
    keys: Mutex<HashMap<ObjectId, PendingKey>>,
    wake: Notify,
}

impl PendingKeys {
    /// Map chỉ chứa dữ liệu thuần: một luồng panic giữa chừng không làm nó sai, nên dùng tiếp.
    fn lock(&self) -> MutexGuard<'_, HashMap<ObjectId, PendingKey>> {
        self.keys.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn put(&self, file_id: ObjectId, run_id: &str, key: &Arc<ApiKey>) {
        let pending = PendingKey { run_id: run_id.to_string(), key: Arc::clone(key), stored_at: Instant::now() };
        self.lock().insert(file_id, pending);
    }

    /// Lấy và xoá key của đúng lượt `run_id`.
    fn take(&self, file_id: ObjectId, run_id: &str) -> Option<Arc<ApiKey>> {
        let mut keys = self.lock();
        match keys.get(&file_id) {
            Some(p) if p.run_id == run_id => keys.remove(&file_id).map(|p| p.key),
            _ => None,
        }
    }

    /// Bỏ key của lượt không xếp hàng được.
    fn forget(&self, file_id: ObjectId, run_id: &str) {
        drop(self.take(file_id, run_id));
    }

    /// Gọi khi hàng đợi rỗng: key còn lại đã quá [`STALE_KEY`] là của lượt sẽ không bao giờ chạy.
    fn drop_stale(&self) -> usize {
        let mut keys = self.lock();
        let before = keys.len();
        keys.retain(|_, p| p.stored_at.elapsed() < STALE_KEY);
        before - keys.len()
    }
}

/// Email này có được chép lời không (`TRANSCRIBE_EMAILS`; rỗng = tắt tính năng).
pub async fn allowed(state: &AppState, user_id: &str) -> Result<bool, AppError> {
    let email = user::find_email_by_id(&state.db, user_id).await?.unwrap_or_default();
    Ok(state.config.transcribe_allowed(&email))
}

// MARK: API

#[derive(Deserialize)]
pub struct CreateReq {
    #[serde(rename = "fileIds")]
    file_ids: Vec<String>,
    #[serde(rename = "apiKey")]
    api_key: String,
}

/// `POST /api/transcripts` — xếp hàng chép các bản ghi của chính người dùng.
pub async fn create(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Json(req): Json<CreateReq>,
) -> Result<Json<Value>, AppError> {
    if !allowed(&state, &user.0).await? {
        return Err(AppError::Forbidden("tài khoản này chưa được dùng chép lời".into()));
    }
    let key = req.api_key.trim();
    if !key.starts_with("sk-") || key.len() > 512 || key.chars().any(char::is_whitespace) {
        return Err(AppError::BadRequest("API key OpenAI không đúng dạng (bắt đầu bằng sk-)".into()));
    }
    if req.file_ids.is_empty() || req.file_ids.len() > MAX_FILES {
        return Err(AppError::BadRequest(format!("chọn từ 1 đến {MAX_FILES} bản ghi")));
    }
    // Kiểm hết trước khi xếp hàng: một id sai thì không xếp file nào.
    let files = parse_ids(&req.file_ids.join(","))?;
    let owned = file::list_owned_by_ids(&state.db, &user.0, &files).await?;
    if owned.len() != files.len() {
        return Err(AppError::NotFound("bản ghi không tồn tại".into()));
    }
    if owned.iter().any(|f| f.kind != "recording") {
        return Err(AppError::BadRequest("chỉ chép lời được bản ghi".into()));
    }
    // Chép lại đoạn đã có kết quả: câu sửa của lượt cũ hết hiệu lực (gắn `runId`).
    let rerun = transcript::statuses_for_files(&state.db, &user.0, &files).await?.len();
    let key = Arc::new(ApiKey(key.to_string()));
    for &oid in &files {
        let run_id = uuid::Uuid::new_v4().to_string();
        // Key vào RAM TRƯỚC khi lượt hiện trong hàng đợi: worker có thể nhận lượt ngay sau `enqueue`.
        state.transcribe_keys.put(oid, &run_id, &key);
        if let Err(e) = transcript::enqueue(&state.db, oid, &user.0, &run_id, openai::MODEL).await {
            state.transcribe_keys.forget(oid, &run_id);
            return Err(e);
        }
    }
    state.transcribe_keys.wake.notify_one();
    audit::record(&state, Some(&user.0), "transcript.create", doc! { "files": files.len() as i64 });
    if rerun > 0 {
        audit::record(&state, Some(&user.0), "transcript.rerun", doc! { "files": rerun as i64 });
    }
    Ok(Json(json!({ "queued": files.len() })))
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
                "status": t.status,
                "progress": t.progress,
                "error": t.error,
                "sentences": t.sentences,
            })
        })
        .collect();
    Ok(Json(json!({ "items": items })))
}

/// `GET /api/transcripts/export?ids=a,b&format=txt|srt&name=…` — một tệp ghép các file theo thứ tự,
/// mốc của file sau cộng dồn thời lượng các file trước. File chưa chép xong thì bỏ qua (mốc vẫn cộng).
pub async fn export(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Query(q): Query<ExportQuery>,
) -> Result<Response, AppError> {
    let files = export_files(&state, &user.0, &q).await?;
    let ids: Vec<ObjectId> = files.iter().filter_map(|f| f.id).collect();
    let srt = match q.format.as_deref().unwrap_or("txt") {
        "txt" => false,
        "srt" => true,
        _ => return Err(AppError::BadRequest("định dạng chỉ nhận txt hoặc srt".into())),
    };
    let docs = transcript::list_owned(&state.db, &user.0, &ids).await?;
    let by_id: HashMap<ObjectId, &transcript::TranscriptDoc> = docs.iter().map(|t| (t.id, t)).collect();
    // Câu sửa theo phiên của từng file (một truy vấn), áp theo luật `runId` như màn hình đọc.
    let keys: Vec<String> = files.iter().filter_map(|f| Some(file::session_key(&f.name, f.id?))).collect();
    let edits: HashMap<String, transcript_doc::SessionDoc> = transcript_doc::get_many(&state.db, &user.0, &keys)
        .await?
        .into_iter()
        .map(|d| (d.key.clone(), d))
        .collect();

    let mut segments: Vec<(f64, Vec<Sentence>)> = Vec::new();
    let mut offset = 0.0;
    for f in &files {
        let Some(id) = f.id else { continue };
        let done = by_id.get(&id).filter(|t| t.status == "done");
        let sentences = match done {
            Some(t) => {
                let edit = edits.get(&file::session_key(&f.name, id)).and_then(|d| d.edits.get(&id.to_hex()));
                docs::effective_sentences(t, edit).0
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
    let (body, ext, mime) = if srt {
        (export::to_srt(&parts), "srt", "application/x-subrip; charset=utf-8")
    } else {
        (export::to_txt(&parts), "txt", "text/plain; charset=utf-8")
    };
    let base = sanitize_filename(q.name.as_deref().unwrap_or("chep-loi"));
    Ok((
        [
            (header::CONTENT_TYPE, header::HeaderValue::from_static(mime)),
            (header::CONTENT_DISPOSITION, content_disposition_attachment(&format!("{base}.{ext}"))),
        ],
        body,
    )
        .into_response())
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

/// Khởi động: các lượt dở của tiến trình trước đã mất key → báo lỗi; rồi chạy hàng đợi nền.
pub async fn start_worker(state: AppState) {
    match transcript::fail_unfinished(&state.db).await {
        Ok(n) if n > 0 => tracing::info!("{n} lượt chép lời dở bị huỷ vì khởi động lại"),
        Ok(_) => {}
        Err(e) => tracing::warn!("đánh dấu chép lời dở lỗi: {e:?}"),
    }
    tokio::spawn(async move {
        loop {
            match transcript::claim_next(&state.db).await {
                Ok(Some(t)) => {
                    let (file_id, run_id) = (t.id, t.run_id.clone());
                    let st = state.clone();
                    // Panic trong một lượt không được giết vòng hàng đợi.
                    if let Err(e) = tokio::spawn(async move { run(&st, &t).await }).await {
                        tracing::error!(file = %file_id, "chép lời panic: {e}");
                        record(&state.db, file_id, &run_id, Outcome::Failed("lỗi nội bộ khi chép")).await;
                    }
                }
                Ok(None) => {
                    let dropped = state.transcribe_keys.drop_stale();
                    if dropped > 0 {
                        tracing::info!("bỏ {dropped} key của lượt chép lời không còn trong hàng đợi");
                    }
                    let _ = tokio::time::timeout(Duration::from_secs(5), state.transcribe_keys.wake.notified()).await;
                }
                Err(e) => {
                    tracing::error!("lấy lượt chép lời lỗi: {e}");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    });
}

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

async fn run(state: &AppState, t: &transcript::TranscriptDoc) {
    let started = Instant::now();
    let result = match state.transcribe_keys.take(t.id, &t.run_id) {
        Some(key) => transcribe_file(state, t, &key).await,
        None => Err(transcript::RESTARTED.to_string()),
    };
    let db = &state.db;
    match result {
        Err(e) if e == CANCELLED => {
            tracing::info!(file = %t.file_id(), "lượt chép lời bị thay hoặc file bị xoá, dừng");
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
}

async fn transcribe_file(
    state: &AppState,
    t: &transcript::TranscriptDoc,
    key: &ApiKey,
) -> Result<(String, Vec<Sentence>, usize), String> {
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
    let work = transcribe_path(&state.http, &state.config.ffmpeg_path, &f.path, duration, &key.0, Some(progress));
    tokio::time::timeout(limit, work)
        .await
        .map_err(|_| "chép quá lâu nên đã dừng, hãy thử lại".to_string())?
}

/// Lượt không còn hiệu lực (xếp lại, hoặc file bị xoá): dừng, không ghi gì.
const CANCELLED: &str = "lượt chép lời đã bị thay";

/// Nơi ghi tiến độ của một lượt chép.
struct Progress<'a> {
    db: &'a mongodb::Database,
    file_id: ObjectId,
    run_id: &'a str,
}

/// Chép một tệp audio/video: tìm khoảng lặng, cắt mảnh, gọi OpenAI song song, ghép câu có mốc.
/// Trả (chữ liền, các câu, số mảnh đã gửi).
async fn transcribe_path(
    http: &reqwest::Client,
    ffmpeg: &str,
    path: &str,
    duration: f64,
    api_key: &str,
    progress: Option<Progress<'_>>,
) -> Result<(String, Vec<Sentence>, usize), String> {
    let speech = openai::speech_spans(ffmpeg, path, duration).await?;
    let chunks: Vec<chunking::Span> = chunking::plan_chunks(&speech, duration)
        .into_iter()
        .filter(|&c| chunking::has_speech(c, &speech))
        .collect();

    let total = chunks.len();
    let mut texts = Vec::with_capacity(total);
    // `buffered` giữ thứ tự; lỗi một mảnh thì dừng và bỏ các mảnh đang chờ (khỏi tốn tiền vô ích).
    let mut results = stream::iter(chunks.iter().copied())
        .map(|chunk| async move {
            let audio = openai::encode_chunk(ffmpeg, path, chunk).await?;
            openai::transcribe_chunk(http, api_key, audio, chunk.1 - chunk.0).await
        })
        .buffered(PARALLEL);
    while let Some(text) = results.next().await {
        texts.push(text?);
        if let Some(p) = &progress {
            let pct = (texts.len() as f64 / total as f64 * 100.0).min(99.0).floor();
            // Lượt đã bị thay hoặc file bị xoá: dừng để khỏi tốn tiền của người dùng.
            if let Ok(false) = transcript::set_progress(p.db, p.file_id, p.run_id, pct).await {
                return Err(CANCELLED.into());
            }
        }
    }

    let sentences: Vec<Sentence> = chunks
        .iter()
        .zip(&texts)
        .flat_map(|(&chunk, text)| chunking::sentences(text, chunk, &speech))
        .collect();
    let text = texts.iter().filter(|t| !t.is_empty()).cloned().collect::<Vec<_>>().join(" ");
    Ok((text, sentences, total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_taken_once_and_only_by_their_run() {
        let keys = PendingKeys::default();
        let key = Arc::new(ApiKey("sk-test".into()));
        let (a, b) = (ObjectId::new(), ObjectId::new());
        keys.put(a, "run-1", &key);
        keys.put(b, "run-1", &key);
        // Xếp lại a: key của lượt cũ không còn lấy được.
        keys.put(a, "run-2", &key);
        assert!(keys.take(a, "run-1").is_none());
        assert_eq!(keys.take(a, "run-2").map(|k| k.0.clone()).as_deref(), Some("sk-test"));
        assert!(keys.take(a, "run-2").is_none());
        // Key mới chưa bị coi là bỏ rơi; forget thì mất ngay.
        assert_eq!(keys.drop_stale(), 0);
        keys.forget(b, "run-1");
        assert!(keys.lock().is_empty());
        // Một bản key dùng chung cho cả lượt bấm, không sao chép theo từng file.
        assert_eq!(Arc::strong_count(&key), 1);
    }

    /// Chép thật qua OpenAI (tốn tiền, cần mạng): `OPENAI_API_KEY_FILE=<tệp chứa key>
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
            super::transcribe_path(&reqwest::Client::new(), "ffmpeg", &sample, duration, key.trim(), None)
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
