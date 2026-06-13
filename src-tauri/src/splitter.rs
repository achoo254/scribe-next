// Mode B — cắt file sẵn có theo phút / MB / chia-K + batch. ffmpeg -c copy (lossless).
// Mọi tiêu chí quy về một segment_time (giây). Output mỗi file gốc vào subfolder <stem>/.
use crate::ffmpeg::{ffmpeg_command, ffprobe_command};
use std::collections::HashSet;
use std::path::Path;
use tauri::{AppHandle, Emitter};
use tauri_plugin_shell::process::CommandEvent;

#[derive(serde::Serialize, Clone)]
pub struct MediaInfo {
    pub duration_s: f64,
    pub bitrate_bps: u64,
    pub codec: String,
    pub ext: String,
}

#[derive(serde::Serialize, Clone)]
pub struct OutFile {
    pub name: String,
    pub path: String,
}

#[derive(serde::Serialize, Clone)]
struct SplitProgress {
    file_index: usize,
    total: usize,
    file_name: String,
    file_pct: f64,
    overall_pct: f64,
}

/// ffprobe → metadata. Bitrate: format.bit_rate, fallback ước lượng size*8/duration (VBR).
pub async fn probe_file(app: &AppHandle, path: &str) -> Result<MediaInfo, String> {
    let out = ffprobe_command(app)?
        .args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams", path])
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("ffprobe lỗi: {}", String::from_utf8_lossy(&out.stderr)));
    }
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let fmt = &json["format"];
    let duration_s: f64 = fmt["duration"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let mut bitrate_bps: u64 = fmt["bit_rate"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0);
    let size: u64 = fmt["size"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0);
    if bitrate_bps == 0 && duration_s > 0.0 && size > 0 {
        bitrate_bps = ((size as f64 * 8.0) / duration_s) as u64;
    }
    let codec = json["streams"]
        .as_array()
        .and_then(|s| s.iter().find(|x| x["codec_type"] == "audio"))
        .and_then(|x| x["codec_name"].as_str())
        .unwrap_or("")
        .to_string();
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mp3")
        .to_string();
    Ok(MediaInfo { duration_s, bitrate_bps, codec, ext })
}

/// Quy đổi tiêu chí → segment_time (giây).
pub fn compute_segment_time(criterion: &str, value: f64, info: &MediaInfo) -> Result<f64, String> {
    if value <= 0.0 {
        return Err("Giá trị phải > 0".into());
    }
    let secs = match criterion {
        "minutes" => value * 60.0,
        "megabytes" => {
            if info.bitrate_bps == 0 {
                return Err("Không xác định được bitrate để cắt theo MB".into());
            }
            value * 8_000_000.0 / info.bitrate_bps as f64
        }
        "parts" => {
            if info.duration_s <= 0.0 {
                return Err("Không xác định được thời lượng để chia".into());
            }
            let k = value.round();
            if k < 1.0 {
                return Err("Số đoạn phải ≥ 1".into());
            }
            info.duration_s / k
        }
        _ => return Err("Tiêu chí không hợp lệ".into()),
    };
    if secs < 1.0 {
        return Err("Mỗi đoạn quá nhỏ (< 1 giây)".into());
    }
    Ok(secs)
}

fn stem_of(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output")
        .to_string()
}

#[allow(clippy::too_many_arguments)]
async fn split_one(
    app: &AppHandle,
    path: &str,
    seg_secs: f64,
    out_dir: &str,
    info: &MediaInfo,
    sub: &str,
    file_index: usize,
    total: usize,
) -> Result<Vec<OutFile>, String> {
    let subdir = Path::new(out_dir).join(sub);
    std::fs::create_dir_all(&subdir).map_err(|e| e.to_string())?;
    cleanup_segments(&subdir, sub, &info.ext); // xoá đoạn cũ cùng pattern (tránh lẫn lần chạy trước)
    let pattern = subdir
        .join(format!("{sub}_%03d.{}", info.ext))
        .to_string_lossy()
        .to_string();

    let args: Vec<String> = [
        "-v", "error", "-progress", "pipe:1", "-i", path, "-f", "segment", "-segment_time",
    ]
    .iter()
    .map(|s| s.to_string())
    .chain([format!("{seg_secs}")])
    .chain(["-reset_timestamps", "1", "-c", "copy", &pattern].iter().map(|s| s.to_string()))
    .collect();

    let (mut events, _child) = ffmpeg_command(app)?
        .args(args)
        .spawn()
        .map_err(|e| e.to_string())?;

    let dur = info.duration_s.max(0.001);
    while let Some(ev) = events.recv().await {
        match ev {
            CommandEvent::Stdout(b) => {
                let line = String::from_utf8_lossy(&b);
                if let Some(us) = line
                    .trim()
                    .strip_prefix("out_time_us=")
                    .and_then(|v| v.trim().parse::<f64>().ok())
                {
                    let file_pct = ((us / 1e6) / dur * 100.0).clamp(0.0, 100.0);
                    let overall_pct = (file_index as f64 * 100.0 + file_pct) / total as f64;
                    let _ = app.emit(
                        "split_progress",
                        SplitProgress {
                            file_index,
                            total,
                            file_name: sub.to_string(),
                            file_pct,
                            overall_pct,
                        },
                    );
                }
            }
            CommandEvent::Terminated(_) => break,
            _ => {}
        }
    }
    Ok(list_outputs(&subdir, sub, &info.ext))
}

/// Batch tuần tự. Lỗi 1 file → emit split_error + tiếp tục file kế.
pub async fn split_files(
    app: AppHandle,
    paths: Vec<String>,
    criterion: String,
    value: f64,
    out_dir: String,
) -> Result<(), String> {
    let total = paths.len();
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
        let result = async {
            let info = probe_file(&app, path).await?;
            let seg = compute_segment_time(&criterion, value, &info)?;
            split_one(&app, path, seg, &out_dir, &info, &sub, i, total).await
        }
        .await;
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

/// Khớp ĐÚNG "<sub>_<digits>.<ext>" (không match nhầm stem khác cùng tiền tố).
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
