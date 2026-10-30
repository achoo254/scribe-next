//! Phần chạy của tool MCP. Mỗi tool gọi đúng handler `/api` mà app dùng (cùng kiểm sở hữu, giới hạn tần suất, hạn
//! mức và quyền chép lời) với user của token, rồi gọn kết quả cho công cụ AI: phân trang, trần kích thước.
//! Riêng danh sách job cắt không có route: đọc qua `job::list_owned`, cùng điều kiện sở hữu với `GET /api/jobs/{id}`.

use super::tools::{LIST_DEFAULT, LIST_MAX, SENTENCES_DEFAULT, SENTENCES_MAX};
use crate::auth::{self, UserId};
use crate::error::AppError;
use crate::models::{file, job};
use crate::search::ask::AskReq;
use crate::state::AppState;
use crate::transcribe::docs;
use crate::{files, jobs, search, transcribe, translate};
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::{Extension, Json};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use mongodb::bson::oid::ObjectId;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

/// Trần một trang (ký tự JSON) để kết quả vừa ngữ cảnh của model; trang luôn có ít nhất một mục.
const PAGE_CHARS: usize = 25_000;
/// Trần bản xuất dạng chữ; dài hơn thì cắt và báo.
const EXPORT_CHARS: usize = 100_000;
/// Trần tệp docx trả kèm (base64 trong kết quả).
const DOCX_MAX_BYTES: usize = 4 * 1024 * 1024;
/// Trần body đọc từ handler (bản xuất của bản ghi rất dài).
const HANDLER_MAX_BYTES: usize = 32 * 1024 * 1024;

/// Lỗi tool: model đọc được `code` để đổi hướng, `message` nói bước tiếp theo.
#[derive(Debug)]
pub struct ToolError {
    pub code: String,
    pub message: String,
}

impl ToolError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        ToolError { code: code.to_string(), message: message.into() }
    }

    fn invalid(message: impl Into<String>) -> Self {
        Self::new("invalid_argument", message)
    }

    pub fn internal() -> Self {
        Self::new("internal", "internal error; retry later")
    }

    /// Lỗi HTTP của handler (`{error, code?}`) → lỗi tool; mã riêng của handler (vd `DAILY_LIMIT`) được giữ.
    fn from_http(status: StatusCode, body: &[u8]) -> Self {
        let parsed: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
        let message = parsed["error"].as_str().unwrap_or("request failed").to_string();
        let code = parsed["code"].as_str().unwrap_or_else(|| status_code(status));
        Self::new(code, message)
    }
}

fn status_code(status: StatusCode) -> &'static str {
    match status {
        StatusCode::BAD_REQUEST => "invalid_argument",
        StatusCode::UNAUTHORIZED => "unauthenticated",
        StatusCode::FORBIDDEN => "permission_denied",
        StatusCode::NOT_FOUND => "not_found",
        StatusCode::CONFLICT => "conflict",
        StatusCode::PAYLOAD_TOO_LARGE => "too_large",
        StatusCode::TOO_MANY_REQUESTS => "rate_limited",
        StatusCode::SERVICE_UNAVAILABLE => "unavailable",
        _ => "internal",
    }
}

pub enum Output {
    /// Kết quả có cấu trúc: trả cả `structuredContent` lẫn bản chữ JSON.
    Json(Value),
    /// Nội dung tự dựng (bản xuất: chữ hoặc tệp nhúng).
    Content(Vec<Value>),
}

pub type Outcome = Result<Output, ToolError>;

/// `result` của `tools/call`.
pub fn result(outcome: Outcome) -> Value {
    match outcome {
        Ok(Output::Json(value)) => {
            json!({ "content": [{ "type": "text", "text": value.to_string() }], "structuredContent": value, "isError": false })
        }
        Ok(Output::Content(content)) => json!({ "content": content, "isError": false }),
        Err(e) => {
            let value = json!({ "error": { "code": e.code, "message": e.message } });
            json!({ "content": [{ "type": "text", "text": value.to_string() }], "structuredContent": value, "isError": true })
        }
    }
}

