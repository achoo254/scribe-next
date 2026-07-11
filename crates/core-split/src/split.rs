//! Cắt 1 file → nhiều đoạn (ffmpeg segment muxer + `-c copy`). Tiến độ qua callback.

use crate::probe::{probe_file, size_cut_times};
use crate::{compute_segment_time, new_command, stem_of, MediaInfo, OutFile, SplitProgress};
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

/// Tiêu chí cắt. Serde `lowercase` khớp giá trị FE/desktop cũ ("minutes"/"megabytes"/"parts").
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SplitCriterion {
    Minutes,
    Megabytes,
    Parts,
}

/// Cách cắt cho ffmpeg segment muxer.
enum SegmentArg {
    /// Cắt đều theo thời gian (Phút / Chia-K) → `-segment_time`.
    Time(f64),
    /// Cắt tại các mốc thời gian cụ thể (Cắt theo MB) → `-segment_times`.
    Times(Vec<f64>),
}

/// Cắt 1 file vào `out_dir/<stem>/`. Trả danh sách đoạn (đã sort theo tên).
///
/// `ffmpeg_path`/`ffprobe_path`: đường dẫn tường minh (server đọc từ config; desktop resolve sidecar).
pub async fn split_file(
    ffmpeg_path: &str,
    ffprobe_path: &str,
    input: &str,
    criterion: SplitCriterion,
    value: f64,
    out_dir: &str,
    mut on_progress: impl FnMut(SplitProgress),
) -> Result<Vec<OutFile>, String> {
    let info = probe_file(ffprobe_path, input).await?;
    // MB: cắt theo byte thực tế tại keyframe (đều hơn cho VBR). Không lấy được keyframe/pos hoặc
    // file nhỏ hơn 1 đoạn → fallback ước lượng theo bitrate trung bình.
    let seg = if criterion == SplitCriterion::Megabytes {
        let target_bytes = value * 1_000_000.0;
        let times = if target_bytes >= 1.0 {
            size_cut_times(ffprobe_path, input, target_bytes).await
        } else {
            Vec::new()
        };
        if times.is_empty() {
            SegmentArg::Time(compute_segment_time(criterion, value, &info)?)
        } else {
            SegmentArg::Times(times)
        }
    } else {
        SegmentArg::Time(compute_segment_time(criterion, value, &info)?)
    };

    let sub = stem_of(input);
    let subdir = Path::new(out_dir).join(&sub);
    std::fs::create_dir_all(&subdir).map_err(|e| e.to_string())?;
    cleanup_segments(&subdir, &sub, &info.ext); // xoá đoạn cũ cùng pattern (tránh lẫn lần trước)
    let pattern = subdir
        .join(format!("{sub}_%03d.{}", info.ext))
        .to_string_lossy()
        .to_string();

    let mut args: Vec<String> = vec![
        "-v".into(), "error".into(),
        "-progress".into(), "pipe:1".into(),
        "-i".into(), input.into(),
        "-f".into(), "segment".into(),
    ];
    match &seg {
        SegmentArg::Time(t) => {
            args.push("-segment_time".into());
            args.push(format!("{t}"));
        }
        SegmentArg::Times(ts) => {
            args.push("-segment_times".into());
            args.push(ts.iter().map(|t| format!("{t}")).collect::<Vec<_>>().join(","));
        }
    }
    args.extend(["-reset_timestamps", "1", "-c", "copy"].iter().map(|s| s.to_string()));
    // HEVC trong MP4/MOV: ép tag hvc1 — mặc định ffmpeg ghi hev1 khiến nhiều player Windows chỉ phát
    // tiếng, mất hình. Chỉ đổi nhãn, vẫn `-c copy` → không re-encode.
    let is_hevc = matches!(info.vcodec.as_str(), "hevc" | "h265");
    if is_hevc && matches!(info.ext.to_ascii_lowercase().as_str(), "mp4" | "mov" | "m4v") {
        args.push("-tag:v".into());
        args.push("hvc1".into());
    }
    args.push(pattern);

    run_ffmpeg(ffmpeg_path, &args, &info, &mut on_progress).await?;

    let outputs = list_outputs(&subdir, &sub, &info.ext);
    if outputs.is_empty() {
        return Err("ffmpeg không tạo được đoạn nào".to_string());
    }
    Ok(outputs)
}

