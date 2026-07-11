//! SplitWorker — cắt file qua crate `core-split` (ffmpeg `-c copy`). Impl `Worker`.

use super::{OutputSeg, Worker};
use crate::models::{file, job::Job};
use crate::state::AppState;

pub struct SplitWorker;

impl Worker for SplitWorker {
    async fn process(&self, state: &AppState, job: &Job) -> Result<Vec<OutputSeg>, String> {
        // Input file (ownership khớp userId của job).
        let input = file::get_owned(&state.db, &job.input_file_id, &job.user_id)
            .await
            .map_err(|e| format!("{e:?}"))?
            .ok_or("input file không tồn tại")?;

        let out_dir = format!(
            "{}/{}/outputs/{}",
            state.config.data_dir,
            job.user_id,
            job.hex_id()
        );
        std::fs::create_dir_all(&out_dir).map_err(|e| format!("tạo out dir: {e}"))?;

        // -c copy rất nhanh → tiến độ coarse (queued→running→done); progress mịn để dành re-encode Phase 4.
        let outs = core_split::split_file(
            &state.config.ffmpeg_path,
            &state.config.ffprobe_path,
            &input.path,
            job.criterion,
            job.value,
            &out_dir,
            |_p| {},
        )
        .await?;

        let mut segs = Vec::with_capacity(outs.len());
        for o in outs {
            let size = std::fs::metadata(&o.path).map(|m| m.len()).unwrap_or(0);
            segs.push(OutputSeg { name: o.name, path: o.path, size });
        }
        Ok(segs)
    }
}
