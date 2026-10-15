# OneScribe

Desktop app (Windows + macOS) ghi âm micro + cắt live, cắt file audio/video sẵn có theo nhiều tiêu chí, và transcribe offline tiếng Việt (PhoWhisper) cho cả file audio lẫn video. Một sản phẩm **One Series — iNET**.

- **Stack**: Tauri v2 (Rust) + Vue 3 + Vite + Vuetify 3.5 + iNET ViUI design system (theme One Series dark `inetOneDark`).
- **Bundle id**: `vn.inet.onescribe`
- 3 chế độ: **Ghi âm** (Mode A) · **Cắt file** (Mode B) · **Transcript** (Mode C).
- **Tự động transcript khi ghi** (tùy chọn, Mode A): bật toggle → mỗi đoạn ghi vừa xong tự transcribe ngay (real-time), hiển thị transcript inline dưới từng đoạn + lưu `.txt`/`.srt` cạnh đoạn. Dùng lại sidecar PhoWhisper (model giữ ấm) của Mode C.
- **Nguồn ghi** (Mode A): chọn **Mic** (cpal) hoặc **Tiếng hệ thống** (âm thanh đang phát ra loa/tai nghe — WASAPI loopback). Ghi tiếng hệ thống hiện **chỉ Windows** (macOS để sau). Crate `wasapi` (Windows-only, target `cfg(windows)`).
- **Ghi kèm màn hình → MP4** (tùy chọn, Mode A, **chỉ Windows**): bật toggle "Ghi kèm màn hình" → xuất MP4 (video + audio nguồn đã chọn). **Toàn màn hình** hoặc **chọn vùng** kiểu Bandicam (overlay trong suốt kéo chuột, xử lý DPI scaling). Vùng đã chọn hiện viền đỏ kèm thanh công cụ nhỏ phía trên (tay nắm ⠿ **kéo để di chuyển** vùng + nút ✕ đóng khung); chỉ di chuyển được trước khi ghi (idle), vùng quay cố định khi đã ghi. Video bắt bằng `ddagrab` (DXGI Desktop Duplication, GPU), encode `libopenh264` (software, mọi máy), audio giữ pipeline mic/loopback sẵn có; cắt đoạn theo phút (frag-MP4, crash-safe). Auto-transcript chạy trên audio track của MP4 như thường. **Lưu ý đa màn hình**: index màn hình map trực tiếp sang `ddagrab output_idx` — đúng với 1 màn hình; máy ≥2 màn hình cần kiểm tra thực tế. Pause không áp dụng khi ghi màn hình (chỉ Ghi/Dừng).

## Yêu cầu môi trường (Windows)

- **Node.js** ≥ 18 + npm
- **Rust** (rustup, toolchain `stable-x86_64-pc-windows-msvc`)
- **MSVC C++ Build Tools** (Visual Studio Build Tools — workload "Desktop development with C++") + Windows SDK
- **WebView2 runtime** (Windows 11 đã có sẵn)

## Yêu cầu môi trường (macOS)

- **Node.js** ≥ 18 + npm
- **Rust** (rustup, target `aarch64-apple-darwin` cho Apple Silicon / `x86_64-apple-darwin` cho Intel)
- **Xcode Command Line Tools** (`xcode-select --install`)
- **Homebrew** để cài ffmpeg: `brew install ffmpeg`
- Overlay chọn vùng ghi dùng `WebviewWindowBuilder::transparent` → cần feature `macos-private-api`
  (đã bật sẵn trong `Cargo.toml` + `app.macOSPrivateApi` trong `tauri.conf.json`; vô hại với Windows).
- `tauri.macos.conf.json` override bundle target sang `dmg`/`app` (base là `nsis` cho Windows).
- **Chỉ Windows** (compile được nhưng không chạy trên Mac): ghi tiếng hệ thống (WASAPI loopback) + ghi màn hình (`ddagrab`).

## Sidecar

