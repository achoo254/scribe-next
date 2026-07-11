//! Unit test thuật toán quy đổi tiêu chí → segment_time (không cần ffmpeg/ffprobe).

use core_split::{compute_segment_time, MediaInfo, SplitCriterion};

fn info(duration_s: f64, bitrate_bps: u64) -> MediaInfo {
    MediaInfo {
        duration_s,
        bitrate_bps,
        codec: "aac".into(),
        vcodec: "".into(),
        ext: "mp4".into(),
    }
}

#[test]
fn minutes_to_seconds() {
    let i = info(600.0, 128_000);
    assert_eq!(compute_segment_time(SplitCriterion::Minutes, 5.0, &i).unwrap(), 300.0);
}

#[test]
fn parts_divides_duration() {
    let i = info(600.0, 128_000);
    assert_eq!(compute_segment_time(SplitCriterion::Parts, 4.0, &i).unwrap(), 150.0);
}

#[test]
fn megabytes_uses_bitrate() {
    // 10 MB @ 1_000_000 bps → 10 * 8_000_000 / 1_000_000 = 80 giây.
    let i = info(600.0, 1_000_000);
    assert_eq!(compute_segment_time(SplitCriterion::Megabytes, 10.0, &i).unwrap(), 80.0);
}

#[test]
fn megabytes_without_bitrate_errs() {
    let i = info(600.0, 0);
    assert!(compute_segment_time(SplitCriterion::Megabytes, 10.0, &i).is_err());
}

#[test]
fn parts_without_duration_errs() {
    let i = info(0.0, 128_000);
    assert!(compute_segment_time(SplitCriterion::Parts, 4.0, &i).is_err());
}

#[test]
fn zero_or_negative_value_errs() {
    let i = info(600.0, 128_000);
    assert!(compute_segment_time(SplitCriterion::Minutes, 0.0, &i).is_err());
    assert!(compute_segment_time(SplitCriterion::Minutes, -1.0, &i).is_err());
}

#[test]
fn too_small_segment_errs() {
    // 0.001 phút = 0.06s < 1s → lỗi.
    let i = info(600.0, 128_000);
    assert!(compute_segment_time(SplitCriterion::Minutes, 0.001, &i).is_err());
}

#[test]
fn parts_less_than_one_errs() {
    let i = info(600.0, 128_000);
    assert!(compute_segment_time(SplitCriterion::Parts, 0.4, &i).is_err());
}
