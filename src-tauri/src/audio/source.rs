// Nguồn ghi Mode A: mic (cpal) hoặc tiếng hệ thống (loopback WASAPI — Windows).
// Thống nhất vòng đời để recorder.rs xử lý chung: build → play → stop.
use super::capture::{self, CaptureConfig};
use cpal::Stream;
use std::sync::mpsc::Sender;
#[cfg(windows)]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(windows)]
use std::sync::Arc;
#[cfg(windows)]
use std::thread::JoinHandle;

/// Nguồn ghi đang chạy. Cpal::Stream là !Send → sống trên thread ghi; Loopback chạy thread riêng.
pub enum RunningCapture {
    Cpal(Stream),
    #[cfg(windows)]
    Loopback {
        stop: Arc<AtomicBool>,
        handle: Option<JoinHandle<()>>,
    },
}

impl RunningCapture {
    pub fn play(&self) -> Result<(), String> {
        match self {
            RunningCapture::Cpal(s) => {
                use cpal::traits::StreamTrait;
                s.play().map_err(|e| e.to_string())
            }
            #[cfg(windows)]
            RunningCapture::Loopback { .. } => Ok(()), // thread loopback đã chạy từ lúc build
        }
    }

    pub fn stop(self) {
        match self {
            RunningCapture::Cpal(s) => drop(s), // drop → ngừng capture
            #[cfg(windows)]
            RunningCapture::Loopback { stop, handle } => {
                stop.store(true, Ordering::SeqCst);
                if let Some(h) = handle {
                    let _ = h.join();
                }
            }
        }
    }
}

/// Build nguồn theo `source` ("system" = loopback output; còn lại = mic). Trả handle + config.
pub fn build_capture(
    source: &str,
    device: Option<&str>,
    tx: Sender<Vec<i16>>,
    on_error: impl Fn(String) + Send + 'static,
) -> Result<(RunningCapture, CaptureConfig), String> {
    if source == "system" {
        #[cfg(windows)]
        {
            let stop = Arc::new(AtomicBool::new(false));
            let (cfg, handle) = super::loopback::start_loopback_capture(
                device.map(|s| s.to_string()),
                tx,
                stop.clone(),
                on_error,
            )?;
            Ok((RunningCapture::Loopback { stop, handle: Some(handle) }, cfg))
        }
        #[cfg(not(windows))]
        {
            let _ = (device, tx, on_error);
            Err("Ghi tiếng hệ thống hiện chỉ hỗ trợ Windows".into())
        }
    } else {
        let (stream, cfg) = capture::build_stream(device, tx, on_error)?;
        Ok((RunningCapture::Cpal(stream), cfg))
    }
}
