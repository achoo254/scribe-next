//! Job queue endpoints + worker loop (concurrency=1). Ownership mọi route.

use crate::audit;
use crate::auth::UserId;
use crate::error::AppError;
use crate::models::file::{self, FileDoc};
use crate::models::job::{self, Job, SegRef};
use crate::state::AppState;
use crate::worker::{split_worker::SplitWorker, OutputSeg, Worker};
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::header;
use axum::response::Response;
use axum::{Extension, Json};
use core_split::SplitCriterion;
use mongodb::bson::{doc, oid::ObjectId};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;
use tokio_util::io::ReaderStream;

#[derive(Deserialize)]
pub struct CreateReq {
    #[serde(rename = "inputFileId")]
    input_file_id: String,
    criterion: SplitCriterion,
    value: f64,
    /// true = cắt bản ghi (Mode A) → đoạn vào Lịch sử ghi + xoá gốc. Bỏ trống = false (Mode B zip).
    #[serde(rename = "recordingSplit", default)]
    recording_split: bool,
}
#[derive(Serialize)]
pub struct CreateResp {
    #[serde(rename = "jobId")]
    job_id: String,
}

/// `POST /api/jobs` — tạo split job (verify ownership input file).
pub async fn create(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Json(req): Json<CreateReq>,
) -> Result<Json<CreateResp>, AppError> {
    let input = file::get_owned(&state.db, &req.input_file_id, &user.0)
        .await?
        .ok_or_else(|| AppError::NotFound("input file không tồn tại".into()))?;
    // Cắt-thành-bản-ghi chỉ hợp lệ với file kind:recording (chống lạm dụng: cắt file upload thường
    // thành recording để né phân loại/quota-quản-lý).
    if req.recording_split && input.kind != "recording" {
        return Err(AppError::BadRequest("chỉ cắt-thành-bản-ghi được với bản ghi".into()));
    }
    let criterion = format!("{:?}", req.criterion).to_lowercase();
    let job_id = job::create_split(
        &state.db,
        &user.0,
        &req.input_file_id,
        req.criterion,
        req.value,
        req.recording_split,
    )
    .await?;
    let details = doc! {
        "jobId": &job_id,
        "inputFileId": &req.input_file_id,
        "criterion": criterion,
        "value": req.value,
        "recordingSplit": req.recording_split,
    };
    audit::record(&state, Some(&user.0), "job.create", details);
    Ok(Json(CreateResp { job_id }))
}

/// `GET /api/jobs/:id` — poll trạng thái (ownership).
pub async fn get(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let j = job::get_owned(&state.db, &id, &user.0)
        .await?
        .ok_or_else(|| AppError::NotFound("job không tồn tại".into()))?;
    let mut body = json!({ "status": j.status, "progress": j.progress });
    if let Some(segs) = j.segments {
        body["result"] = json!({ "segments": segs });
    }
    if let Some(e) = j.error {
        body["error"] = json!(e);
    }
    Ok(Json(body))
}

/// `GET /api/jobs/:id/download` — zip streaming các đoạn (ownership).
pub async fn download(
    State(state): State<AppState>,
    Extension(user): Extension<UserId>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let j = job::get_owned(&state.db, &id, &user.0)
        .await?
        .ok_or_else(|| AppError::NotFound("job không tồn tại".into()))?;
    if j.status != "done" {
        return Err(AppError::BadRequest("job chưa hoàn tất".into()));
    }
    let files = file::list_outputs_for_job(&state.db, &id, &user.0).await?;
    if files.is_empty() {
        return Err(AppError::NotFound("không có đoạn để tải".into()));
    }
    let details = doc! { "jobId": &id, "count": files.len() as i64 };
    audit::record(&state, Some(&user.0), "job.download", details);
    let entries: Vec<(String, String)> = files.into_iter().map(|f| (f.name, f.path)).collect();
    let zip_path = format!("{}/{}/outputs/{}.zip", state.config.data_dir, user.0, id);
    let zp = zip_path.clone();
    tokio::task::spawn_blocking(move || build_zip(&zp, &entries))
        .await
        .map_err(|e| AppError::Internal(format!("zip task: {e}")))?
        .map_err(|e| AppError::Internal(format!("tạo zip: {e}")))?;

    let f = tokio::fs::File::open(&zip_path)
        .await
        .map_err(|e| AppError::Internal(format!("mở zip: {e}")))?;
    let body = Body::from_stream(ReaderStream::new(f));
    Response::builder()
        .header(header::CONTENT_TYPE, "application/zip")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"scribe-{id}.zip\""),
        )
        .body(body)
        .map_err(|e| AppError::Internal(format!("build response: {e}")))
}

fn build_zip(zip_path: &str, files: &[(String, String)]) -> std::io::Result<()> {
    use zip::write::{SimpleFileOptions, ZipWriter};
    let out = std::fs::File::create(zip_path)?;
    let mut zw = ZipWriter::new(out);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, path) in files {
        zw.start_file(name.as_str(), opts)?;
        let mut src = std::fs::File::open(path)?;
        std::io::copy(&mut src, &mut zw)?;
    }
    zw.finish()?;
    Ok(())
}

