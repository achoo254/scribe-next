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
    if format == "m4a" {
        "m4a"
    } else {
        "mp3"
    }
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
