//! Worker abstraction (honor quyết định #8: khung pluggable cho engine tương lai).
//!
//! `trait Worker` = 1 job type → 1 impl. Dispatch tĩnh theo `job.type` trong `jobs::run_job`.
//! Chép lời có hàng đợi riêng (`crate::transcribe`), không đi qua đây.

pub mod split_worker;

use crate::models::job::Job;
use crate::state::AppState;

/// 1 đoạn output do worker sinh ra (chưa lưu Mongo — queue loop lưu `files` + gắn jobId).
pub struct OutputSeg {
    pub name: String,
    pub path: String,
    pub size: u64,
    /// Thời lượng đoạn (giây) — set khi cắt bản ghi (`recording_split`) để lưu `kind:recording`
    /// hiển thị + phát lại. None cho Mode B (đoạn output không cần duration).
    pub duration_s: Option<f64>,
    /// Đoạn có video stream (ghi màn hình) → player dùng `<video>`. false cho audio-only / Mode B.
    pub has_video: bool,
}

#[allow(async_fn_in_trait)] // dùng nội bộ, dispatch tĩnh — không cần Send bound của async-trait.
pub trait Worker {
    async fn process(&self, state: &AppState, job: &Job) -> Result<Vec<OutputSeg>, String>;
}
