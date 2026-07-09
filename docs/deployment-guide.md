# Deployment Guide — OneScribe

Quy trình build + phát hành installer OneScribe. **Windows = đã hỗ trợ đầy đủ**; macOS = chưa làm (xem cuối).

> Yêu cầu môi trường build (Node/Rust/MSVC/Python) đã liệt kê trong `README.md` → mục "Yêu cầu môi trường". File này chỉ mô tả **quy trình đóng gói release** + verify.

## Windows — NSIS installer

### Kiến trúc đóng gói

- **Bundle target**: `nsis` (`src-tauri/tauri.conf.json`). Output 1 file `*-setup.exe`.
- **3 sidecar** khai báo `externalBin` (`binaries/ffmpeg`, `binaries/ffprobe`, `binaries/transcriber`):
  - `ffmpeg-<target>.exe` + `ffprobe-<target>.exe` — bản **ffmpeg LGPL** (BtbN), tải riêng (gitignored).
  - `transcriber-<target>.exe` — Python sidecar faster-whisper, build bằng PyInstaller onefile.
  - Lúc `tauri build`, Tauri copy externalBin sang **tên trần** (`ffmpeg.exe`/`ffprobe.exe`/`transcriber.exe`) cạnh `onescribe.exe`, rồi nhét vào installer. `resolve()` trong `transcribe.rs` tìm `transcriber.exe` cạnh app; ffmpeg/ffprobe qua `app.shell().sidecar()`.
- **Model KHÔNG nhúng** — tải-khi-dùng lần đầu (~0.5/1.5/3 GB theo size) về app-data, sau đó offline. Installer chỉ ~174 MB.

### Các bước build

```powershell
# 1) Sidecar ffmpeg/ffprobe LGPL (BtbN) → đặt vào src-tauri/binaries/ với đúng target-triple:
#    ffmpeg-x86_64-pc-windows-msvc.exe, ffprobe-x86_64-pc-windows-msvc.exe
#    (tải 1 lần; gitignored — xem README mục Sidecar)

# 2) Build lại transcriber sidecar (BẮT BUỘC khi sidecar/transcriber/main.py đổi)
sidecar\transcriber\build_pyinstaller.ps1
#    → src-tauri\binaries\transcriber-x86_64-pc-windows-msvc.exe

# 3) Đóng gói installer
npm run tauri build
#    → src-tauri\target\release\bundle\nsis\OneScribe_<version>_x64-setup.exe
```

> **LƯU Ý stale sidecar**: `transcriber.exe` là PyInstaller onefile bundle của `main.py` tại thời điểm build. Mỗi khi `main.py` / `requirements.txt` / `transcriber.spec` đổi → **phải build lại** (bước 2) trước khi `tauri build`, nếu không installer sẽ mang bản transcriber cũ dù frontend đã cập nhật.

### Verify sau build (không cần máy sạch)

- Artifact tồn tại: `src-tauri/target/release/bundle/nsis/OneScribe_<ver>_x64-setup.exe`, size ~170–185 MB (đã gồm sidecar; ~5 MB là dấu hiệu THIẾU sidecar).
- 3 sidecar staged cạnh `onescribe.exe` trong `src-tauri/target/release/` (`ffmpeg.exe`, `ffprobe.exe`, `transcriber.exe`) — timestamp `transcriber.exe` phải khớp lần build bước 2.
- Smoke test transcriber onefile: `echo '{"cmd":"shutdown"}' | transcriber-<target>.exe` → in `{"type":"ready"}` rồi exit 0.

### QA máy sạch (Reviewer/QA — máy KHÔNG có Python/Rust/dev tools)

- [ ] Cài `OneScribe_<ver>_x64-setup.exe`; app mở, hiện 3 tab, theme dark đúng.
- [ ] **Mode A**: ghi mic → có level meter, split đoạn; ghi "Tiếng hệ thống" (loopback) ra file có tiếng.
- [ ] **Mode B**: cắt 1 file audio/video theo phút → các đoạn phát được.
- [ ] **Mode C**: transcribe 1 file VN → lần đầu tự tải model (báo % download) → ra `.txt/.srt/.vtt/.json`; player click-to-seek; sửa + Lưu.
- [ ] Auto-transcript khi ghi (Mode A toggle) chạy được.
- [ ] Sau khi tải model 1 lần → tắt mạng → transcribe vẫn chạy (offline).
- [ ] Ghi kèm màn hình → MP4 (Windows) hợp lệ.

## macOS — CHƯA LÀM (future)

Cần (theo `plans/260613-1130-split-record-implementation/phase-05-*`): ffmpeg/ffprobe LGPL macOS (intel+arm), transcriber PyInstaller build trên macOS (`build_pyinstaller.sh`), `tauri.macos.conf.json` override target `dmg`/`app`, `Info.plist` `NSMicrophoneUsageDescription`, entitlements + ký Developer ID + notarize (`notarytool`) + staple. Ghi màn hình + loopback KHÔNG chạy trên macOS (Windows-only).

## CI (tùy chọn, chưa làm)

GitHub Actions matrix windows + macos: build sidecar per-OS → `tauri build` → upload artifact; macOS thêm sign/notarize qua secrets.