- `src-tauri/binaries/ffmpeg-<target>` + `ffprobe-<target>` — bản **ffmpeg LGPL** (BtbN). Dùng cho ghi/cắt audio + **ghi màn hình** (`ddagrab` + `libopenh264`, đều có sẵn trong bản LGPL — không cần libx264/GPL).
- `transcriber-<target>` — Python sidecar faster-whisper (PyInstaller onefile). Mặc định **CPU int8** (mọi máy, offline); **GPU NVIDIA (CUDA float16)** khi bật toggle (xem dưới). Build: Windows `sidecar/transcriber/build_pyinstaller.ps1`; macOS/Linux `bash sidecar/transcriber/build_pyinstaller.sh` (đặt `PYTHON=python3.x` để ép version). Trên macOS, ffmpeg/ffprobe copy từ brew vào `src-tauri/binaries/ffmpeg-<target>` + `ffprobe-<target>`.
  - **Picker đa-model**: small / medium / large / **large-v3-turbo** (mặc định **medium**; CPU) — chọn theo tốc độ ↔ chất lượng. Mỗi size tải riêng (~0.5 / ~1.5 / ~3 / ~1.6 GB) về app-data lần đầu, sau đó offline. small/medium từ `quocphu/PhoWhisper-ct2-FasterWhisper`, large từ `kiendt/PhoWhisper-large-ct2`, turbo từ `deepdml/faster-whisper-large-v3-turbo-ct2` (hợp nội dung Việt-Anh code-switch). Trên macOS Apple Silicon còn có **Whisper Large-v3 (MLX)** chạy Metal GPU.
  - **GPU NVIDIA (CUDA, chỉ Windows)**: máy có GPU NVIDIA → hiện toggle "Dùng GPU (CUDA)" (Mode C, áp dụng chung Mode A). Bật lần đầu → tải on-demand **cuBLAS (~553MB tải / ~771MB đĩa)** thẳng từ PyPI về app-data (KHÔNG nhúng vào installer). cuDNN KHÔNG cần — verified qua EnumProcessModules: CTranslate2 Whisper chỉ nạp cublas64_12 + cublasLt64_12. Bắt buộc `compute_type=float16` (RTX 50-series/Blackwell crash với int8). Lỗi/không có GPU → tự fallback CPU int8. Verified RTX 5070 Ti: PhoWhisper-large ~73× realtime (GPU) vs ~5× (CPU) — nhanh ~14.5×.
  - **Transcript (Mode C) — last mile**: (1) **Glossary sửa được trong app** (thuật ngữ/tên riêng → bias model, mặc định domain DevOps, dùng chung Mode A) thay cho prompt hardcode cũ; (2) **xuất `.txt` / `.srt` / `.vtt` / `.json`** — SRT/VTT resegment chuẩn phụ đề (≤42 ký tự/dòng, ≤2 dòng, ≤6s/cue) nhờ `word_timestamps`; (3) **player nghe lại đồng bộ** (click segment → nhảy tới mốc) + **sửa transcript & Lưu lại** ghi đè `.txt`; batch xem/sửa từng file + **thử lại** file lỗi; (4) **quản lý/xóa model** để lấy lại disk. Phát file local dùng Tauri asset protocol (`convertFileSrc`).
  - **Chống lặp khi chép offline**: ct2 decode tuần tự (không `BatchedInferencePipeline`) + `condition_on_previous_text=False` + `hallucination_silence_threshold=2.0` (cả MLX). Batched ép temperature 0 không fallback nên dễ kẹt vòng lặp ("vâng, vâng…"). Glossary > ~280 ký tự hiện cảnh báo (prompt dài làm mất chữ).

## Design system

UI **bắt buộc** theo iNET ViUI: chỉ dùng wrapper `Vi*` (`src/design-system/vi-ui/`), token-only (token vendor từ ViUI MCP → `src/assets/styles/viui-tokens.css`), WCAG 2.1 AA, icon Tabler. Không hardcode màu/spacing; không `!important`.

## Lệnh

```bash
npm install            # cài deps
npm run tauri dev      # chạy app (dev)
npm run tauri build    # đóng gói installer
npm run dev            # chỉ frontend (Vite, không cần Rust)
```

## Cấu trúc

```
src/                      # UI Vue 3 + ViUI
  main.ts                 # import token css + vuetify, mount
  App.vue                 # shell 3 tab
  plugins/vuetify.ts      # theme inetOneDark + Tabler iconset
  assets/styles/viui-tokens.css   # token vendor từ MCP
  design-system/vi-ui/    # wrapper Vi* (Button/Select/Input/Card/Alert/Icon/Tabs/Radio/Switch)
  views/                  # RecordView / SplitView / TranscriptView / RegionSelectorOverlay (overlay chọn vùng)
src-tauri/
  src/{lib,commands,ffmpeg,settings,screen}.rs   # screen.rs = monitor + overlay chọn vùng ghi màn hình
  src/audio/encoder.rs    # ffmpeg_args (audio) + ffmpeg_args_screen (ddagrab → MP4)
  capabilities/           # default.json (main) + region-selector.json (overlay)
  binaries/               # ffmpeg, ffprobe, transcriber
docs/ plans/              # thiết kế + kế hoạch triển khai
```

## Tài liệu

- [docs/codebase-summary.md](./docs/codebase-summary.md) — tổng quan kiến trúc (desktop + web), module BE/FE, API, MongoDB, storage
- [docs/project-roadmap.md](./docs/project-roadmap.md) — lộ trình + trạng thái phase (OneScribe Web 5/5 DONE) + backlog
- [docs/deployment-guide.md](./docs/deployment-guide.md) — build musl + deploy .65 (nginx/systemd/CF)
- [docs/split-record-design.md](./docs/split-record-design.md) — thiết kế cắt/ghi media
