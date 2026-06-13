# OneScribe

Desktop app (Windows + macOS) ghi âm micro + cắt live, cắt file audio sẵn có theo nhiều tiêu chí, và transcribe offline tiếng Việt (PhoWhisper). Một sản phẩm **One Series — iNET**.

- **Stack**: Tauri v2 (Rust) + Vue 3 + Vite + Vuetify 3.5 + iNET ViUI design system (theme One Series dark `inetOneDark`).
- **Bundle id**: `vn.inet.onescribe`
- 3 chế độ: **Ghi âm** (Mode A) · **Cắt file** (Mode B) · **Transcript** (Mode C).

## Yêu cầu môi trường (Windows)

- **Node.js** ≥ 18 + npm
- **Rust** (rustup, toolchain `stable-x86_64-pc-windows-msvc`)
- **MSVC C++ Build Tools** (Visual Studio Build Tools — workload "Desktop development with C++") + Windows SDK
- **WebView2 runtime** (Windows 11 đã có sẵn)

## Sidecar

- `src-tauri/binaries/ffmpeg-<target>` + `ffprobe-<target>` — bản **ffmpeg LGPL** (BtbN). Dùng cho ghi/cắt audio.
- (Phase 4) `transcriber-<target>` — Python sidecar faster-whisper (PyInstaller).

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
  design-system/vi-ui/    # wrapper Vi* (Button/Select/Input/Card/Alert/Icon/Tabs/Radio)
  views/                  # RecordView / SplitView / TranscriptView
src-tauri/
  src/{lib,commands,ffmpeg,settings}.rs
  binaries/               # ffmpeg, ffprobe (+ transcriber ở P4)
docs/ plans/              # thiết kế + kế hoạch triển khai
```
