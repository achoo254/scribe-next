//! Worker abstraction (honor quyết định #8: khung pluggable cho engine tương lai).
//!
//! `trait Worker` = 1 job type → 1 impl. Cắm engine mới (VD transcribe) = thêm 1 `impl Worker`,
//! không đụng queue/API. Dispatch tĩnh theo `job.type` trong `jobs::run_job`.

pub mod split_worker;

use crate::models::job::Job;
use crate::state::AppState;

/// 1 đoạn output do worker sinh ra (chưa lưu Mongo — queue loop lưu `files` + gắn jobId).
pub struct OutputSeg {
    pub name: String,
    pub path: String,
    pub size: u64,
}

#[allow(async_fn_in_trait)] // dùng nội bộ, dispatch tĩnh — không cần Send bound của async-trait.
pub trait Worker {
    async fn process(&self, state: &AppState, job: &Job) -> Result<Vec<OutputSeg>, String>;
}

/// Stub cho transcribe (decision #8: khung sẵn, engine cắm sau khi có hạ tầng GPU).
pub struct TranscribeWorker;

impl Worker for TranscribeWorker {
    async fn process(&self, _state: &AppState, _job: &Job) -> Result<Vec<OutputSeg>, String> {
        Err("transcribe chưa được hỗ trợ".into())
    }
}
