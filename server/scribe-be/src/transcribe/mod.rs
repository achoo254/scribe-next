//! Chép lời bản ghi bằng OpenAI `gpt-4o-mini-transcribe`, key OpenAI của chủ sản phẩm.
//!
//! Key nằm trong tệp `OPENAI_KEY_PATH` trên server, đọc một lần lúc khởi động ([`Transcriber`]): không
//! có trong repo, binary, log hay audit. Tiền là của chủ sản phẩm nên mỗi lượt giữ chỗ phút trong sổ
//! (`models::usage`) trước khi xếp hàng và quyết toán theo số giây thật sự đã gửi khi kết thúc.
//! Chỉ email trong `TRANSCRIBE_EMAILS` được dùng, trừ khi bật `TRANSCRIBE_OPEN`.
//!
//! Hàng đợi riêng (collection `transcripts`), chạy tuần tự từng file, mỗi file gọi song song
//! [`PARALLEL`] mảnh — không chặn hàng đợi Cắt file. Backend khởi động lại thì lượt dở chạy lại.

pub mod bookmarks;
pub mod chunking;
pub mod docs;
mod export;
pub(crate) mod openai;

use crate::audit;
use crate::auth::UserId;
use crate::error::AppError;
use crate::files::{content_disposition_attachment, sanitize_filename};
use crate::models::usage::{self, Blocked, Charge};
use crate::notify;
use crate::models::{file, plan, transcript, transcript_doc, user};
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
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Notify;

/// Số mảnh gọi OpenAI cùng lúc trong một file.
const PARALLEL: usize = 4;
/// Một lần bấm "Chép lời" tối đa bao nhiêu file (phiên ghi 3 giờ, đoạn 1 phút = 180 file).
const MAX_FILES: usize = 200;

/// Một file chép quá lâu thì dừng, khỏi chặn hàng đợi: 10 phút + thời lượng bản ghi.
const FILE_TIMEOUT_BASE_S: u64 = 600;

/// API key OpenAI của server. Không có `Debug`/`Display` để khỏi lỡ in ra log.
struct ApiKey(String);

/// Thông báo cho người dùng khi key server không dùng được (không lộ chi tiết key/tài khoản).
pub(crate) const PAUSED: &str = "Chép lời đang tạm ngưng, quản trị đã được báo. Thử lại sau.";

/// Key server + hàng đợi. Key hỏng (OpenAI báo 401 hoặc hết credit) thì ngắt: không nhận lượt mới
/// cho tới khi khởi động lại với key mới.
#[derive(Default)]
pub struct Transcriber {
    key: Option<ApiKey>,
    broken: AtomicBool,
    wake: Notify,
}

impl Transcriber {
    /// Đọc key từ tệp. Thiếu, rỗng hay sai dạng thì chép lời tắt (log một dòng, không có giá trị key).
    pub fn load(path: Option<&str>) -> Transcriber {
        let key = path.and_then(|p| match std::fs::read_to_string(p) {
            Ok(raw) => {
                let key = raw.trim().to_string();
                if key.starts_with("sk-") && !key.chars().any(char::is_whitespace) {
                    Some(ApiKey(key))
                } else {
                    tracing::error!(path = %p, "tệp key OpenAI không đúng dạng — chép lời tắt");
                    None
                }
            }
            Err(e) => {
                tracing::error!(path = %p, "không đọc được tệp key OpenAI ({}) — chép lời tắt", e.kind());
                None
            }
        });
        if key.is_some() {
            tracing::info!("chép lời bật (key server)");
        }
        Transcriber { key, broken: AtomicBool::new(false), wake: Notify::new() }
    }

    pub fn ready(&self) -> bool {
        self.key.is_some() && !self.broken.load(Ordering::Relaxed)
    }

    /// Key server khi còn dùng được. Tóm tắt dùng chung key: chép lời bị ngắt thì tóm tắt cũng dừng; lỗi
    /// key ở lượt tóm tắt chỉ ngắt tóm tắt (`summary::Summaries`), không ngắt chép lời.
    pub(crate) fn api_key(&self) -> Option<&str> {
        self.key.as_ref().filter(|_| self.ready()).map(|k| k.0.as_str())
    }

    /// Lần đầu key hỏng thì trả `true` (để ghi audit một lần).
    fn trip(&self) -> bool {
        !self.broken.swap(true, Ordering::Relaxed)
    }
}

// MARK: API

