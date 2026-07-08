# Report — Cook triển khai cải tiến Transcript

> 2026-07-08 · Mac mini M4 arm64, macOS 26.5.1 · branch `main` · **chưa commit**
> Plan: `plans/260708-0859-transcript-improvements/`

## Kết quả

4 phase xong, verify sạch, code review + fix. Không commit (chờ user).

### Thay đổi (theo file)

**Rust (`src-tauri/`)**
- `settings.rs` — field `glossary: Option<String>` (default = prompt DevOps cũ, giữ trải nghiệm).
- `transcribe.rs` — `transcribe()` nạp `settings.glossary` → chèn `prompt` vào JSON sidecar; thêm `ModelInfo` + `list_models()` + `delete_model()`.
- `commands.rs` — command `save_transcript`, `list_models`, `delete_model`.
- `lib.rs` — đăng ký 3 command.
- `Cargo.toml` — feature tauri `protocol-asset`.
- `tauri.conf.json` — `app.security.assetProtocol {enable, scope:["**"]}`.

**Sidecar (`sidecar/transcriber/main.py`)**
- `word_timestamps=True` cả ct2 + mlx; chuẩn hoá segment `{start,end,text,words[]}`.
- `resegment_cues` (≤42 ký tự/dòng, ≤2 dòng cân bằng, ≤6s/cue) + `to_srt/to_vtt/to_json`.
- `transcribe()` nhận `prompt`; `done` payload thêm `segments/vtt_path/json_path`.

**Frontend (`src/`)**
- `composables/settings-store.ts` (MỚI) — `loadSettings` + `patchSettings` (merge, chống clobber).
- `composables/use-glossary.ts` (MỚI) — load/save glossary qua settings-store.
- `composables/use-transcribe.ts` — rewrite: results per-file (segments + text sửa + save + dirty/saved), `exportFormats`, `retry`, `edit`, upsert/dedup.
- `composables/asr-models.ts` — `useModelManager` + `formatBytes`.
- `components/TranscriptPlayer.vue` (MỚI) — player audio/video + segment list click-to-seek.
- `views/TranscriptView.vue` — glossary textarea, export toggle, per-file player+editor, retry, model management.
- `views/RecordView.vue` — dùng `patchSettings` (fix clobber glossary).
- `plugins/tabler-iconset.ts` — +IconRefresh/IconDeviceFloppy/IconTrash.

### Quyết định tự chốt (Mac mini M4)
- word_timestamps bật mặc định (máy đủ nhanh, bỏ toggle — KISS).
- asset scope `["**"]` read-only + `save_transcript` command riêng.
- default model giữ `medium`.
- **Phase 04: chỉ retry + model management. GIỮ hard-kill cancel, KHÔNG refactor main-loop sidecar** — bảo vệ teardown vừa fix ở `688b395`/`a3c2614`. Đánh đổi có chủ đích (rủi ro cao / giá trị thấp trên máy nhanh).

### Verify (bằng chứng)
- `cargo build`: Finished dev profile, 0 error.
- `vue-tsc --noEmit`: exit 0, 0 dòng lỗi.
- `vite build`: exit 0, 6502 modules.
- Sidecar smoke (file wav TTS tiếng Việt, model medium ct2): `ready→info→progress→done`, xuất đủ txt/srt/vtt/json, exit 0. SRT sau resegment cân bằng: dòng dài nhất 38 ký tự (≤42), 2 cue ≤6s. JSON hợp lệ có `words[]`.

### Code review (code-reviewer) + xử lý
- **H1** clobber glossary (RecordView ghi settings thiếu field → serde nhồi default): FIX bằng `patchSettings` merge (cả RecordView + use-glossary).
- **M1** retry mất file khi lỗi chung: FIX — giữ entry lỗi tới khi có kết quả; done xoá, lỗi-file thay.
- **M3** dòng SRT thứ 2 không giới hạn: FIX — cắt cân bằng (cả 2 dòng ≤max_chars).
- **L6** mlx word `[]`→`.get` + lọc word thiếu mốc; **L7** bỏ cast thừa; **L3** sửa comment save_transcript.
- **Accept/known**: M2 (cancel hard-kill app-wide — descope có chủ đích), L2 (glossary save nuốt lỗi — rare), L4 (asset scope `**` — threat model local OK), L5 (Mode A info/error cross-talk — cosmetic, pre-existing).

## Chưa verify được trong session (cần user kiểm bằng mắt)
- **Player phát file local qua asset protocol** trong GUI thật (`convertFileSrc`): config Tauri v2 đúng + build sạch, nhưng playback trong webview chưa chạy GUI để xác nhận. → chạy `npm run tauri dev`, vào tab Transcript, transcribe 1 file, bấm play + click segment.
- Xóa model / retry / lưu-lại trên GUI (logic đã build sạch, chưa click thật).

## Unresolved questions
- Mode A (auto-transcript khi ghi) chạy ĐỒNG THỜI Mode C batch có phải workflow cần hỗ trợ thật không? Nếu có → M2 (cancel app-wide) + L5 (cross-talk) nên fix thật thay vì document. Nếu 2 mode gần như loại trừ nhau khi dùng → giữ nguyên.
