//! Probe metadata + tính mốc cắt theo byte thực tế (keyframe) cho tiêu chí MB.

use crate::{new_command, MediaInfo};
use std::path::Path;

/// ffprobe → metadata. Bitrate: `format.bit_rate`, fallback ước lượng `size*8/duration` (VBR).
pub async fn probe_file(ffprobe_path: &str, path: &str) -> Result<MediaInfo, String> {
    let out = new_command(ffprobe_path)
        .args([
            "-v", "error", "-print_format", "json",
            "-show_format", "-show_streams", path,
        ])
        .output()
        .await
        .map_err(|e| format!("không chạy được ffprobe ({ffprobe_path}): {e}"))?;
    if !out.status.success() {
        return Err(format!("ffprobe lỗi: {}", String::from_utf8_lossy(&out.stderr)));
    }
    let json: serde_json::Value =
        serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let fmt = &json["format"];
    let duration_s: f64 = fmt["duration"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let mut bitrate_bps: u64 = fmt["bit_rate"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0);
    let size: u64 = fmt["size"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0);
    if bitrate_bps == 0 && duration_s > 0.0 && size > 0 {
        bitrate_bps = ((size as f64 * 8.0) / duration_s) as u64;
    }
    let find_codec = |kind: &str| {
        json["streams"]
            .as_array()
            .and_then(|s| s.iter().find(|x| x["codec_type"] == kind))
            .and_then(|x| x["codec_name"].as_str())
            .unwrap_or("")
            .to_string()
    };
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mp3")
        .to_string();
    Ok(MediaInfo {
        duration_s,
        bitrate_bps,
        codec: find_codec("audio"),
        vcodec: find_codec("video"),
        ext,
    })
}

/// Mốc thời gian cắt cho tiêu chí MB: keyframe video ĐẦU TIÊN có byte-offset (pos) vượt mỗi bội số
/// `target_bytes`. Bám byte THỰC TẾ trong file (không phải bitrate trung bình) → kích thước đoạn đều
/// hơn hẳn với video VBR. Vẫn cắt tại keyframe (`-c copy` lossless) nên đoạn có thể nhỉnh hơn target
/// chút ít. Rỗng (probe lỗi / file nhỏ hơn 1 đoạn) → caller fallback bitrate trung bình.
pub async fn size_cut_times(ffprobe_path: &str, path: &str, target_bytes: f64) -> Vec<f64> {
    let out = new_command(ffprobe_path)
        .args([
            "-v", "error", "-select_streams", "v:0", "-show_entries",
            "packet=pts_time,pos,flags", "-of", "csv=p=0", path,
        ])
        .output()
        .await;
    let out = match out {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut times = Vec::new();
    let mut next_boundary = target_bytes;
    let mut last_t = 0.0_f64;
    for line in text.lines() {
        // csv: pts_time,pos,flags — pts_time có thể "N/A", pos có thể -1 → bỏ qua.
        let mut it = line.split(',');
        let pts = it.next().and_then(|s| s.trim().parse::<f64>().ok());
        let pos = it.next().and_then(|s| s.trim().parse::<f64>().ok());
        let is_key = it.next().map(|f| f.trim_start().starts_with('K')).unwrap_or(false);
        let (Some(pts), Some(pos)) = (pts, pos) else { continue };
        if !is_key || pos <= 0.0 || pts <= 0.0 {
            continue;
        }
        if pos >= next_boundary {
            if pts > last_t {
                times.push(pts);
                last_t = pts;
            }
            // Nhảy mốc lên trên pos hiện tại — keyframe thưa (GOP > target) thì gộp, không tạo đoạn rỗng.
            let crossed = (pos / target_bytes).floor();
            next_boundary = (crossed + 1.0) * target_bytes;
        }
    }
    times
}