#[derive(Deserialize)]
pub struct CreateReq {
    /// App cũ còn gửi kèm `apiKey`: serde bỏ qua trường lạ.
    #[serde(rename = "fileIds")]
    file_ids: Vec<String>,
}

/// Giây cần giữ chỗ cho một bản ghi: thời lượng lưu sẵn, thiếu hoặc ≤ 0 thì đo lại từ tệp.
async fn billable_seconds(state: &AppState, f: &file::FileDoc) -> Result<i64, AppError> {
    let stored = f.duration_s.filter(|d| *d > 0.0);
    let duration = match stored {
        Some(d) => d,
        None => core_split::probe_file(&state.config.ffprobe_path, &f.path)
            .await
            .ok()
            .map(|i| i.duration_s)
            .filter(|d| *d > 0.0)
            .ok_or_else(|| AppError::BadRequest(format!("không đọc được thời lượng của “{}”", f.name)))?,
    };
    Ok(duration.ceil() as i64)
}

/// Giờ mở lại cho người dùng đọc (giờ Việt Nam).
pub(crate) fn vn_time(at: chrono::DateTime<chrono::Utc>) -> String {
    at.with_timezone(&usage::vietnam()).format("%H:%M ngày %d/%m").to_string()
}

fn blocked_error(blocked: &Blocked, used_s: i64, limit_s: i64, now: chrono::DateTime<chrono::Utc>) -> AppError {
    let (used_minutes, limit_minutes) = (usage::used_minutes(used_s), limit_s / 60);
    let (code, message, retry_at) = match blocked {
        Blocked::UserQuota => (
            "QUOTA_EXCEEDED",
            format!("Không đủ phút chép lời: tháng này đã dùng {used_minutes}/{limit_minutes} phút."),
            usage::next_reset(now),
        ),
        Blocked::DailyLimit => (
            "DAILY_LIMIT",
            format!("Gói miễn phí chép được {limit_minutes} phút mỗi ngày, hôm nay đã dùng {used_minutes} phút. Mở lại lúc 0 giờ."),
            usage::next_day(now),
        ),
        Blocked::FreePool | Blocked::TotalCap => (
            "CAP_REACHED",
            "Chép lời đã dùng hết hạn mức chung của tháng này, sẽ mở lại đầu tháng sau.".to_string(),
            usage::next_reset(now),
        ),
    };
    AppError::Quota { code, message, used_minutes, limit_minutes, retry_at: Some(retry_at.to_rfc3339()) }
}

/// Gói free: tài khoản mới phải chờ `free_delay_hours` từ lúc tạo. User không có `createdAt` là user cũ.
fn too_new(profile: &user::Profile, delay_hours: i64, now: chrono::DateTime<chrono::Utc>) -> Option<AppError> {
    let created = chrono::DateTime::from_timestamp_millis(profile.created_at?.timestamp_millis())?;
    let opens = created + chrono::Duration::hours(delay_hours);
    (now < opens).then(|| AppError::Quota {
        code: "ACCOUNT_TOO_NEW",
        message: format!("Tài khoản mới dùng được chép lời miễn phí từ {}.", vn_time(opens)),
        used_minutes: 0,
        limit_minutes: 0,
        retry_at: Some(opens.to_rfc3339()),
    })
}

/// Một tài khoản chạm giới hạn ngày lần thứ 3 trong ngày: báo chủ sản phẩm (một lần mỗi ngày).
async fn note_repeated_block(state: &AppState, charge: &Charge, blocked: &Blocked, email: &str) {
    let (Blocked::DailyLimit, Some(day)) = (blocked, &charge.day) else { return };
    if usage::count_hit(&state.db, &format!("daily:{}:{day}", charge.user_id)).await == 3 {
        notify::send(state, format!("Tài khoản {email} chạm giới hạn chép lời miễn phí trong ngày lần thứ 3 ({day})."));
    }
}

