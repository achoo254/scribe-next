# Journal — 2026-08-21 09:57 — MLX transcribe lỗi `ffmpeg` không tìm thấy

## Triệu chứng
App macOS (bundle) → Mode C, model `Whisper Large-v3 (MLX)` → mỗi file lỗi:
`<file>: [Errno 2] No such file or directory: 'ffmpeg'`. Nhánh ct2 (small/medium/large/turbo) bình thường.

## Root cause
`sidecar/transcriber/main.py:_transcribe_mlx` truyền **đường dẫn file** cho `mlx_whisper.transcribe`
→ `mlx_whisper.audio.load_audio` chạy `subprocess(["ffmpeg", ...])` **lấy từ PATH**.
App mở từ Finder có PATH tối thiểu (`/usr/bin:/bin:/usr/sbin:/sbin`) — không có `/opt/homebrew/bin`.
ct2 không dính vì faster-whisper decode bằng PyAV (`av`), không cần ffmpeg CLI.

Reproduce (baseline, code cũ):
`env -i PATH=/usr/bin:/bin HOME=$HOME .venv/bin/python main.py` + JSON transcribe size `large-v3-mlx`
→ đúng thông báo lỗi trên.

## Fix
Decode audio bằng chính PyAV đã bundle (`faster_whisper.audio.decode_audio`, sr = `mlx_whisper.audio.SAMPLE_RATE`)
rồi truyền `ndarray` cho `mlx_whisper.transcribe` (signature nhận `str | np.ndarray | mx.array`).
Bỏ hẳn phụ thuộc ffmpeg CLI ở nhánh MLX. Import lazy trong hàm → Windows không chạm.

## Verify
- Cùng lệnh repro, code mới → `{"type":"done"}` + xuất đủ txt/srt/json.
- Nhánh ct2 (`medium`) với PATH tối thiểu → vẫn done, không đổi.
- Binary PyInstaller rebuild → smoke lại với PATH tối thiểu → done.
- `.app` sau `tauri build`: đủ 4 binary, `codesign --verify --deep --strict` = valid, smoke transcriber trong bundle = done.

## Phát hành
0.2.0 → 0.2.1 (package.json + tauri.conf.json + Cargo.toml). `OneScribe_0.2.1_aarch64.dmg` +
`/Applications/OneScribe.app` (hash binary khớp artifact build, đã `xattr -cr`, chạy được).
Windows: installer 0.2.1 **chưa build** (cần máy Windows chạy `build_pyinstaller.ps1` rồi `tauri build`).

## Bài học
Sidecar Python phụ thuộc CLI ngoài (`ffmpeg`) qua PATH là bẫy chỉ lộ khi chạy từ Finder, không lộ khi dev
chạy từ terminal. Ưu tiên decode in-process bằng lib đã bundle.
