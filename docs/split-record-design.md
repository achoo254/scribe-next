# OneScribe — Design Document

> **OneScribe** (One Series iNET · bundle `vn.inet.onescribe`) — Desktop app (Windows + macOS) ghi âm micro, cắt file theo nhiều tiêu chí, transcribe offline tiếng Việt.
> Brainstorm summary — 2026-06-13. Status: APPROVED → ready for `/ck:plan`. *(Tên cũ "Split-Record"; UI/naming/theme cập nhật Validation Session 2+3.)*

## 1. Problem statement

Cần desktop app **đơn giản** để: (1) ghi âm micro và tự cắt thành đoạn theo thời lượng/dung lượng ngay khi ghi (split live); (2) cắt file audio đã có sẵn thành nhiều đoạn; (3) transcribe audio sang text offline bằng model AI tiếng Việt.

## 2. Locked requirements

| # | Quyết định | Nguồn |
|---|---|---|
| R1 | Stack: **Tauri v2** (Rust backend + web UI **Vue 3/Vite + iNET ViUI design system**) | user *(UI stack cập nhật Validation Session 2 — bắt buộc ViUI)* |
| R2 | Platform: **Windows + macOS** | user |
| R3 | Mode A: ghi **audio micro**, output nén **MP3/M4A**, **split LIVE** mỗi N phút | user |
| R4 | Mode A features: chọn mic + thư mục, **pause/resume**, **level meter**, danh sách file gần đây | user |
| R5 | Mode B: **cắt file sẵn có** theo **phút / MB / chia-K đều** + **batch** nhiều file | user |
| R6 | Mode C: **transcribe offline** model `kiendt/PhoWhisper-large-ct2`, runtime **faster-whisper (Python sidecar)** | user |
| R7 | Mode C: trigger **thủ công + batch**; output **.txt + .srt + hiển thị in-app** | user |
| R8 | Build **phân phase**: P1 = record+split, P2 = transcribe | user |

## 3. Architecture

### 3.1 Mode A — Ghi âm + split live (hybrid: cpal → ffmpeg stdin)

```
[Mic] --cpal--> PCM buffer (Rust)
                 ├── RMS/peak ──emit──► level meter (UI)
                 ├── pause = ngừng đẩy buffer vào stdin
                 ▼
        stdin pipe (s16le)
                 ▼
  ffmpeg sidecar (encode-only):
    -f s16le -ar 48000 -ac 1 -i -
    -c:a aac|libmp3lame
    -f segment -segment_time <giây> -reset_timestamps 1
                 ▼
  rec_YYYYMMDD_HHMMSS_%03d.<m4a|mp3>
```

Vì sao hybrid: cpal lo device-enum sạch (một code path Win/Mac) + level meter (RMS) + pause (ngừng feed); ffmpeg segment muxer lo split-live + nén native. Input ffmpeg luôn là PCM qua stdin → **giống hệt trên Win & Mac**, không dính device-naming dshow/avfoundation.

### 3.2 Mode B — Cắt file sẵn có (ffmpeg `-c copy`)

```
ffmpeg -i <input> -f segment -segment_time <giây> -reset_timestamps 1 -c copy <out>_%03d.<ext>
```

Lossless, nhanh, giữ codec gốc. **3 tiêu chí quy về 1 `segment_time` (giây):**
- Phút → `N×60`
- MB → `MB×8 / bitrate` (bitrate từ ffprobe; CBR sát, VBR ước lượng ±)
- Chia-K → `duration / K` (ffprobe; chỉ Mode B)

**Batch**: chọn nhiều file → cùng tiêu chí → ffmpeg chạy **lần lượt**; output mỗi file gốc vào subfolder riêng `<tên-gốc>/<tên-gốc>_001.<ext>` (tránh trùng tên); progress per-file + tổng.

### 3.3 Mode C — Transcribe (Python sidecar persistent)

```
Rust backend ──spawn 1 lần (lazy)──► Python sidecar (PyInstaller, không cần Python hệ thống)
   │                                    │ load PhoWhisper-large-ct2 (int8 CPU) MỘT LẦN, giữ ấm
   │  JSON over stdio                   │ faster-whisper đọc thẳng mp3/m4a/wav (PyAV)
   ├─ {"cmd":"transcribe", file, outputs, out_dir} ─►
   ◄─ {progress %} … {done, text, txt_path, srt_path} ─┘
   ▼
UI: progress + panel transcript + lưu .txt/.srt cạnh file gốc
```

