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
    pub codec: String,  // codec audio (hiển thị)
    pub vcodec: String, // codec video — quyết định có ép tag hvc1 khi cắt HEVC vào mp4/mov
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
    let find_codec = |kind: &str| {
        json["streams"]
            .as_array()
            .and_then(|s| s.iter().find(|x| x["codec_type"] == kind))
            .and_then(|x| x["codec_name"].as_str())
            .unwrap_or("")
            .to_string()
    };
    let codec = find_codec("audio");
    let vcodec = find_codec("video");
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mp3")
        .to_string();
    Ok(MediaInfo { duration_s, bitrate_bps, codec, vcodec, ext })
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

/// Cách cắt cho ffmpeg segment muxer.
enum SegmentArg {
    /// Cắt đều theo thời gian (Phút / Chia-K) → -segment_time.
    Time(f64),
    /// Cắt tại các mốc thời gian cụ thể (Cắt theo MB) → -segment_times.
    Times(Vec<f64>),
}

/// Mốc thời gian cắt cho tiêu chí MB: keyframe video ĐẦU TIÊN có byte-offset (pos) vượt mỗi bội số
/// target_bytes. Bám byte THỰC TẾ trong file (không phải bitrate trung bình) → kích thước đoạn đều hơn
/// hẳn với video VBR. Vẫn cắt tại keyframe (-c copy lossless) nên đoạn có thể nhỉnh hơn target chút ít
/// (+ overhead container mỗi đoạn). Rỗng (probe lỗi / file nhỏ hơn 1 đoạn) → caller fallback bitrate TB.
async fn size_cut_times(app: &AppHandle, path: &str, target_bytes: f64) -> Vec<f64> {
    let out = match ffprobe_command(app) {
        Ok(c) => {
            c.args([
                "-v", "error", "-select_streams", "v:0", "-show_entries",
                "packet=pts_time,pos,flags", "-of", "csv=p=0", path,
            ])
            .output()
            .await
        }
        Err(_) => return Vec::new(),
    };
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

#[allow(clippy::too_many_arguments)]
async fn split_one(
    app: &AppHandle,
    path: &str,
    seg: SegmentArg,
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

    // Lệnh cắt: segment muxer + -c copy (lossless). Phút/Chia-K → cắt đều (-segment_time); MB → cắt
    // tại các mốc keyframe đã tính theo byte (-segment_times) cho kích thước đều hơn với video VBR.
    let mut args: Vec<String> = vec![
        "-v".into(), "error".into(),
        "-progress".into(), "pipe:1".into(),
        "-i".into(), path.into(),
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
    // HEVC trong MP4/MOV: ép tag hvc1 — mặc định ffmpeg ghi hev1 khiến nhiều player Windows (Media
    // Player, Photos) chỉ phát tiếng, mất hình. Chỉ đổi nhãn, vẫn -c copy → không re-encode.
    let is_hevc = matches!(info.vcodec.as_str(), "hevc" | "h265");
    if is_hevc && matches!(info.ext.to_ascii_lowercase().as_str(), "mp4" | "mov" | "m4v") {
        args.push("-tag:v".into());
        args.push("hvc1".into());
    }
    args.push(pattern);

    let (mut events, _child) = ffmpeg_command(app)?
        .args(args)
        .spawn()
        .map_err(|e| e.to_string())?;

    let dur = info.duration_s.max(0.001);
    let mut stderr_buf = String::new();
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
            // Gom stderr (đang -v error) → báo lỗi rõ thay vì im lặng tạo ra 0 đoạn.
            CommandEvent::Stderr(b) => stderr_buf.push_str(&String::from_utf8_lossy(&b)),
            CommandEvent::Terminated(_) => break,
            _ => {}
        }
    }
    let outputs = list_outputs(&subdir, sub, &info.ext);
    if outputs.is_empty() {
        return Err(if stderr_buf.trim().is_empty() {
            "ffmpeg không tạo được đoạn nào".to_string()
        } else {
            format!("ffmpeg: {}", stderr_buf.trim())
        });
    }
    Ok(outputs)
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
            // MB: cắt theo byte thực tế tại keyframe (đều hơn cho VBR). Không lấy được keyframe/pos
            // hoặc file nhỏ hơn 1 đoạn → fallback ước lượng theo bitrate trung bình.
            let seg = if criterion == "megabytes" {
                let target_bytes = value * 1_000_000.0;
                let times = if target_bytes >= 1.0 {
                    size_cut_times(&app, path, target_bytes).await
                } else {
                    Vec::new()
                };
                if times.is_empty() {
                    SegmentArg::Time(compute_segment_time(&criterion, value, &info)?)
                } else {
                    SegmentArg::Times(times)
                }
            } else {
                SegmentArg::Time(compute_segment_time(&criterion, value, &info)?)
            };
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
