// Build ffmpeg args cho ghi live: stdin s16le (rate/channels gốc) → segment muxer (mp3/m4a).
// Output chuẩn hóa mono 48k. MP3 default (stream-friendly); M4A dùng frag để crash-safe.
const BITRATE_KBPS: u32 = 128;

/// Quy đổi đơn vị split → segment_time (giây). MB tính theo bitrate cố định 128k.
pub fn segment_time_secs(unit: &str, value: f64) -> f64 {
    match unit {
        "megabytes" => value * 8_000_000.0 / (BITRATE_KBPS as f64 * 1000.0), // = value * 62.5
        _ => value * 60.0,                                                    // minutes (default)
    }
}

pub fn ext_for(format: &str) -> &'static str {
    match format {
        "m4a" => "m4a",
        "mp4" => "mp4", // ghi màn hình
        _ => "mp3",
    }
}

/// Cấu hình ghi màn hình. `region` = None → toàn màn hình; Some → vùng (px VẬT LÝ theo monitor đó).
pub struct VideoConfig {
    pub monitor: u32, // ddagrab output_idx (= index monitor theo thứ tự enumerate)
    pub fps: u32,
    pub region: Option<Region>,
}

/// Vùng ghi (px vật lý, gốc toạ độ = góc trên-trái của monitor được chọn).
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

// Bitrate video mục tiêu. Nội dung tĩnh (màn hình) thường dùng ít hơn nhiều; đây là trần cho cảnh động.
const VIDEO_BITRATE_KBPS: u32 = 6000;

/// Args ffmpeg ghi màn hình: ddagrab (video, input 0) + stdin s16le (audio, input 1) → segment MP4.
/// Dừng bằng cách ĐÓNG STDIN (audio EOF): `-shortest` làm ffmpeg kết thúc theo stream ngắn nhất (audio)
/// rồi finalize MP4 hợp lệ — KHÔNG kill cứng (kill để lại file ~rỗng vì fragment chưa flush xuống đĩa).
/// Video tải GPU→RAM (hwdownload) rồi encode H.264 bằng openh264 (software).
pub fn ffmpeg_args_screen(
    rate: u32,
    channels: u16,
    video: &VideoConfig,
    seg_secs: f64,
    out_pattern: &str,
) -> Vec<String> {
    let mut a: Vec<String> = Vec::new();
    let push = |a: &mut Vec<String>, items: &[&str]| {
        for s in items {
            a.push((*s).to_string());
        }
    };

    push(&mut a, &["-hide_banner", "-loglevel", "error"]);

    // Input 0 (video): Desktop Duplication (DXGI). draw_mouse để thấy con trỏ; region qua offset/size.
    let mut dda = format!(
        "ddagrab=output_idx={}:framerate={}:draw_mouse=1",
        video.monitor, video.fps
    );
    if let Some(r) = &video.region {
        dda.push_str(&format!(
            ":offset_x={}:offset_y={}:video_size={}x{}",
            r.x, r.y, r.w, r.h
        ));
    }
    push(&mut a, &["-f", "lavfi", "-i"]);
    a.push(dda);

    // Input 1 (audio): PCM từ Rust qua stdin. thread_queue_size để 2 input đọc song song, không kẹt nhau.
    push(&mut a, &["-thread_queue_size", "1024", "-f", "s16le", "-ar"]);
    a.push(rate.to_string());
    push(&mut a, &["-ac"]);
    a.push(channels.to_string());
    push(&mut a, &["-i", "-"]);

    // Map + encode: tải frame GPU→RAM, ép yuv420p (H.264 cần kích thước chẵn), openh264 + AAC.
    push(&mut a, &["-map", "0:v", "-map", "1:a"]);
    push(&mut a, &["-filter:v", "hwdownload,format=bgra", "-pix_fmt", "yuv420p"]);
    push(&mut a, &["-c:v", "libopenh264", "-b:v"]);
    a.push(format!("{VIDEO_BITRATE_KBPS}k"));
    push(&mut a, &["-g"]);
    a.push((video.fps * 2).to_string());
    push(&mut a, &["-c:a", "aac", "-b:a", "160k"]);

    // Kết thúc theo stream ngắn nhất (audio): khi dừng ghi ta đóng stdin → audio EOF → ffmpeg thoát
    // dù ddagrab vô hạn, rồi ghi trailer/flush fragment → file MP4 hợp lệ (tránh kill cứng để lại file rỗng).
    push(&mut a, &["-shortest"]);

    // Cắt đoạn theo thời gian + ép keyframe đúng mốc cắt → mỗi đoạn phát độc lập. frag → crash-safe.
    push(&mut a, &["-f", "segment", "-segment_time"]);
    a.push(format!("{seg_secs}"));
    push(&mut a, &["-reset_timestamps", "1", "-segment_format", "mp4"]);
    push(&mut a, &["-force_key_frames"]);
    a.push(format!("expr:gte(t,n_forced*{seg_secs})"));
    push(&mut a, &["-segment_format_options", "movflags=+frag_keyframe+empty_moov"]);
    a.push(out_pattern.to_string());
    a
}

/// Args ffmpeg đầy đủ. `out_pattern` ví dụ "<dir>/rec_..._%03d.mp3".
pub fn ffmpeg_args(rate: u32, channels: u16, format: &str, seg_secs: f64, out_pattern: &str) -> Vec<String> {
    let mut a: Vec<String> = Vec::new();
    let push = |a: &mut Vec<String>, items: &[&str]| {
        for s in items {
            a.push((*s).to_string());
        }
    };

    push(&mut a, &["-hide_banner", "-loglevel", "error", "-f", "s16le"]);
    push(&mut a, &["-ar"]);
    a.push(rate.to_string());
    push(&mut a, &["-ac"]);
    a.push(channels.to_string());
    push(&mut a, &["-i", "-", "-ar", "48000", "-ac", "1"]);

    if format == "m4a" {
        push(&mut a, &["-c:a", "aac", "-b:a", "128k"]);
    } else {
        push(&mut a, &["-c:a", "libmp3lame", "-b:a", "128k"]);
    }
    push(&mut a, &["-f", "segment", "-segment_time"]);
    a.push(format!("{seg_secs}"));
    push(&mut a, &["-reset_timestamps", "1"]);
    if format == "m4a" {
        push(&mut a, &["-segment_format_options", "movflags=+frag_keyframe+empty_moov"]);
    }
    a.push(out_pattern.to_string());
    a
}