- **Persistent sidecar**: model load chậm + ~1.5–2GB RAM → load 1 lần, xử lý hàng đợi, kill khi đóng app → batch nhanh.
- **Model download lần đầu**: `huggingface_hub.snapshot_download("kiendt/PhoWhisper-large-ct2")` (~1.5GB) về app-data + progress; sau đó `local_files_only=True` → offline hoàn toàn.
- **Đóng gói**: PyInstaller per-OS → `binaries/transcriber-<target>`, khai báo Tauri `externalBin`; macOS cần sign/notarize.
- faster-whisper dùng CTranslate2 (không PyTorch) → sidecar nhẹ hơn whisper gốc nhiều.

## 4. Approaches evaluated

**Audio engine (Mode A):**
- ✅ **A. Hybrid cpal → ffmpeg stdin** (chọn): thỏa hết pause/meter/nén/split-live, một code path cross-platform.
- ❌ B. FFmpeg ghi thẳng device: ít code nhưng pause/meter khó + device-naming theo OS.
- ❌ C. Thuần Rust (cpal + encoder crate + segmenter tự viết): bundle nhỏ nhưng phải tự viết encode+split, nhiều bug-surface. *(Càng bị loại khi thêm Mode B/C vốn cần ffmpeg.)*

**ASR runtime (Mode C):**
- **faster-whisper Python sidecar** (user chọn): chắc chắn chạy, khớp sample model card, nhiều tính năng ASR sẵn. Giá: installer phình, đóng gói cross-platform công hơn.
- ct2rs (Rust native): gọn hơn, không Python, nhưng build CTranslate2 (C++) rủi ro — *không chọn*.

## 5. UI

3 tab: **Ghi âm** / **Cắt file** / **Transcript**. 1 cửa sổ, **Vue 3 + Vite + Vuetify (public)** (BẮT BUỘC theo iNET ViUI). UI **chỉ dùng wrapper `Vi*` tự dựng** (theo spec ViUI MCP; cấm raw Vuetify trong view), **token-only** (token vendor từ MCP, không hex cứng), **WCAG 2.1 AA**, icon Tabler. Packages **public**: `vue` + `vuetify@^3.5` + `@tabler/icons-vue` (+ dev `@vitejs/plugin-vue`, `vite-plugin-vuetify`). **KHÔNG cài `@hobui/*`** (private/không có trên npm — token sinh từ MCP `generate_color_vars`). *(UI stack cập nhật Validation Session 2 — đổi từ vanilla TS; xem `plan.md` → "Design system — iNET ViUI".)*
- Ghi âm: dropdown mic, chọn folder, ô N phút/MB, chọn format M4A/MP3, Record/Pause/Stop, đồng hồ + số segment, level meter, danh sách gần đây.
- Cắt file: chọn 1+ file, radio [Phút|MB|Chia-K], ô giá trị, output folder, Cắt + progress, danh sách kết quả.
- Transcript: chọn 1+ file, trạng thái model (download nếu cần), Transcribe + progress, panel transcript (copy), nút lưu .txt/.srt.

**Theme**: **One Series dark `inetOneDark`** (dark-first) — primary green `#49c98d` (Record/CTA), accent orange `#f7ad30`, nền `#0d0d0d`/card `#1a1a1a`, text `#e0e0e0`. Green/orange chỉ dùng trên nền tối (cảnh báo WCAG từ token: FAIL 3:1 trên nền trắng). *(Validation Session 3.)*

## 6. Project structure (mỗi file < 200 dòng)

```
src/                       # UI Vue 3 + Vuetify + ViUI tokens
  main.ts                  # import assets/styles/viui-tokens.css → createApp().use(vuetify).mount
  App.vue                  # shell 3 tab (Vi* tab switcher + <component :is>)
  plugins/vuetify.ts       # createVuetify theme iNET (màu từ token vendor) + Tabler iconset
  assets/styles/viui-tokens.css  # token VENDOR từ MCP generate_color_vars (--viui-color-*)
  tokens/                  # spacing 8pt / typography / radius vendor từ MCP lookup_token
  design-system/vi-ui/vi-*/Vi*.vue  # wrapper Vi* TỰ DỰNG (mỏng quanh Vuetify, theo spec MCP)
  views/{RecordView,SplitView,TranscriptView}.vue   # 3 tab — chỉ Vi* + token
  components/LevelMeter.vue # level meter custom (token-styled; không có Vi* sẵn)
  composables/use-{recorder,splitter,transcribe}.ts # wrap invoke + listen events
  styles.css               # global, chỉ token CSS vars (không hex, không !important)
src-tauri/
  src/
    main.rs                # setup + đăng ký commands
    commands.rs            # list_devices, start/pause/resume/stop, pick_folder, probe_file, split_files, transcribe
    recorder.rs            # state machine: Idle → Recording ⇄ Paused → Idle
    audio/capture.rs       # cpal stream + device enum
    audio/meter.rs         # RMS/peak
    audio/encoder.rs       # ffmpeg sidecar pipe (live encode + segment)
    splitter.rs            # Mode B: batch + 3 tiêu chí → segment_time (tách criterion.rs nếu >200 dòng)
    transcribe.rs          # Mode C: vòng đời python sidecar + JSON protocol + orchestrate model download
    settings.rs            # persist config (mic, folder, format, N)
  binaries/                # ffmpeg-<target>, transcriber-<target>
  tauri.conf.json
sidecar/transcriber/       # Python source faster-whisper (build PyInstaller)
docs/
```