/// Chạy tool `name` (đã có trong danh mục) cho `user`.
pub async fn call(state: &AppState, user: &UserId, name: &str, arguments: Value) -> Outcome {
    let ctx = Ctx { state, user };
    match name {
        "scribe_list_transcripts" => ctx.list_transcripts(args(arguments)?).await,
        "scribe_list_files" => ctx.list_files(args(arguments)?).await,
        "scribe_get_file" => ctx.get_file(args(arguments)?).await,
        "scribe_list_jobs" => ctx.list_jobs(args(arguments)?).await,
        "scribe_get_job" => ctx.get_job(args(arguments)?).await,
        "scribe_get_transcript_status" => ctx.transcript_status(args(arguments)?).await,
        "scribe_get_transcript" => ctx.get_transcript(args(arguments)?).await,
        "scribe_export_transcript" => ctx.export(args(arguments)?).await,
        "scribe_get_summary" => ctx.summary(args(arguments)?).await,
        "scribe_get_translation" => ctx.translation(args(arguments)?).await,
        "scribe_get_speakers" => ctx.speakers(args(arguments)?).await,
        "scribe_get_bookmarks" => ctx.bookmarks(args(arguments)?).await,
        "scribe_search" => ctx.search(args(arguments)?).await,
        "scribe_ask" => ctx.ask(args(arguments)?).await,
        "scribe_get_usage" => ctx.usage(args::<NoArgs>(arguments)?).await,
        _ => Err(ToolError::new("unknown_tool", format!("no tool named {name}"))),
    }
}

// MARK: Tham số

fn args<T: DeserializeOwned>(arguments: Value) -> Result<T, ToolError> {
    let arguments = if arguments.is_null() { json!({}) } else { arguments };
    serde_json::from_value(arguments).map_err(|e| ToolError::invalid(format!("invalid arguments: {e}")))
}

