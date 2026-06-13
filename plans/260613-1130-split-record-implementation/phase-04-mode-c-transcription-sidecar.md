---
phase: 4
title: "Mode C - Transcription sidecar"
status: implemented
priority: P2
effort: "4-5d"
dependencies: [1]
implemented: "2026-06-13"
note: "Code + build + sidecar smoke (ready/shutdown) + code-review (fix H1 batch-error, H2 partial-model, M1 pct, M2 stderr) + ViUI validate xong. faster-whisper verify import trên Py3.14. HOÃN: PyInstaller build (P5), download model 1.5GB + transcribe thật (cần network + audio tiếng Việt)."
---

# Phase 4: Mode C - Transcription sidecar

## Overview

Transcribe offline tiếng Việt bằng **faster-whisper** chạy trong **Python sidecar persistent** (đóng gói PyInstaller, không cần Python hệ thống), dùng model `kiendt/PhoWhisper-large-ct2` (int8 CPU). Trigger thủ công + batch; xuất `.txt` + `.srt` + hiển thị transcript trong app. Đây là build-block 2.

<!-- Updated: Validation Session 1 - v1 chỉ PhoWhisper-large (picker model nhỏ hơn → v2); transcript lưu CẠNH FILE GỐC (đổi được trong settings sau) -->
## Requirements

- Functional: chọn 1+ file; tự tải model lần đầu (progress) rồi offline; nút Transcribe (đơn + batch); progress per-file; panel hiển thị transcript (copy được); lưu `.txt` + `.srt` **cạnh file gốc** (mặc định).
- v1 chỉ dùng **PhoWhisper-large** (không có model picker — để v2).
- Non-functional: model load **một lần** (giữ ấm) cho batch nhanh; không block UI; chạy offline sau khi tải; RAM ~1.5–2GB chấp nhận.

## Architecture

```
Rust (transcribe.rs) --spawn 1 lần (lazy)--> Python sidecar (transcriber-<target>)
   │  JSON over stdio (line-delimited)        │ load PhoWhisper-large-ct2 int8 MỘT LẦN
   ├─ {"cmd":"ensure_model"}                  │ huggingface_hub.snapshot_download → app-data
   │     ◄─ {"type":"download", pct}          │ (sau đó local_files_only=True)
   ├─ {"cmd":"transcribe", file, out_dir, outputs:["txt","srt"]}
   │     ◄─ {"type":"progress", file, pct}    │ faster-whisper segments (PyAV decode trực tiếp)
   │     ◄─ {"type":"done", file, text, txt_path, srt_path}
   └─ (đóng app → kill sidecar)
UI TranscriptView.vue (Vi*): model status + Transcribe + progress + panel text + save .txt/.srt
```

- **Python sidecar** (`sidecar/transcriber/main.py`): vòng lặp đọc stdin JSON; `WhisperModel(model_dir, device="cpu", compute_type="int8")`; lệnh `ensure_model` (download + progress), `transcribe` (stream progress theo `segment.end / duration`, build `.srt` từ timestamps, ghi `.txt`, trả full text). PyInstaller one-dir → `binaries/transcriber-<target>`.
- **transcribe.rs**: quản lý vòng đời sidecar (spawn lazy, giữ handle, kill khi exit); gửi lệnh, đọc JSON, emit Tauri events; hàng đợi batch tuần tự (model đã ấm).
- **Model storage**: `app_data_dir()/models/PhoWhisper-large-ct2/`. Kiểm tra tồn tại trước khi transcribe; nếu thiếu → `ensure_model`.
- **Output**: `.txt` (text thuần) + `.srt` (timestamp theo segment) cạnh file gốc; full text trả về UI để hiển thị panel.

## Related Code Files

- Create: `sidecar/transcriber/main.py` (faster-whisper loop + JSON protocol + srt builder), `sidecar/transcriber/requirements.txt`, `sidecar/transcriber/build_pyinstaller.sh` / `.ps1`
- Create: `src-tauri/src/transcribe.rs` (sidecar lifecycle + protocol + model orchestration)
- Create: `src-tauri/binaries/transcriber-x86_64-pc-windows-msvc(.exe)` (output PyInstaller)
- Modify: `src-tauri/src/commands.rs` (`ensure_model`, `transcribe_files`, `cancel_transcribe`), `src-tauri/tauri.conf.json` (externalBin thêm transcriber), `src-tauri/src/main.rs`
- Modify: `src/views/TranscriptView.vue` (UI ViUI), `src/composables/use-transcribe.ts` (wrap invoke + listen events), `src/styles.css`

