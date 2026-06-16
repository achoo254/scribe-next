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

## Sidecar

- `src-tauri/binaries/ffmpeg-<target>` + `ffprobe-<target>` — bản **ffmpeg LGPL** (BtbN). Dùng cho ghi/cắt audio + **ghi màn hình** (`ddagrab` + `libopenh264`, đều có sẵn trong bản LGPL — không cần libx264/GPL).
- `transcriber-<target>` — Python sidecar faster-whisper (PyInstaller onefile, CPU int8). Build: `sidecar/transcriber/build_pyinstaller.ps1`.
  - **Picker đa-model**: small / medium / large (mặc định **medium**) — chọn theo nhu cầu tốc độ ↔ chất lượng. Mỗi size tải riêng (~0.5 / ~1.5 / ~3 GB) về app-data lần đầu, sau đó offline. small/medium từ `quocphu/PhoWhisper-ct2-FasterWhisper`, large từ `kiendt/PhoWhisper-large-ct2`.

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
