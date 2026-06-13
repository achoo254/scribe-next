// cpal capture: liệt kê mic + build input stream → gửi Vec<i16> interleaved qua channel.
// KHÔNG resample/downmix ở Rust — giữ rate/channels gốc, để ffmpeg lo (input -ar/-ac).
// Lưu ý: cpal::Stream là !Send → phải build + giữ trên thread ghi (xem recorder.rs).
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use std::sync::mpsc::Sender;

#[derive(serde::Serialize, Clone)]
pub struct DeviceInfo {
    pub id: String, // cpal định danh theo name
    pub name: String,
}

pub struct CaptureConfig {
    pub sample_rate: u32,
    pub channels: u16,
}

/// Liệt kê mic đầu vào.
pub fn list_input_devices() -> Vec<DeviceInfo> {
    let host = cpal::default_host();
    let mut out = Vec::new();
    if let Ok(devices) = host.input_devices() {
        for d in devices {
            if let Ok(name) = d.name() {
                out.push(DeviceInfo { id: name.clone(), name });
            }
        }
    }
    out
}

fn find_device(name: Option<&str>) -> Result<Device, String> {
    let host = cpal::default_host();
    match name {
        Some(n) if !n.is_empty() => host
            .input_devices()
            .map_err(|e| e.to_string())?
            .find(|d| d.name().map(|dn| dn == n).unwrap_or(false))
            .ok_or_else(|| format!("Không tìm thấy mic: {n}")),
        _ => host
            .default_input_device()
            .ok_or_else(|| "Không có mic mặc định".to_string()),
    }
}

/// Build input stream (chưa play). Callback convert mẫu → i16 rồi gửi qua `tx`.
/// `on_error` báo lỗi runtime (vd rút mic giữa chừng).
pub fn build_stream(
    device_name: Option<&str>,
    tx: Sender<Vec<i16>>,
    on_error: impl Fn(String) + Send + 'static,
) -> Result<(Stream, CaptureConfig), String> {
    let device = find_device(device_name)?;
    let supported = device.default_input_config().map_err(|e| e.to_string())?;
    let sample_format = supported.sample_format();
    let config: StreamConfig = supported.config();
    let cap = CaptureConfig {
        sample_rate: config.sample_rate.0,
        channels: config.channels,
    };
    let err_fn = move |e: cpal::StreamError| on_error(e.to_string());

    let stream = match sample_format {
        SampleFormat::F32 => device.build_input_stream(
            &config,
            move |data: &[f32], _: &_| {
                let buf = data.iter().map(|s| (s.clamp(-1.0, 1.0) * 32767.0) as i16).collect();
                let _ = tx.send(buf);
            },
            err_fn,
            None,
        ),
        SampleFormat::I16 => device.build_input_stream(
            &config,
            move |data: &[i16], _: &_| {
                let _ = tx.send(data.to_vec());
            },
            err_fn,
            None,
        ),
        SampleFormat::U16 => device.build_input_stream(
            &config,
            move |data: &[u16], _: &_| {
                let buf = data.iter().map(|s| (*s as i32 - 32768) as i16).collect();
                let _ = tx.send(buf);
            },
            err_fn,
            None,
        ),
        other => return Err(format!("Định dạng mẫu không hỗ trợ: {other:?}")),
    }
    .map_err(|e| e.to_string())?;

    Ok((stream, cap))
}