/// Spawn ffmpeg, đọc `-progress pipe:1` (stdout) báo tiến độ, gom stderr để báo lỗi rõ.
async fn run_ffmpeg(
    ffmpeg_path: &str,
    args: &[String],
    info: &MediaInfo,
    on_progress: &mut impl FnMut(SplitProgress),
) -> Result<(), String> {
    let mut child = new_command(ffmpeg_path)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("không chạy được ffmpeg ({ffmpeg_path}): {e}"))?;

    let stdout = child.stdout.take().ok_or("không mở được stdout ffmpeg")?;
    let mut stderr = child.stderr.take().ok_or("không mở được stderr ffmpeg")?;
    // Đọc stderr song song (task riêng) tránh deadlock khi pipe đầy.
    let stderr_task = tokio::spawn(async move {
        let mut buf = String::new();
        let _ = stderr.read_to_string(&mut buf).await;
        buf
    });

    let dur = info.duration_s.max(0.001);
    let mut lines = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if let Some(us) = line
            .trim()
            .strip_prefix("out_time_us=")
            .and_then(|v| v.trim().parse::<f64>().ok())
        {
            let file_pct = ((us / 1e6) / dur * 100.0).clamp(0.0, 100.0);
            on_progress(SplitProgress { file_pct });
        }
    }

    let status = child.wait().await.map_err(|e| e.to_string())?;
    let stderr_buf = stderr_task.await.unwrap_or_default();
    if !status.success() {
        let msg = stderr_buf.trim();
        return Err(if msg.is_empty() {
            "ffmpeg thất bại (không rõ lỗi)".to_string()
        } else {
            format!("ffmpeg: {msg}")
        });
    }
    Ok(())
}

/// Khớp ĐÚNG `<sub>_<digits>.<ext>` (không match nhầm stem khác cùng tiền tố).
fn is_segment(name: &str, sub: &str, ext: &str) -> bool {
    let Some(rest) = name.strip_prefix(&format!("{sub}_")) else {
        return false;
    };
    let Some(num) = rest.strip_suffix(&format!(".{ext}")) else {
        return false;
    };
    !num.is_empty() && num.chars().all(|c| c.is_ascii_digit())
}

/// Xoá các đoạn cũ khớp pattern trong subdir trước khi cắt lại (tránh lẫn kết quả cũ).
fn cleanup_segments(dir: &Path, sub: &str, ext: &str) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            if is_segment(&e.file_name().to_string_lossy(), sub, ext) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

fn list_outputs(dir: &Path, sub: &str, ext: &str) -> Vec<OutFile> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if is_segment(&name, sub, ext) {
                out.push(OutFile {
                    name,
                    path: e.path().to_string_lossy().to_string(),
                });
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[cfg(test)]
mod tests {
    use super::is_segment;

    #[test]
    fn segment_name_matching() {
        assert!(is_segment("song_000.mp3", "song", "mp3"));
        assert!(is_segment("song_012.mp3", "song", "mp3"));
        assert!(!is_segment("song.mp3", "song", "mp3")); // không có _digits
        assert!(!is_segment("song_.mp3", "song", "mp3")); // digits rỗng
        assert!(!is_segment("song_abc.mp3", "song", "mp3")); // không phải digits
        assert!(!is_segment("song2_000.mp3", "song", "mp3")); // stem khác cùng tiền tố
        assert!(!is_segment("song_000.wav", "song", "mp3")); // sai ext
    }
}
