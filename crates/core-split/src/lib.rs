//! core-split — thuật toán cắt media thuần (ffmpeg segment muxer, `-c copy` lossless).
//!
//! Tách khỏi Tauri: spawn ffmpeg/ffprobe qua `tokio::process` (không dùng tauri shell sidecar),
//! tiến độ báo qua callback (không dùng `app.emit`). Caller truyền tường minh `ffmpeg_path` +
//! `ffprobe_path` (bắt buộc — `probe_file`/`size_cut_times` spawn ffprobe, không phải ffmpeg).
//!
//! Dùng chung: desktop (wrapper Tauri) + server (SplitWorker). Bản extract này phục vụ server;
//! desktop tiếp tục dùng bản `src-tauri/src/splitter.rs` cho tới khi rewire kèm regression test.

mod probe;
mod split;

pub use probe::{probe_file, size_cut_times};
pub use split::{remux_copy_to_mkv, split_file, split_file_into, SplitCriterion};

use std::path::Path;

/// Metadata media đọc từ ffprobe.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct MediaInfo {
    pub duration_s: f64,
    pub bitrate_bps: u64,
    pub codec: String,  // codec audio (hiển thị)
    pub vcodec: String, // codec video — quyết định ép tag hvc1 khi cắt HEVC vào mp4/mov
    pub ext: String,
}

/// Một file đoạn đầu ra.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct OutFile {
    pub name: String,
    pub path: String,
}

/// Tiến độ cắt (báo qua callback thay cho `app.emit`).
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct SplitProgress {
    pub file_pct: f64,
}

/// Quy đổi tiêu chí cắt → `segment_time` (giây). Thuần số học, không I/O → unit-test được.
///
/// - `minutes`: value phút → giây.
/// - `megabytes`: ước lượng theo bitrate trung bình (fallback khi không lấy được keyframe thực tế).
/// - `parts`: chia đều duration cho K đoạn.
pub fn compute_segment_time(
    criterion: SplitCriterion,
    value: f64,
    info: &MediaInfo,
) -> Result<f64, String> {
    if value <= 0.0 {
        return Err("Giá trị phải > 0".into());
    }
    let secs = match criterion {
        SplitCriterion::Minutes => value * 60.0,
        SplitCriterion::Megabytes => {
            if info.bitrate_bps == 0 {
                return Err("Không xác định được bitrate để cắt theo MB".into());
            }
            value * 8_000_000.0 / info.bitrate_bps as f64
        }
        SplitCriterion::Parts => {
            if info.duration_s <= 0.0 {
                return Err("Không xác định được thời lượng để chia".into());
            }
            let k = value.round();
            if k < 1.0 {
                return Err("Số đoạn phải ≥ 1".into());
            }
            info.duration_s / k
        }
    };
    if secs < 1.0 {
        return Err("Mỗi đoạn quá nhỏ (< 1 giây)".into());
    }
    Ok(secs)
}

pub(crate) fn stem_of(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output")
        .to_string()
}

/// Tạo `tokio::process::Command`; trên Windows set `CREATE_NO_WINDOW` để không nháy console
/// (tiền lệ desktop `cuda.rs`). No-op trên Linux/macOS (server chạy Linux).
pub(crate) fn new_command(program: &str) -> tokio::process::Command {
    #[allow(unused_mut)]
    let mut cmd = tokio::process::Command::new(program);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}
