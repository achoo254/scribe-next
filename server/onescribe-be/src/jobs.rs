//! Job queue endpoints + worker loop (concurrency=1). Ownership mọi route.

use crate::auth::UserId;
use crate::error::AppError;
use crate::models::file::{self, FileDoc};
use crate::models::job::{self, Job, SegRef};
use crate::state::AppState;
use crate::worker::{split_worker::SplitWorker, OutputSeg, TranscribeWorker, Worker};
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::header;
use axum::response::Response;
use axum::{Extension, Json};
use core_split::SplitCriterion;
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
    file::get_owned(&state.db, &req.input_file_id, &user.0)
        .await?
        .ok_or_else(|| AppError::NotFound("input file không tồn tại".into()))?;
    let job_id =
        job::create_split(&state.db, &user.0, &req.input_file_id, req.criterion, req.value).await?;
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
            format!("attachment; filename=\"onescribe-{id}.zip\""),
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
        _ => TranscribeWorker.process(state, job).await,
    };
    match result {
        Ok(segs) => {
            let mut refs = Vec::with_capacity(segs.len());
            for s in segs {
                let doc = FileDoc::new_output(&job.user_id, &job_id, &s.name, &s.path, s.size as i64);
                match file::create(&state.db, doc).await {
                    Ok(fid) => refs.push(SegRef { name: s.name, file_id: fid }),
                    Err(e) => {
                        let _ = job::finish_failed(&state.db, &job_id, &format!("lưu output: {e:?}")).await;
                        return;
                    }
                }
            }
            if let Err(e) = job::finish_done(&state.db, &job_id, refs).await {
                tracing::error!("finish_done lỗi: {e:?}");
            }
        }
        Err(e) => {
            tracing::warn!(job = %job_id, "job failed: {e}");
            let _ = job::finish_failed(&state.db, &job_id, &e).await;
        }
    }
}
