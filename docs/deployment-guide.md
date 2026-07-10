# Deployment Guide — OneScribe

Quy trình build + phát hành installer OneScribe. **Windows** (NSIS) + **macOS Apple Silicon** (.dmg, ad-hoc nội bộ) = đã hỗ trợ.

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

## macOS — DMG (Apple Silicon, ad-hoc nội bộ)

### Kiến trúc đóng gói

- **Chỉ Apple Silicon (arm64)**. PyInstaller + Tauri không cross-build sidecar Python → build native trên máy arm64. Bản Intel (x86_64) cần máy Intel Mac / CI riêng (chưa làm).
- **Bundle target**: `app` + `dmg`, override qua `src-tauri/tauri.macos.conf.json` (Tauri v2 tự merge file platform này đè lên `tauri.conf.json` khi build macOS — base `nsis` chỉ dùng cho Windows).
- **3 sidecar** đúng target-triple trong `src-tauri/binaries/`: `ffmpeg-aarch64-apple-darwin`, `ffprobe-aarch64-apple-darwin` (ffmpeg **LGPL** 8.x), `transcriber-aarch64-apple-darwin` (PyInstaller onefile; macOS arm64 gói thêm **mlx/mlx-whisper** cho model `large-v3-mlx` Metal GPU, loại `torch`). Tauri copy sang tên trần cạnh `onescribe` trong `.app/Contents/MacOS/`.
- **Quyền + ký**:
  - `src-tauri/Info.plist` → `NSMicrophoneUsageDescription` (Tauri merge vào Info.plist của .app; thiếu → crash khi mở mic).
  - `src-tauri/entitlements.plist` → `device.audio-input` + `disable-library-validation` (nạp dylib ct2/mlx khác chữ ký) + `allow-unsigned-executable-memory` (PyInstaller onefile).
  - **Ad-hoc sign** (`signingIdentity: "-"` trong `tauri.macos.conf.json`) — KHÔNG notarize (tool nội bộ, không cần Apple Developer). Tauri tự re-sign từng sidecar + .app.
- **Model KHÔNG nhúng** — tải-khi-dùng như Windows.

### Các bước build (trên máy Apple Silicon)

```bash
# 1) Sidecar ffmpeg/ffprobe LGPL macOS arm64 → src-tauri/binaries/
#    ffmpeg-aarch64-apple-darwin, ffprobe-aarch64-apple-darwin (tải 1 lần; gitignored)

# 2) Build lại transcriber sidecar (BẮT BUỘC khi main.py/requirements/spec đổi)
PYTHON=python3 bash sidecar/transcriber/build_pyinstaller.sh
#    → src-tauri/binaries/transcriber-aarch64-apple-darwin

# 3) Đóng gói .app + .dmg
npm run tauri build
#    → src-tauri/target/release/bundle/dmg/OneScribe_<version>_aarch64.dmg
#    → src-tauri/target/release/bundle/macos/OneScribe.app
```

> **LƯU Ý stale sidecar** (giống Windows): `transcriber` là bundle của `main.py` lúc build. `main.py`/`requirements.txt`/`transcriber.spec` đổi → build lại bước 2 trước `tauri build`.

### Verify sau build (không cần máy sạch)

- Artifact: `bundle/dmg/OneScribe_<ver>_aarch64.dmg` (~185 MB) + `bundle/macos/OneScribe.app`.
- Đủ 4 binary trong `OneScribe.app/Contents/MacOS/`: `onescribe`, `ffmpeg`, `ffprobe`, `transcriber`.
- Chữ ký hợp lệ: `codesign --verify --deep --strict OneScribe.app` → "valid on disk".
- Quyền: `codesign -d --entitlements - OneScribe.app` có `device.audio-input`; `Info.plist` có `NSMicrophoneUsageDescription`.
- Smoke sidecar: `printf '{"cmd":"shutdown"}\n' | OneScribe.app/Contents/MacOS/transcriber` → `{"type":"ready"}`.

### Cài đặt trên máy người dùng (Gatekeeper — QUAN TRỌNG)

Vì ad-hoc (không notarize), Gatekeeper chặn lần đầu. Người dùng làm **1 trong 2** cách:

1. **Chuột phải → Mở** (khuyến nghị): kéo OneScribe.app vào /Applications → chuột phải vào app → **Open** → hộp thoại cảnh báo → bấm **Open**. Chỉ cần 1 lần.
2. **Gỡ cờ quarantine bằng lệnh** (nếu cách 1 báo "damaged / không mở được"):
   ```bash
   xattr -cr /Applications/OneScribe.app
   ```

Lần đầu Mode A/ghi âm → macOS hỏi quyền **micro** → Cho phép. Lần đầu Mode C/transcribe → app tự tải model (~0.5–3 GB theo size), sau đó chạy offline.

### QA máy sạch (Reviewer/QA — máy KHÔNG có Python/Rust/dev tools)

- [ ] Mở `.dmg`, kéo app vào /Applications, mở được (qua bước Gatekeeper ở trên); hiện 3 tab, theme đúng.
- [ ] **Mode A**: ghi mic → xin quyền micro, có level meter, split đoạn. (Ghi màn hình/loopback = Windows-only, không có trên macOS.)
- [ ] **Mode B**: cắt 1 file audio/video theo phút → các đoạn phát được.
- [ ] **Mode C**: transcribe 1 file VN → lần đầu tải model → ra `.txt/.srt/.vtt/.json`; player click-to-seek; sửa + Lưu. Thử model `large-v3-mlx` (Metal GPU) nếu máy đủ RAM.
- [ ] Sau khi tải model 1 lần → tắt mạng → transcribe vẫn chạy (offline).

## CI (tùy chọn, chưa làm)

GitHub Actions matrix windows + macos: build sidecar per-OS → `tauri build` → upload artifact; macOS thêm sign/notarize qua secrets.