## Implementation Steps

1. `main.py`: protocol JSON line-delimited; load model lazy; `transcribe()` dùng `model.transcribe(path, beam_size=5, language="vi")`, lặp segments → progress + gom text + dòng srt; ghi file; trả `done`.
2. `ensure_model`: `snapshot_download("kiendt/PhoWhisper-large-ct2", local_dir=...)` với callback progress (hoặc tqdm parse) → emit `download` pct.
3. `requirements.txt`: `faster-whisper`, `ctranslate2`, `huggingface_hub`, `av` (PyAV). PyInstaller spec gom CTranslate2 native libs.
4. Build script PyInstaller → one-dir, rename theo target → `binaries/transcriber-<target>`.
5. `transcribe.rs`: spawn sidecar (tauri shell), serde structs cho request/response, đọc stdout theo dòng → emit events; batch queue; kill on drop.
6. `commands.rs`: `ensure_model`, `transcribe_files(paths[], out_dir, outputs[])`, `cancel`.
7. `TranscriptView.vue` (**UI bắt buộc ViUI — chỉ Vi*, token-only, WCAG AA**): multi-file picker = `ViButton` + `ViList`; trạng thái model = `ViAlert`/`ViChip` (chưa tải → `ViButton` Tải kèm progress); nút Transcribe = `ViButton` (primary); progress per-file = thanh progress (Vi* progress/wrapper); panel transcript = `ViTextarea` (readonly) + `ViButton` Copy (`ViIcon` Tabler copy); nút lưu .txt/.srt = `ViButton` + link mở (`ViButton`/`ViIcon`). Tra component/token qua ViUI MCP.
8. Test: tải model 1 lần; transcribe 1 file mp3 tiếng Việt → text hợp lý + srt timestamp đúng; batch 2 file (model không reload); ngắt mạng sau khi tải → vẫn chạy (offline).
9. **Verify design system**: ViUI MCP `validate_brand_usage` + `review_design_tokens` trên `TranscriptView.vue` → 0 vi phạm.

## Todo List

- [x] main.py: JSON protocol + faster-whisper transcribe + srt builder + BOM-safe
- [x] ensure_model download + progress (JsonTqdm) + model giữ ấm (load 1 lần)
- [x] requirements.txt (pin version) + PyInstaller build script (onefile — onedir cân nhắc P5)
- [x] transcribe.rs lifecycle (std::process persistent + spawn lazy + reader thread + Drop/Exit kill)
- [x] commands ensure_model/transcribe_files/cancel/model_status
- [x] TranscriptView.vue UI (Vi* + token + WCAG): model status, transcribe, progress, panel copy, mở .txt/.srt
- [ ] **test offline + batch + chất lượng tiếng Việt** (cần download model 1.5GB + audio thật) — HOÃN
- [x] ViUI MCP `validate_brand_usage` → 0 vi phạm (TranscriptView). *`review_design_tokens` không chạy được (MCP sandbox).*
- [x] sidecar smoke: ready + shutdown sạch (exit 0); faster-whisper import OK trên Python 3.14

## Success Criteria

- [ ] Tải model lần đầu có progress, sau đó chạy offline
- [ ] Transcribe 1 file → `.txt` + `.srt` đúng nội dung tiếng Việt + hiển thị in-app
- [ ] Batch nhiều file: model load 1 lần, xử lý tuần tự, progress đúng
- [ ] Đóng app → sidecar bị kill (không treo process)

## Risk Assessment

- PyInstaller gom CTranslate2 native libs thiếu → sidecar crash khi load. Mitigation: test one-dir, thêm hidden-imports/binaries vào spec; verify trên máy sạch.
- Model 1.5GB tải lâu/đứt giữa chừng. Mitigation: `snapshot_download` resume; verify file đủ trước khi `local_files_only`.
- Tốc độ large trên CPU yếu chậm hơn realtime. Mitigation: cảnh báo UI cho file dài; (v2) option model nhỏ hơn.
- macOS: notarize binary sidecar + CPU-only (không Metal). Xử lý ở P5.

## Next Steps

Chuyển sang P5 đóng gói: thêm transcriber sidecar macOS + ký/notarize + bundle config.
