// Mode B — cắt file sẵn có theo phút / MB / chia-K + batch. Thuật toán cắt (ffmpeg `-c copy`) uỷ
// thác crate `core-split` (dùng CHUNG với server, một nguồn chân lý). Desktop lo phần Tauri:
// resolve PATH sidecar ffmpeg/ffprobe, điều phối batch + né trùng subdir, phát event tiến độ cho FE.
use crate::ffmpeg::sidecar_path;
use core_split::{split_file_into, SplitCriterion};
use std::collections::HashSet;
use std::path::Path;
use tauri::{AppHandle, Emitter};

// MediaInfo lấy thẳng từ core-split — shape khớp payload FE cũ (duration_s/bitrate_bps/codec/vcodec/
// ext) nên frontend không đổi. Re-export để command trong commands.rs không đổi. (OutFile của
// core-split cũng khớp {name,path} FE và được emit qua Vec trả về của split_file_into.)
pub use core_split::MediaInfo;

/// Tiến độ cắt bắn cho FE. Giữ nguyên payload cũ (file_index/total/file_name/file_pct/overall_pct);
/// `core-split` chỉ báo `file_pct` mỗi file → wrapper tính `overall_pct` cho cả batch.
#[derive(serde::Serialize, Clone)]
struct SplitProgress {
    file_index: usize,
    total: usize,
    file_name: String,
    file_pct: f64,
    overall_pct: f64,
}

/// ffprobe → metadata cho UI (trước khi cắt). Sidecar resolve từ current_exe (không cần AppHandle).
pub async fn probe_file(path: &str) -> Result<MediaInfo, String> {
    let ffprobe = sidecar_path("ffprobe")?;
    core_split::probe_file(&ffprobe, path).await
}

/// Batch tuần tự. Lỗi 1 file → emit split_error + tiếp tục file kế (đúng hành vi cũ).
pub async fn split_files(
    app: AppHandle,
    paths: Vec<String>,
    criterion: String,
    value: f64,
    out_dir: String,
) -> Result<(), String> {
    let total = paths.len();
    let ffmpeg = sidecar_path("ffmpeg")?;
    let ffprobe = sidecar_path("ffprobe")?;
    let crit = parse_criterion(&criterion);
    let mut used: HashSet<String> = HashSet::new();
    for (i, path) in paths.iter().enumerate() {
        // subdir duy nhất — 2 file trùng tên (khác thư mục nguồn) không ghi đè nhau.
        let stem = stem_of(path);
        let mut sub = stem.clone();
        let mut n = 2;
        while !used.insert(sub.clone()) {
            sub = format!("{stem}_{n}");
            n += 1;
        }
        let result = match crit {
            Some(c) => {
                let app_cb = app.clone();
                let name = sub.clone();
                split_file_into(&ffmpeg, &ffprobe, path, c, value, &out_dir, &sub, |p| {
                    let overall_pct = (i as f64 * 100.0 + p.file_pct) / total as f64;
                    let _ = app_cb.emit(
                        "split_progress",
                        SplitProgress {
                            file_index: i,
                            total,
                            file_name: name.clone(),
                            file_pct: p.file_pct,
                            overall_pct,
                        },
                    );
                })
                .await
            }
            None => Err("Tiêu chí không hợp lệ".to_string()),
        };
        match result {
            Ok(segments) => {
                let _ = app.emit("split_file_done", (path.clone(), segments));
            }
            Err(e) => {
                let _ = app.emit("split_error", (path.clone(), e));
            }
        }
    }
    let _ = app.emit("split_all_done", ());
    Ok(())
}

/// Tiêu chí FE ("minutes"/"megabytes"/"parts") → enum core-split. Sai → None (caller emit lỗi/file).
fn parse_criterion(s: &str) -> Option<SplitCriterion> {
    match s {
        "minutes" => Some(SplitCriterion::Minutes),
        "megabytes" => Some(SplitCriterion::Megabytes),
        "parts" => Some(SplitCriterion::Parts),
        _ => None,
    }
}

fn stem_of(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output")
        .to_string()
}