/// `POST /api/transcripts` — giữ chỗ phút rồi xếp hàng chép các bản ghi của chính người dùng.
pub async fn create(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Json(req): Json<CreateReq>,
) -> Result<Json<Value>, AppError> {
    let profile = user::find_profile(&state.db, &user.0)
        .await?
        .ok_or_else(|| AppError::Unauthorized("user không tồn tại".into()))?;
    if profile.blocked {
        return Err(AppError::Forbidden("tài khoản đã bị khoá".into()));
    }
    if !state.config.transcribe_allowed(&profile.email) {
        return Err(AppError::Forbidden("tài khoản này chưa được dùng chép lời".into()));
    }
    if !state.transcriber.ready() {
        return Err(AppError::Unavailable(PAUSED.into()));
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
    if owned.iter().any(|f| !file::is_history_kind(&f.kind)) {
        return Err(AppError::BadRequest("chỉ chép lời được bản ghi hoặc tệp tải lên".into()));
    }
    let mut needs = HashMap::new();
    for f in &owned {
        if let Some(id) = f.id {
            needs.insert(id, billable_seconds(&state, f).await?);
        }
    }
    let total: i64 = needs.values().sum();
    // Đọc trước khi giữ chỗ: sau khi giữ, mọi đường lỗi phải hoàn được phần đã giữ.
    let rerun = transcript::statuses_for_files(&state.db, &user.0, &files).await?.len();

    let now = chrono::Utc::now();
    let plan = plan::effective(profile.plan.as_deref(), profile.plan_until, mongodb::bson::DateTime::now());
    let limits = state.config.transcribe_limits;
    if plan == plan::Plan::Free {
        if let Some(e) = too_new(&profile, limits.free_delay_hours, now) {
            audit::record(&state, Some(&user.0), "transcript.blocked", doc! { "reason": "AccountTooNew" });
            return Err(e);
        }
    }
    if plan == plan::Plan::Free && total > limits.free_daily_seconds {
        let max = limits.free_daily_seconds / 60;
        return Err(AppError::Quota {
            code: "TOO_LONG_FOR_FREE",
            message: format!(
                "Gói miễn phí chép tối đa {max} phút mỗi ngày, các bản ghi đã chọn dài {} phút. Chọn ít bản ghi hơn.",
                usage::used_minutes(total)
            ),
            used_minutes: usage::used_minutes(total),
            limit_minutes: max,
            retry_at: None,
        });
    }
    let charge = Charge::new(&user.0, plan, now);
    let user_limit = plan.limit_seconds(&limits);
    if let Err(blocked) = usage::reserve(&state.db, &charge, user_limit, total, &limits).await? {
        let used_s = usage::blocked_usage(&state.db, &charge, &blocked).await?;
        audit::record(&state, Some(&user.0), "transcript.blocked", doc! { "reason": format!("{blocked:?}") });
        note_repeated_block(&state, &charge, &blocked, &profile.email).await;
        return Err(blocked_error(&blocked, used_s, usage::limit_of(&blocked, user_limit, &limits), now));
    }
    note_cap_levels(&state, &charge.month).await;

    // Từ đây không có `?`: file nào không vào được hàng thì trả lại phần đã giữ cho file đó.
    let mut queued = 0;
    for &oid in &files {
        let need = needs.get(&oid).copied().unwrap_or(0);
        let run_id = uuid::Uuid::new_v4().to_string();
        let ok = match transcript::enqueue(&state.db, oid, &user.0, &run_id, openai::MODEL, &charge, need).await {
            Ok(ok) => ok,
            // Mongo standalone có thể báo lỗi dù đã ghi: đọc lại theo `runId` trước khi hoàn.
            Err(e) => {
                tracing::warn!(file = %oid, "xếp hàng lỗi, kiểm lại trước khi hoàn: {e:?}");
                transcript::has_run(&state.db, oid, &run_id).await.unwrap_or(false)
            }
        };
        if ok {
            queued += 1;
        } else if let Err(e) = usage::settle(&state.db, &charge, -need).await {
            tracing::error!(file = %oid, need, "hoàn phút cho file không xếp được lỗi: {e:?}");
        }
    }
    if queued == 0 {
        return Err(AppError::Conflict("các bản ghi này đang được chép".into()));
    }
    state.transcriber.wake.notify_one();
    let details = doc! { "files": queued as i64, "seconds": total, "plan": plan.name() };
    audit::record(&state, Some(&user.0), "transcript.create", details);
    if rerun > 0 {
        audit::record(&state, Some(&user.0), "transcript.rerun", doc! { "files": rerun as i64 });
    }
    Ok(Json(json!({ "queued": queued, "skipped": files.len() - queued })))
}

/// Quỹ free qua 50/80/100% và trần tổng qua 80/100%: ghi `audit_events` và báo qua bot chat, mỗi mốc một
/// lần mỗi tháng.
pub(crate) async fn note_cap_levels(state: &AppState, month: &str) {
    let Ok(used) = usage::month_usage(&state.db, "-", month).await else { return };
    let limits = state.config.transcribe_limits;
    for (pool, label, used_s, limit_s, levels) in [
        ("free", "Quỹ chép lời miễn phí", used.free_pool_seconds, limits.free_pool_seconds, &[100, 80, 50][..]),
        ("all", "Trần chi phí chép lời", used.total_seconds, limits.monthly_cap_seconds, &[100, 80][..]),
    ] {
        let Some(&level) = levels.iter().find(|&&level| limit_s > 0 && used_s * 100 >= limit_s * level) else {
            continue;
        };
        let action = if level == 100 { "transcript.cap_reached".to_string() } else { format!("transcript.cap_{level}") };
        if usage::mark_once(&state.db, &format!("{action}:{pool}:{month}")).await {
            audit::record(state, None, &action, doc! { "pool": pool, "month": month, "seconds": used_s });
            let (used_m, limit_m) = (usage::used_minutes(used_s), limit_s / 60);
            notify::send(state, format!("{label} tháng {month} đã dùng {level}% ({used_m}/{limit_m} phút)."));
        }
    }
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
    let keys: Vec<String> = files.iter().filter_map(file::FileDoc::session_key).collect();
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
                let edit = edits.get(&file::session_key(&f.kind, &f.name, id)).and_then(|d| d.edits.get(&id.to_hex()));
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

/// Khởi động: lượt bị cắt ngang được xếp lại (lượt đã bị cắt hai lần thì dừng); rồi chạy hàng đợi nền.
pub async fn start_worker(state: AppState) {
    match transcript::requeue_interrupted(&state.db).await {
        Ok((requeued, stopped)) if requeued + stopped > 0 => {
            tracing::info!("chép lời dở sau khởi động lại: {requeued} lượt chạy lại, {stopped} lượt dừng")
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("xếp lại chép lời dở lỗi: {e:?}"),
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
                    let _ = tokio::time::timeout(Duration::from_secs(5), state.transcriber.wake.notified()).await;
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

/// Key server hỏng (401 hoặc hết credit) ở một lượt chép lời: mọi lượt sau cũng hỏng y vậy ⟹ ngắt chép
/// lời (tóm tắt dùng chung key nên dừng theo), ghi audit và báo chủ sản phẩm một lần; người dùng chỉ thấy
/// thông báo tạm ngưng chung.
pub(crate) fn report_key_broken(state: &AppState, e: &str) {
    if state.transcriber.trip() {
        tracing::error!("key OpenAI của server không dùng được ({e}) — ngắt chép lời tới khi khởi động lại");
        audit::record(state, None, "transcript.key_invalid", doc! { "reason": e });
        notify::send(state, format!("Key OpenAI của server không dùng được ({e}); chép lời tạm ngưng tới khi thay key và khởi động lại."));
    }
}

async fn run(state: &AppState, t: &transcript::TranscriptDoc) {
    let started = Instant::now();
    let result = match state.transcriber.key.as_ref().filter(|_| state.transcriber.ready()) {
        Some(key) => transcribe_file(state, t, key).await,
        None => Err(PAUSED.to_string()),
    };
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
        Err(e) if openai::is_key_broken(&e) => {
            report_key_broken(state, &e);
            record(db, t.id, &t.run_id, Outcome::Failed(PAUSED)).await;
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
    key: &ApiKey,
) -> Result<(String, Vec<Sentence>, usize), String> {
    // Khoá sau khi đã xếp hàng: không gửi audio nữa; lượt đóng với 0 giây đã gửi nên được hoàn hết.
    if user::find_profile(&state.db, &t.user_id).await.ok().flatten().is_some_and(|p| p.blocked) {
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
    reserve_shortfall(state, t, duration).await?;
    let progress = Progress { db: &state.db, file_id: t.id, run_id: &t.run_id };
    let limit = Duration::from_secs(FILE_TIMEOUT_BASE_S + duration as u64);
    let work = transcribe_path(&state.http, &state.config.ffmpeg_path, &f.path, duration, &key.0, Some(progress));
    tokio::time::timeout(limit, work)
        .await
        .map_err(|_| "chép quá lâu nên đã dừng, hãy thử lại".to_string())?
}

/// Thời lượng đo lại (cộng phần đã gửi ở lần chạy trước) vượt phần đã giữ: giữ thêm phần chênh trước khi
/// gửi, không giữ được thì dừng. Phần gửi lại vì thử temperature khác và làm tròn từng mảnh không giữ
/// trước (tối đa gấp ba mảnh bị lặp, ~1 giây mỗi mảnh): quyết toán cộng sau, vượt hạn mức có giới hạn.
async fn reserve_shortfall(state: &AppState, t: &transcript::TranscriptDoc, duration: f64) -> Result<(), String> {
    // Chạy lại sau khởi động lại: phần đã gửi ở lần trước đã tính, lần này gửi lại cả file.
    let extra = duration.ceil() as i64 + t.sent_s - t.charged_s;
    let Some(charge) = t.charge().filter(|_| extra > 0) else { return Ok(()) };
    let plan = if t.free_pool { plan::Plan::Free } else { plan::Plan::Plus };
    let limits = state.config.transcribe_limits;
    let fail = |e: AppError| {
        tracing::warn!(file = %t.file_id(), "giữ thêm phút lỗi: {e:?}");
        "không giữ được phút chép lời, thử lại sau".to_string()
    };
    match usage::reserve(&state.db, &charge, plan.limit_seconds(&limits), extra, &limits).await.map_err(fail)? {
        Ok(()) => {
            if !transcript::add_charge(&state.db, t.id, &t.run_id, extra).await.map_err(fail)? {
                usage::settle(&state.db, &charge, -extra).await.map_err(fail)?;
                return Err(CANCELLED.into());
            }
            Ok(())
        }
        Err(_) => Err("bản ghi dài hơn số phút chép lời còn lại".into()),
    }
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

/// Chép một tệp audio/video: tìm khoảng lặng, cắt mảnh, gọi OpenAI song song, ghép câu có mốc. Mảnh
/// hoàn toàn im lặng (đỉnh < `SILENT_PEAK_DB`) không gửi và không tính vào giây đã gửi, nên người dùng
/// được hoàn phần đã giữ cho nó. Trả (chữ liền, các câu, số mảnh đã gửi).
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
    let progress = progress.as_ref();
    // `buffered` giữ thứ tự; lỗi một mảnh thì dừng và bỏ các mảnh đang chờ (khỏi tốn tiền vô ích).
    let mut results = stream::iter(chunks.iter().copied())
        .map(|chunk| async move {
            let (audio, peak) = openai::encode_chunk(ffmpeg, path, chunk).await?;
            if peak.is_some_and(|p| p < chunking::SILENT_PEAK_DB) {
                return Ok::<_, String>((String::new(), false));
            }
            let seconds = (chunk.1 - chunk.0).ceil() as i64;
            // Ghi giây sắp gửi TRƯỚC khi gửi: tiền đã chi luôn nằm trong sổ, kể cả khi tiến trình chết.
            if let Some(p) = progress {
                p.count_sent(seconds).await?;
            }
            let (text, posts) = openai::transcribe_chunk(http, api_key, audio, chunk.1 - chunk.0).await?;
            // Gửi lại với temperature khác cũng bị tính tiền.
            if let Some(p) = progress.filter(|_| posts > 1) {
                p.count_sent(seconds * (i64::from(posts) - 1)).await?;
            }
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
        .zip(&texts)
        .flat_map(|(&chunk, text)| chunking::sentences(text, chunk, &speech))
        .collect();
    let text = texts.iter().filter(|t| !t.is_empty()).cloned().collect::<Vec<_>>().join(" ");
    Ok((text, sentences, sent))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_free_accounts_wait_for_the_delay() {
        let created: chrono::DateTime<chrono::Utc> = "2026-10-16T05:00:00Z".parse().expect("giờ");
        let profile = |created_at: Option<chrono::DateTime<chrono::Utc>>| user::Profile {
            email: "a@x.vn".into(),
            plan: None,
            plan_until: None,
            created_at: created_at.map(|c| mongodb::bson::DateTime::from_millis(c.timestamp_millis())),
            blocked: false,
        };
        let just_before = created + chrono::Duration::hours(24) - chrono::Duration::seconds(1);
        let Some(AppError::Quota { code, retry_at, message, .. }) = too_new(&profile(Some(created)), 24, just_before) else {
            panic!("phải chặn tài khoản mới");
        };
        assert_eq!(code, "ACCOUNT_TOO_NEW");
        assert_eq!(retry_at.as_deref(), Some("2026-10-17T05:00:00+00:00"));
        assert!(message.contains("12:00 ngày 17/10"), "{message}");
        assert!(too_new(&profile(Some(created)), 24, created + chrono::Duration::hours(24)).is_none());
        // User cũ không có `createdAt`, và giới hạn 0 giờ: không chặn.
        assert!(too_new(&profile(None), 24, created).is_none());
        assert!(too_new(&profile(Some(created)), 0, created).is_none());
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