/// Worker queue nền: claim atomic (concurrency=1) → chạy → cập nhật done/failed.
pub fn start_worker(state: AppState) {
    tokio::spawn(async move {
        loop {
            match job::claim_next(&state.db).await {
                Ok(Some(j)) => {
                    let job_id = j.hex_id();
                    let st = state.clone();
                    // Panic-isolation: chạy job trong task con → panic KHÔNG giết worker loop.
                    let handle = tokio::spawn(async move { run_job(&st, &j).await });
                    if let Err(e) = handle.await {
                        tracing::error!(job = %job_id, "job task panic: {e}");
                        let _ = job::finish_failed(&state.db, &job_id, "worker panic").await;
                    }
                }
                Ok(None) => tokio::time::sleep(Duration::from_secs(1)).await,
                Err(e) => {
                    tracing::error!("claim job lỗi: {e}");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    });
}

async fn run_job(state: &AppState, job: &Job) {
    let job_id = job.hex_id();
    let result: Result<Vec<OutputSeg>, String> = match job.job_type.as_str() {
        "split" => SplitWorker.process(state, job).await,
        other => Err(format!("loại job không hỗ trợ: {other}")),
    };
    let segs = match result {
        Ok(segs) => segs,
        Err(e) => {
            tracing::warn!(job = %job_id, "job failed: {e}");
            let _ = job::finish_failed(&state.db, &job_id, &e).await;
            let details = doc! { "jobId": &job_id, "error": &e };
            audit::record(state, Some(&job.user_id), "job.failed", details);
            return;
        }
    };
    let details = doc! {
        "jobId": &job_id,
        "segments": segs.len() as i64,
        "recordingSplit": job.recording_split,
    };
    audit::record(state, Some(&job.user_id), "job.done", details);
    if job.recording_split {
        finish_recording_split(state, job, &job_id, segs).await;
    } else {
        finish_output_split(state, &job_id, &job.user_id, segs).await;
    }
}

/// Mode B (Cắt file): lưu mỗi đoạn thành `kind:output` (tải zip). KHÔNG đụng input file.
async fn finish_output_split(state: &AppState, job_id: &str, user_id: &str, segs: Vec<OutputSeg>) {
    let mut refs = Vec::with_capacity(segs.len());
    for s in segs {
        let doc = FileDoc::new_output(user_id, job_id, &s.name, &s.path, s.size as i64);
        match file::create(&state.db, doc).await {
            Ok(fid) => refs.push(SegRef { name: s.name, file_id: fid }),
            Err(e) => {
                let _ = job::finish_failed(&state.db, job_id, &format!("lưu output: {e:?}")).await;
                return;
            }
        }
    }
    if let Err(e) = job::finish_done(&state.db, job_id, refs).await {
        tracing::error!("finish_done lỗi: {e:?}");
    }
}

/// Mode A (Cắt bản ghi): lưu mỗi đoạn thành `kind:recording` (hiện Lịch sử ghi, phát/tua riêng) rồi
/// xoá bản ghi gốc. Xoá gốc CHỈ sau khi mọi đoạn đã lưu (lỗi giữa chừng → giữ gốc, không mất dữ liệu).
async fn finish_recording_split(state: &AppState, job: &Job, job_id: &str, segs: Vec<OutputSeg>) {
    // Bản ghi gốc: tên hiển thị (đặt tên đoạn) + path/id (xoá sau khi cắt xong).
    let input = match file::get_owned(&state.db, &job.input_file_id, &job.user_id).await {
        Ok(Some(f)) => f,
        Ok(None) => {
            // Gốc đã bị xoá lúc job chạy → đoạn đã ghi ra đĩa nhưng chưa có doc → dọn NGAY để KHÔNG
            // mồ côi file (retention chỉ quét theo doc, không đụng đoạn không doc).
            cleanup_seg_files(&segs);
            let _ = job::finish_failed(&state.db, job_id, "bản ghi gốc không còn").await;
            return;
        }
        Err(e) => {
            cleanup_seg_files(&segs);
            let _ = job::finish_failed(&state.db, job_id, &format!("đọc bản ghi gốc: {e:?}")).await;
            return;
        }
    };
    let base = input.name.clone();

    let mut refs = Vec::with_capacity(segs.len());
    for (i, s) in segs.iter().enumerate() {
        let name = format!("{base} — Phần {:02}", i + 1);
        let doc = FileDoc::new_recording(
            &job.user_id,
            &name,
            &s.path,
            s.size as i64,
            s.duration_s.unwrap_or(0.0),
            s.has_video,
            ObjectId::new(),
        );
        match file::create(&state.db, doc).await {
            Ok(fid) => refs.push(SegRef { name, file_id: fid }),
            Err(e) => {
                // Giữ bản ghi gốc (chưa xoá) → user không mất dữ liệu. Đoạn 0..i đã có doc (kind:recording,
                // có expiresAt → retention tự dọn); đoạn i.. CHƯA có doc → dọn file để không mồ côi.
                cleanup_seg_files(&segs[i..]);
                let _ = job::finish_failed(&state.db, job_id, &format!("lưu đoạn ghi: {e:?}")).await;
                return;
            }
        }
    }

    // Mọi đoạn đã lưu → xoá bản ghi gốc (disk trước; chỉ xoá doc khi disk OK để KHÔNG mồ côi file).
    let disk_ok = match tokio::fs::remove_file(&input.path).await {
        Ok(()) => true,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
        Err(e) => {
            tracing::warn!(job = %job_id, "xoá file gốc lỗi: {e} (giữ doc cho retention sweep)");
            false
        }
    };
    if disk_ok {
        if let Err(e) = file::delete_doc(&state.db, &input).await {
            tracing::warn!(job = %job_id, "xoá doc gốc lỗi: {e:?}");
        }
    }

    if let Err(e) = job::finish_done(&state.db, job_id, refs).await {
        tracing::error!("finish_done lỗi: {e:?}");
    }
}

/// Dọn file đoạn cắt CHƯA có doc (job cắt-bản-ghi thất bại giữa chừng). Retention chỉ quét theo doc
/// `files`; đoạn không doc sẽ mồ côi vĩnh viễn nếu không xoá tay ở đây.
fn cleanup_seg_files(segs: &[OutputSeg]) {
    for s in segs {
        let _ = std::fs::remove_file(&s.path);
    }
}
