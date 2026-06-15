// Audio module — Mode A (ghi âm + split live).
pub mod capture;
pub mod encoder;
pub mod meter;
pub mod source;
// Loopback ghi tiếng output hệ thống — chỉ Windows (WASAPI).
#[cfg(windows)]
pub mod loopback;
