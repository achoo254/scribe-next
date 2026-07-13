//! Regression tích hợp: cắt file THẬT qua ffmpeg (input sinh bằng lavfi) → xác nhận pipeline đầu-cuối
//! (probe → segment_time → ffmpeg `-c copy` → naming/subdir). Bỏ qua nếu máy không có ffmpeg/ffprobe
//! trên PATH (in cảnh báo, KHÔNG fail) để CI không cài ffmpeg vẫn xanh.
//!
//! Desktop (`src-tauri/src/splitter.rs`) nay uỷ thác toàn bộ thuật toán cắt cho crate này → test ở đây
//! bảo chứng luôn hành vi cắt của desktop Mode B (một nguồn chân lý, không còn bản song sinh).

use core_split::{probe_file, split_file, split_file_into, SplitCriterion};
use std::path::{Path, PathBuf};
use std::process::Command;

const FFMPEG: &str = "ffmpeg";
const FFPROBE: &str = "ffprobe";

fn tools_available() -> bool {
    let ok = |bin: &str| {
        Command::new(bin)
            .arg("-version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    };
    ok(FFMPEG) && ok(FFPROBE)
}

/// Thư mục tạm riêng cho từng test (dọn đầu + cuối). pid tách các lần chạy song song.
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("core-split-it-{}-{}", std::process::id(), tag));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Sinh file wav `secs` giây bằng ffmpeg lavfi (pcm — không phụ thuộc encoder ngoài như libmp3lame).
fn gen_wav(dir: &Path, name: &str, secs: u32) -> String {
    let path = dir.join(name);
    let status = Command::new(FFMPEG)
        .args([
            "-v", "error", "-y",
            "-f", "lavfi",
            "-i", &format!("sine=frequency=440:duration={secs}"),
            path.to_str().unwrap(),
        ])
        .status()
        .expect("spawn ffmpeg gen");
    assert!(status.success(), "ffmpeg sinh wav thất bại");
    path.to_string_lossy().into_owned()
}

#[tokio::test]
async fn probe_reports_duration_and_ext() {
    if !tools_available() {
        eprintln!("SKIP probe_reports_duration_and_ext: ffmpeg/ffprobe không có trên PATH");
        return;
    }
    let dir = temp_dir("probe");
    let input = gen_wav(&dir, "tone.wav", 8);
    let info = probe_file(FFPROBE, &input).await.expect("probe");
    assert!((info.duration_s - 8.0).abs() < 0.5, "duration {} kỳ vọng ~8s", info.duration_s);
    assert!(info.bitrate_bps > 0, "bitrate phải > 0");
    assert_eq!(info.ext, "wav");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn split_parts_creates_expected_segments() {
    if !tools_available() {
        eprintln!("SKIP split_parts_creates_expected_segments: ffmpeg/ffprobe không có trên PATH");
        return;
    }
    let dir = temp_dir("parts");
    let input = gen_wav(&dir, "song.wav", 12);
    let out = dir.join("out");
    // 12s chia 4 phần → segment_time 3s → ~4 đoạn (dung sai ±1 do căn theo packet).
    let segs = split_file(FFMPEG, FFPROBE, &input, SplitCriterion::Parts, 4.0, out.to_str().unwrap(), |_| {})
        .await
        .expect("split_file");
    assert!((3..=5).contains(&segs.len()), "số đoạn {} ngoài [3,5]", segs.len());
    // Đoạn vào out/song/, tên song_000.wav... (list_outputs sort tăng dần).
    assert_eq!(segs[0].name, "song_000.wav");
    assert!(Path::new(&segs[0].path).starts_with(out.join("song")));
    for s in &segs {
        let non_empty = std::fs::metadata(&s.path).map(|m| m.len() > 0).unwrap_or(false);
        assert!(non_empty, "đoạn rỗng: {}", s.name);
    }
    // Đoạn đầu ~3s → segment_time đã chảy đúng vào ffmpeg (không lệch tiêu chí).
    let first = probe_file(FFPROBE, &segs[0].path).await.expect("probe đoạn đầu");
    assert!((2.0..=4.0).contains(&first.duration_s), "đoạn đầu {}s ngoài [2,4]", first.duration_s);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn split_file_into_uses_caller_subdir() {
    if !tools_available() {
        eprintln!("SKIP split_file_into_uses_caller_subdir: ffmpeg/ffprobe không có trên PATH");
        return;
    }
    let dir = temp_dir("subdir");
    let input = gen_wav(&dir, "clip.wav", 6);
    let out = dir.join("out");
    // sub tường minh KHÁC stem → đúng cơ chế desktop batch né trùng tên giữa nhiều file nguồn.
    let segs = split_file_into(
        FFMPEG, FFPROBE, &input, SplitCriterion::Parts, 2.0, out.to_str().unwrap(), "clip_2", |_| {},
    )
    .await
    .expect("split_file_into");
    assert!(!segs.is_empty());
    assert_eq!(segs[0].name, "clip_2_000.wav");
    assert!(Path::new(&segs[0].path).starts_with(out.join("clip_2")));
    // KHÔNG được tạo subdir theo stem "clip" khi caller đã chỉ định sub.
    assert!(!out.join("clip").exists(), "không được tạo subdir theo stem khi có sub tường minh");
    let _ = std::fs::remove_dir_all(&dir);
}
