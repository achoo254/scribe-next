//! SplitWorker — cắt file qua crate `core-split` (ffmpeg `-c copy`). Impl `Worker`.
//!
//! Nhánh recording webm (MediaRecorder): webm streaming thiếu seekable cues → segment `-c copy` sai
//! mốc. Remux `-c copy -f matroska` (fix cues) TRƯỚC rồi mới segment (đã CHỐT ở validate). Remux tạo
//! bản sao đầy đủ → check disk-free ≥ 2× input + xoá `fixed.mkv` ngay sau (kể cả khi fail).

use super::{OutputSeg, Worker};
use crate::models::{file, job::Job};
use crate::state::AppState;
use std::path::Path;

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

        // Recording webm → remux fix cues trước khi segment (webm MediaRecorder thiếu cues).
        let is_webm = input.path.to_ascii_lowercase().ends_with(".webm");
        let (split_input, remuxed) = if is_webm {
            let in_size = std::fs::metadata(&input.path).map(|m| m.len()).unwrap_or(0);
            // Remux = bản sao đầy đủ → cần đĩa trống ≥ 2× (fixed.mkv + đoạn output). Best-effort:
            // không xác định được free (non-unix/df lỗi) → bỏ qua, để ffmpeg tự báo nếu hết đĩa.
            if let Some(free) = free_bytes(&out_dir).await {
                if free < in_size.saturating_mul(2) {
                    return Err("không đủ dung lượng đĩa để cắt bản ghi (cần ~2× kích thước)".into());
                }
            }
            let stem = Path::new(&input.path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("rec");
            let fixed = format!("{out_dir}/{stem}.mkv");
            // Remux có thể fail giữa chừng (timeout/hết đĩa/input hỏng) và để lại bản dở → dọn NGAY
            // trong nhánh lỗi (dùng `?` sẽ return trước khi tới bước dọn phía dưới, bỏ sót file).
            if let Err(e) = core_split::remux_copy_to_mkv(&state.config.ffmpeg_path, &input.path, &fixed).await {
                let _ = std::fs::remove_file(&fixed);
                return Err(e);
            }
            (fixed, true)
        } else {
            (input.path.clone(), false)
        };

        // -c copy rất nhanh → tiến độ coarse (queued→running→done); progress mịn để dành re-encode.
        let result = core_split::split_file(
            &state.config.ffmpeg_path,
            &state.config.ffprobe_path,
            &split_input,
            job.criterion,
            job.value,
            &out_dir,
            |_p| {},
        )
        .await;

        // Dọn fixed.mkv NGAY (kể cả khi fail) — không giữ bản sao đầy đủ ăn disk (box shared).
        if remuxed {
            let _ = std::fs::remove_file(&split_input);
        }
        let outs = result?;

        let mut segs = Vec::with_capacity(outs.len());
        for o in outs {
            let size = std::fs::metadata(&o.path).map(|m| m.len()).unwrap_or(0);
            segs.push(OutputSeg { name: o.name, path: o.path, size });
        }
        Ok(segs)
    }
}

/// Dung lượng trống (byte) của filesystem chứa `dir`. None = không xác định được → caller bỏ qua guard.
#[cfg(unix)]
async fn free_bytes(dir: &str) -> Option<u64> {
    let out = tokio::process::Command::new("df")
        .arg("-Pk")
        .arg(dir)
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let avail_kb: u64 = text.lines().nth(1)?.split_whitespace().nth(3)?.parse().ok()?;
    Some(avail_kb.saturating_mul(1024))
}

#[cfg(not(unix))]
async fn free_bytes(_dir: &str) -> Option<u64> {
    None
}