## 7. Defaults

Split N phút mặc định **10**; format ghi live mặc định **MP3 mono ~128kbps 48kHz** (crash-safe; M4A-AAC là option, dùng `+frag_keyframe+empty_moov`); naming live `rec_YYYYMMDD_HHMMSS_%03d`; ASR model **PhoWhisper-large int8 CPU** (v1 chỉ large); transcript lưu **cạnh file gốc**. *(Cập nhật theo Validation Session 1, 2026-06-13.)*

## 8. Risks & mitigations

| Risk | Mitigation |
|---|---|
| Crash giữa segment M4A (moov atom ở cuối → file dở hỏng) | `-movflags +frag_keyframe+empty_moov` (fragmented mp4); hoặc MP3 (stream-friendly, miễn nhiễm) |
| Mic ngắt giữa chừng | cpal error callback → finalize segment hiện tại + báo UI |
| `-c copy` cắt ở biên frame (~vài chục ms) | Chấp nhận cho audio; re-encode chính xác để v2 |
| MB trên file VBR = ước lượng | Note trong UI; CBR/live thì sát |
| Model 1.5GB + sidecar lớn → app không "nhẹ" | Download-on-first-use (không nhúng installer); chấp nhận tradeoff |
| Tốc độ large CPU (~0.5–2× realtime), RAM ~1.5–2GB | Persistent sidecar (load 1 lần); cảnh báo file dài; option model nhỏ hơn (v2) |
| macOS: quyền mic + notarize sidecar | `NSMicrophoneUsageDescription` (Info.plist); sign/notarize ffmpeg + transcriber |
| macOS CT2 = CPU-only (không Metal) | Chấp nhận; Metal/whisper.cpp để cân nhắc v2 |

## 9. Success criteria

1. Chọn mic+folder, set N, Record → các file ~N phút liên tiếp, mỗi file phát được.
2. Pause/Resume không tạo file lỗi, không mất tiếng.
3. Level meter nhúc nhích đúng theo tiếng vào.
4. Stop → segment cuối hợp lệ + hiện trong danh sách.
5. Cắt file theo phút/MB/chia-K ra đúng số đoạn kỳ vọng, lossless, phát được.
6. Batch nhiều file: mỗi file ra bộ đoạn riêng, không trùng tên, progress đúng.
7. Transcribe (thủ công + batch) ra .txt + .srt đúng nội dung tiếng Việt, hiển thị in-app, chạy offline sau khi tải model.
8. Build & chạy được trên cả Windows và macOS.

## 10. Build phasing

Thứ tự BUILD (chỉ phát hành 1 lần ở cuối):
- **Build-block 1** — Mode A + B (recorder + splitter): làm trước, rủi ro thấp.
- **Build-block 2** — Mode C (transcription Python sidecar): làm sau, phần nặng + rủi ro đóng gói.
- **Release** — **một installer đầy đủ** cả 2 build-block (không phát hành sớm build-block 1 riêng — Validation Session 1).

## 11. Out of scope (v1)

Audio hệ thống/loopback; video/màn hình/webcam; silence-detect cắt theo khoảng lặng; merge/ghép file; re-encode cắt chính xác sample; auto-transcribe sau ghi; GPU acceleration (mặc định CPU int8); ngôn ngữ ngoài tiếng Việt.

## 12. Open questions — RESOLVED (Validation Session 1, 2026-06-13)

- ~~Model option nhỏ hơn cho máy yếu?~~ → v1 chỉ PhoWhisper-large; model picker (medium) để v2.
- ~~Output folder transcript?~~ → **cạnh file gốc** (đổi được trong settings sau).
- ~~Bitrate/sample-rate cho chỉnh ở v1?~~ → **cố định mặc định** v1.
- ~~Format ghi live default?~~ → **MP3** (crash-safe); M4A là option.
- ~~Release sớm build-block 1?~~ → Không; **1 installer đầy đủ**.

Không còn open question.
