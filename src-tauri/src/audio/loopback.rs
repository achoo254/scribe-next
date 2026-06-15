// Ghi "tiếng output" hệ thống (loopback) qua WASAPI — Windows-only.
// Mẹo loopback: mở RENDER device rồi initialize_client ở Direction::Capture (shared)
// → wasapi tự bật AUDCLNT_STREAMFLAGS_LOOPBACK. Request format i16 PCM + autoconvert
// → WASAPI tự convert (float→i16, resample) nên không cần xử lý mẫu thủ công.
// Toàn bộ object COM tạo + dùng + drop TRỌN trong thread capture (COM theo apartment-thread).
use super::capture::{CaptureConfig, DeviceInfo};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use wasapi::{
    initialize_mta, Direction, DeviceEnumerator, SampleType, StreamMode, WaveFormat,
};

// Format giao cho ffmpeg: i16 PCM stereo 48k (WASAPI autoconvert từ mix-format gốc của render).
const RATE: usize = 48_000;
const CHANNELS: usize = 2;

/// Liệt kê output (render) device để chọn nguồn "tiếng hệ thống".
pub fn list_output_devices() -> Vec<DeviceInfo> {
    let mut out = Vec::new();
    let _ = initialize_mta(); // best-effort; nếu COM đã init khác mode vẫn dùng được
    if let Ok(en) = DeviceEnumerator::new() {
        if let Ok(coll) = en.get_device_collection(&Direction::Render) {
            let n = coll.get_nbr_devices().unwrap_or(0);
            for i in 0..n {
                if let Ok(dev) = coll.get_device_at_index(i) {
                    if let Ok(name) = dev.get_friendlyname() {
                        out.push(DeviceInfo { id: name.clone(), name });
                    }
                }
            }
        }
    }
    out
}

/// Bắt đầu loopback: spawn thread capture, trả CaptureConfig (sau khi mở thiết bị OK) + handle.
/// Dừng: set `stop` rồi join handle.
pub fn start_loopback_capture(
    device_name: Option<String>,
    tx: Sender<Vec<i16>>,
    stop: Arc<AtomicBool>,
    on_error: impl Fn(String) + Send + 'static,
) -> Result<(CaptureConfig, JoinHandle<()>), String> {
    let (cfg_tx, cfg_rx) = std::sync::mpsc::channel::<Result<CaptureConfig, String>>();
    let stop_thread = stop.clone(); // closure giữ bản clone; `stop` gốc dùng cho nhánh timeout
    let handle = thread::Builder::new()
        .name("loopback".into())
        .spawn(move || {
            if let Err(e) = capture_loop(device_name, tx, &stop_thread, &cfg_tx) {
                // Lỗi trước khi gửi config → caller nhận qua cfg_tx; lỗi runtime (config đã gửi,
                // cfg_rx đã drop) → gửi thất bại → báo qua on_error.
                if cfg_tx.send(Err(e.clone())).is_err() {
                    on_error(e);
                }
            }
        })
        .map_err(|e| e.to_string())?;

    match cfg_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(cfg)) => Ok((cfg, handle)),
        Ok(Err(e)) => {
            let _ = handle.join();
            Err(e)
        }
        Err(_) => {
            stop.store(true, Ordering::SeqCst);
            let _ = handle.join();
            Err("Hết thời gian chờ mở thiết bị loopback".into())
        }
    }
}

fn capture_loop(
    device_name: Option<String>,
    tx: Sender<Vec<i16>>,
    stop: &AtomicBool,
    cfg_tx: &Sender<Result<CaptureConfig, String>>,
) -> Result<(), String> {
    let _ = initialize_mta(); // thread mới → khởi tạo COM (MTA)
    let en = DeviceEnumerator::new().map_err(|e| e.to_string())?;
    let device = match device_name.as_deref() {
        Some(n) if !n.is_empty() => en
            .get_device_collection(&Direction::Render)
            .map_err(|e| e.to_string())?
            .get_device_with_name(n)
            .map_err(|e| format!("Không tìm thấy thiết bị output \"{n}\": {e}"))?,
        _ => en
            .get_default_device(&Direction::Render)
            .map_err(|e| e.to_string())?,
    };

    let mut client = device.get_iaudioclient().map_err(|e| e.to_string())?;
    let format = WaveFormat::new(16, 16, &SampleType::Int, RATE, CHANNELS, None);
    let blockalign = format.get_blockalign() as usize; // = CHANNELS * 2 byte
    let (_def, min_time) = client.get_device_period().map_err(|e| e.to_string())?;
    let mode = StreamMode::EventsShared { autoconvert: true, buffer_duration_hns: min_time };
    // Render device + Direction::Capture (shared) → loopback.
    client
        .initialize_client(&format, &Direction::Capture, &mode)
        .map_err(|e| e.to_string())?;
    let h_event = client.set_get_eventhandle().map_err(|e| e.to_string())?;
    let capture = client.get_audiocaptureclient().map_err(|e| e.to_string())?;
    client.start_stream().map_err(|e| e.to_string())?;

    // Mở thiết bị OK → báo config cho caller (recorder.rs cấu hình ffmpeg theo đây).
    let _ = cfg_tx.send(Ok(CaptureConfig { sample_rate: RATE as u32, channels: CHANNELS as u16 }));

    let chunk_frames = 2_048usize;
    let chunk_bytes = blockalign * chunk_frames;
    let mut queue: VecDeque<u8> = VecDeque::with_capacity(chunk_bytes * 8);

    while !stop.load(Ordering::SeqCst) {
        capture
            .read_from_device_to_deque(&mut queue)
            .map_err(|e| e.to_string())?;
        while queue.len() >= chunk_bytes {
            let mut bytes = vec![0u8; chunk_bytes];
            for b in bytes.iter_mut() {
                *b = queue.pop_front().unwrap();
            }
            let samples: Vec<i16> = bytes
                .chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]]))
                .collect();
            if tx.send(samples).is_err() {
                let _ = client.stop_stream();
                return Ok(()); // downstream đóng → dừng êm
            }
        }
        // timeout ngắn để vòng lặp còn kiểm tra cờ stop (render idle → timeout là bình thường).
        let _ = h_event.wait_for_event(200);
    }
    let _ = client.stop_stream();
    Ok(())
}