/// Query của handler dựng từ JSON (cùng kiểu mà route đọc từ URL).
fn query<T: DeserializeOwned>(value: Value) -> Result<Query<T>, ToolError> {
    serde_json::from_value(value).map(Query).map_err(|e| ToolError::invalid(format!("invalid arguments: {e}")))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoArgs {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Paging {
    #[serde(default)]
    offset: usize,
    limit: Option<usize>,
}

#[derive(Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Source {
    #[default]
    History,
    Split,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FilesArgs {
    #[serde(default)]
    source: Source,
    #[serde(default)]
    offset: usize,
    limit: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileArgs {
    file_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JobArgs {
    job_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyArgs {
    key: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyPageArgs {
    key: String,
    #[serde(default)]
    offset: usize,
    limit: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TranslationArgs {
    key: String,
    lang: String,
    #[serde(default)]
    offset: usize,
    limit: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportArgs {
    key: String,
    format: Option<String>,
    lang: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchArgs {
    query: String,
    key: Option<String>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AskArgs {
    question: String,
    key: Option<String>,
}

// MARK: Gọi handler

/// Chạy một handler: thành công → header + body; lỗi → lỗi tool theo status và body.
async fn run<R: IntoResponse>(result: Result<R, AppError>) -> Result<(HeaderMap, Bytes), ToolError> {
    let response = result.into_response();
    let status = response.status();
    let (parts, body) = response.into_parts();
    let bytes = axum::body::to_bytes(body, HANDLER_MAX_BYTES)
        .await
        .map_err(|_| ToolError::new("too_large", "the result is too large to return"))?;
    if status.is_success() {
        Ok((parts.headers, bytes))
    } else {
        Err(ToolError::from_http(status, &bytes))
    }
}

async fn run_json<R: IntoResponse>(result: Result<R, AppError>) -> Result<Value, ToolError> {
    let (_, bytes) = run(result).await?;
    serde_json::from_slice(&bytes).map_err(|_| ToolError::internal())
}

/// Một trang của `items` từ `offset`: tối đa `limit` mục và khoảng [`PAGE_CHARS`] ký tự, kèm số liệu phân trang.
fn paged(field: &str, items: Vec<Value>, offset: usize, limit: usize) -> Value {
    let total = items.len();
    let mut page = Vec::new();
    let mut chars = 0;
    for item in items.into_iter().skip(offset).take(limit) {
        let len = item.to_string().len();
        if !page.is_empty() && chars + len > PAGE_CHARS {
            break;
        }
        chars += len;
        page.push(item);
    }
    let next = offset + page.len();
    let has_more = next < total;
    json!({
        field: page,
        "total": total,
        "offset": offset,
        "hasMore": has_more,
        "nextOffset": if has_more { Some(next) } else { None },
    })
}

fn limit_of(limit: Option<usize>, default: usize, max: usize) -> usize {
    limit.unwrap_or(default).clamp(1, max)
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// Khoá bản chép lời của một file trong lịch sử (`f:<id>` hoặc `s:<phiên>` với đoạn ghi điện thoại).
fn transcript_key(item: &Value) -> Option<String> {
    let id = ObjectId::parse_str(item["id"].as_str()?).ok()?;
    Some(file::session_key(item["kind"].as_str()?, item["name"].as_str()?, id))
}

fn items(mut value: Value) -> Vec<Value> {
    match value["items"].take() {
        Value::Array(items) => items,
        _ => Vec::new(),
    }
}

struct Ctx<'a> {
    state: &'a AppState,
    user: &'a UserId,
}

impl Ctx<'_> {
    fn st(&self) -> State<AppState> {
        State(self.state.clone())
    }

    fn me(&self) -> Extension<UserId> {
        Extension(self.user.clone())
    }

    async fn list_transcripts(&self, a: Paging) -> Outcome {
        let list = run_json(docs::list(self.st(), self.me()).await).await?;
        Ok(Output::Json(paged("items", items(list), a.offset, limit_of(a.limit, LIST_DEFAULT, LIST_MAX))))
    }

    async fn history(&self) -> Result<(Vec<Value>, Value), ToolError> {
        let mut list = run_json(files::list(self.st(), self.me()).await).await?;
        let retention = list["retentionDays"].take();
        let items = items(list)
            .into_iter()
            .map(|mut item| {
                item["transcriptKey"] = json!(transcript_key(&item));
                item
            })
            .collect();
        Ok((items, retention))
    }

    async fn split_files(&self) -> Result<(Vec<Value>, Value), ToolError> {
        let mut list = run_json(files::manage_list(self.st(), self.me()).await).await?;
        let retention = list["retentionDays"].take();
        Ok((items(list), retention))
    }

    async fn list_files(&self, a: FilesArgs) -> Outcome {
        let (items, retention) = match a.source {
            Source::History => self.history().await?,
            Source::Split => self.split_files().await?,
        };
        let mut page = paged("items", items, a.offset, limit_of(a.limit, LIST_DEFAULT, LIST_MAX));
        page["retentionDays"] = retention;
        Ok(Output::Json(page))
    }

    async fn get_file(&self, a: FileArgs) -> Outcome {
        if ObjectId::parse_str(&a.file_id).is_err() {
            return Err(ToolError::invalid("file_id must be a 24-character hex id from scribe_list_files"));
        }
        let find = |items: Vec<Value>| items.into_iter().find(|item| item["id"] == a.file_id.as_str());
        if let Some(mut item) = find(self.history().await?.0) {
            item["source"] = json!("history");
            return Ok(Output::Json(item));
        }
        if let Some(mut item) = find(self.split_files().await?.0) {
            item["source"] = json!("split");
            return Ok(Output::Json(item));
        }
        Err(ToolError::new("not_found", "no file with this id; use scribe_list_files to find valid ids"))
    }

    async fn list_jobs(&self, a: Paging) -> Outcome {
        let limit = limit_of(a.limit, LIST_DEFAULT, LIST_MAX);
        // Một job dư để biết còn trang sau.
        let fetched = job::list_owned(&self.state.db, &self.user.0, a.offset as u64, limit as i64 + 1).await.map_err(|e| {
            tracing::error!("MCP đọc danh sách job lỗi: {e:?}");
            ToolError::internal()
        })?;
        let has_more = fetched.len() > limit;
        let jobs: Vec<Value> = fetched
            .iter()
            .take(limit)
            .map(|j| {
                json!({
                    "jobId": j.hex_id(),
                    "status": j.status,
                    "progress": j.progress,
                    "inputFileId": j.input_file_id,
                    "criterion": j.criterion,
                    "value": j.value,
                    "recordingSplit": j.recording_split,
                    "segments": j.segments,
                    "error": j.error,
                    "createdAt": j.created_at.try_to_rfc3339_string().ok(),
                })
            })
            .collect();
        let next = a.offset + jobs.len();
        Ok(Output::Json(json!({
            "items": jobs,
            "offset": a.offset,
            "hasMore": has_more,
            "nextOffset": if has_more { Some(next) } else { None },
        })))
    }

    async fn get_job(&self, a: JobArgs) -> Outcome {
        let mut status = run_json(jobs::get(self.st(), self.me(), Path(a.job_id.clone())).await).await?;
        status["jobId"] = json!(a.job_id);
        Ok(Output::Json(status))
    }

    async fn transcript_status(&self, a: KeyArgs) -> Outcome {
        let mut status = run_json(docs::status(self.st(), self.me(), Path(a.key.clone())).await).await?;
        status["key"] = json!(a.key);
        Ok(Output::Json(status))
    }

    /// Toàn bộ bản chép lời (`GET /api/transcript-docs/{key}`).
    async fn doc(&self, key: &str) -> Result<Value, ToolError> {
        run_json(docs::get(self.st(), self.me(), Path(key.to_string())).await).await
    }

    async fn get_transcript(&self, a: KeyPageArgs) -> Outcome {
        let doc = self.doc(&a.key).await?;
        let names = &doc["speakers"]["names"];
        let mut sentences = Vec::new();
        let mut segments = Vec::new();
        for seg in doc["segments"].as_array().into_iter().flatten() {
            let offset = seg["offsetS"].as_f64().unwrap_or(0.0);
            for s in seg["sentences"].as_array().into_iter().flatten() {
                let speaker = s["speaker"].as_str().map(|id| names[id].as_str().unwrap_or(id));
                sentences.push(json!({
                    "n": sentences.len(),
                    "fileId": seg["fileId"],
                    "at": round2(offset + s["start"].as_f64().unwrap_or(0.0)),
                    "end": round2(offset + s["end"].as_f64().unwrap_or(0.0)),
                    "speaker": speaker,
                    "text": s["text"],
                }));
            }
            segments.push(json!({
                "fileId": seg["fileId"],
                "index": seg["index"],
                "status": seg["status"],
                "progress": seg["progress"],
                "error": seg["error"],
                "offsetS": seg["offsetS"],
                "durationS": seg["durationS"],
                "edited": seg["edited"],
            }));
        }
        let mut out = paged("sentences", sentences, a.offset, limit_of(a.limit, SENTENCES_DEFAULT, SENTENCES_MAX));
        for field in ["key", "title", "name", "createdAt", "missingIndexes"] {
            out[field] = doc[field].clone();
        }
        out["segments"] = json!(segments);
        out["speakers"] = names.clone();
        Ok(Output::Json(out))
    }

    async fn export(&self, a: ExportArgs) -> Outcome {
        docs::check_key(&a.key).map_err(|_| ToolError::invalid("key is not a valid recording key; use scribe_list_transcripts"))?;
        let target = match (a.key.strip_prefix("s:"), a.key.strip_prefix("f:")) {
            (Some(session), _) => json!({ "session": session }),
            (_, Some(file_id)) => json!({ "ids": file_id }),
            _ => return Err(ToolError::invalid("key must start with f: or s:")),
        };
        let format = a.format.unwrap_or_else(|| "txt".into());
        let mut q = json!({ "format": format, "lang": a.lang });
        if let (Some(q), Some(target)) = (q.as_object_mut(), target.as_object()) {
            q.extend(target.clone());
        }
        let (headers, bytes) = run(transcribe::export(self.st(), self.me(), query(q)?).await).await?;
        let mime = headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("application/octet-stream");
        let file_name: String = a.key.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' }).collect();
        if format == "docx" {
            if bytes.len() > DOCX_MAX_BYTES {
                return Err(ToolError::new("too_large", "the docx is larger than 4 MiB; export as md or txt instead"));
            }
            return Ok(Output::Content(vec![
                json!({ "type": "text", "text": format!("Exported {} as docx ({} bytes).", a.key, bytes.len()) }),
                json!({ "type": "resource", "resource": {
                    "uri": format!("scribe://exports/{file_name}.docx"),
                    "mimeType": mime,
                    "blob": STANDARD.encode(&bytes),
                } }),
            ]));
        }
        let text = String::from_utf8_lossy(&bytes);
        let total = text.chars().count();
        let mut content = vec![json!({ "type": "text", "text": text.chars().take(EXPORT_CHARS).collect::<String>() })];
        if total > EXPORT_CHARS {
            content.push(json!({ "type": "text", "text": format!(
                "[Cut at {EXPORT_CHARS} of {total} characters. Read the rest with scribe_get_transcript.]"
            ) }));
        }
        Ok(Output::Content(content))
    }

    async fn summary(&self, a: KeyArgs) -> Outcome {
        let doc = self.doc(&a.key).await?;
        Ok(Output::Json(json!({ "key": a.key, "title": doc["title"], "summary": doc["summary"] })))
    }

    async fn speakers(&self, a: KeyArgs) -> Outcome {
        let doc = self.doc(&a.key).await?;
        let speakers: Vec<Value> = doc["speakers"]["names"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(id, name)| json!({ "id": id, "name": name }))
            .collect();
        Ok(Output::Json(json!({ "key": a.key, "speakers": speakers })))
    }

    async fn bookmarks(&self, a: KeyArgs) -> Outcome {
        let doc = self.doc(&a.key).await?;
        Ok(Output::Json(json!({ "key": a.key, "bookmarks": doc["bookmarks"] })))
    }

    async fn translation(&self, a: TranslationArgs) -> Outcome {
        let lang = query(json!({ "lang": a.lang }))?;
        let t = run_json(translate::get(self.st(), self.me(), Path(a.key.clone()), lang).await).await?;
        let mut sentences = Vec::new();
        for seg in t["segments"].as_array().into_iter().flatten() {
            for (idx, s) in seg["sentences"].as_array().into_iter().flatten().enumerate() {
                sentences.push(json!({
                    "n": sentences.len(),
                    "fileId": seg["fileId"],
                    "idx": idx,
                    "text": s["text"],
                    "outdated": s["outdated"].as_bool().unwrap_or(false),
                }));
            }
        }
        let mut out = paged("sentences", sentences, a.offset, limit_of(a.limit, SENTENCES_DEFAULT, SENTENCES_MAX));
        out["key"] = json!(a.key);
        for field in ["lang", "status", "error", "model", "updatedAt"] {
            out[field] = t[field].clone();
        }
        out["progress"] = json!({ "done": t["done"], "total": t["total"] });
        out["outdatedSentences"] = t["outdated"].clone();
        out["missingSentences"] = t["missing"].clone();
        Ok(Output::Json(out))
    }

    async fn search(&self, a: SearchArgs) -> Outcome {
        let q = query(json!({ "q": a.query, "key": a.key, "limit": a.limit }))?;
        Ok(Output::Json(run_json(search::search(self.st(), self.me(), q).await).await?))
    }

    async fn ask(&self, a: AskArgs) -> Outcome {
        let req: AskReq = args(json!({ "question": a.question, "key": a.key }))?;
        Ok(Output::Json(run_json(search::ask::ask(self.st(), self.me(), Json(req)).await).await?))
    }

    async fn usage(&self, _: NoArgs) -> Outcome {
        let me = run_json(auth::me(self.st(), self.me()).await).await?;
        let storage = run_json(files::usage(self.st(), self.me()).await).await?;
        Ok(Output::Json(json!({
            "plan": me["plan"],
            "canTranscribe": me["canTranscribe"],
            "transcription": me["transcription"],
            "storage": storage,
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_stop_at_the_limit_or_the_size_cap_but_hold_at_least_one_item() {
        let items: Vec<Value> = (0..10).map(|i| json!({ "i": i })).collect();
        let page = paged("items", items.clone(), 2, 3);
        assert_eq!(page["items"], json!([{ "i": 2 }, { "i": 3 }, { "i": 4 }]));
        assert_eq!((page["total"].clone(), page["hasMore"].clone(), page["nextOffset"].clone()), (json!(10), json!(true), json!(5)));
        let last = paged("items", items, 8, 50);
        assert_eq!((last["hasMore"].clone(), last["nextOffset"].clone()), (json!(false), Value::Null));

        let big = "x".repeat(PAGE_CHARS);
        let huge = paged("items", vec![json!(big), json!(big)], 0, 10);
        assert_eq!(huge["items"].as_array().map(Vec::len), Some(1), "một mục quá trần vẫn trả, mục sau sang trang sau");
        assert_eq!(huge["nextOffset"], json!(1));
    }

    #[test]
    fn handler_errors_keep_their_own_code_or_follow_the_status() {
        let quota = ToolError::from_http(StatusCode::FORBIDDEN, br#"{"error":"het luot","code":"DAILY_LIMIT"}"#);
        assert_eq!((quota.code.as_str(), quota.message.as_str()), ("DAILY_LIMIT", "het luot"));
        let missing = ToolError::from_http(StatusCode::NOT_FOUND, br#"{"error":"khong co"}"#);
        assert_eq!(missing.code, "not_found");
        assert_eq!(ToolError::from_http(StatusCode::INTERNAL_SERVER_ERROR, b"").code, "internal");
    }

    #[test]
    fn history_files_carry_their_transcript_key() {
        let id = "65f0a1b2c3d4e5f60718293a";
        let upload = json!({ "id": id, "kind": "upload", "name": "hop.m4a" });
        assert_eq!(transcript_key(&upload), Some(format!("f:{id}")));
        let segment = json!({ "id": id, "kind": "recording", "name": "aaaaaaaa-1111#3" });
        assert_eq!(transcript_key(&segment).as_deref(), Some("s:aaaaaaaa-1111"));
        assert_eq!(transcript_key(&json!({ "id": null })), None);
    }

    #[test]
    fn tool_results_mark_errors_and_keep_structured_content() {
        let ok = result(Ok(Output::Json(json!({ "a": 1 }))));
        assert_eq!((ok["isError"].clone(), ok["structuredContent"]["a"].clone()), (json!(false), json!(1)));
        let err = result(Err(ToolError::new("not_found", "no")));
        assert_eq!(err["isError"], true);
        assert_eq!(err["structuredContent"]["error"]["code"], "not_found");
    }
}
