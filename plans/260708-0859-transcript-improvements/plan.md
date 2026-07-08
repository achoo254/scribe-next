# Kế hoạch — Cải tiến tính năng Transcript (Mode C + auto-transcript Mode A)

> Trạng thái: **DONE (đã triển khai + verify, chưa commit)** · 2026-07-08 · Branch nền: `main`
>
> **Kết quả triển khai** (Mac mini M4): cả 4 phase xong. Quyết định tự chốt cho máy này:
> word_timestamps bật mặc định (ct2+mlx); asset scope `["**"]` read-only + command `save_transcript`;
> default model giữ `medium`; **Phase 04 chỉ làm retry + model management, GIỮ hard-kill cancel
> (KHÔNG refactor main-loop sidecar — bảo vệ teardown vừa fix ở 688b395/a3c2614)**.
>
> Verify: `cargo build` sạch · `vue-tsc --noEmit` sạch · `vite build` sạch (6502 modules) · sidecar
> chạy thật 1 file wav VN → txt/srt/vtt/json OK, SRT resegment ≤42 ký tự/dòng ≤6s/cue.
> Code review (code-reviewer): 1 HIGH + 3 MEDIUM + 7 LOW → đã fix H1 (patchSettings merge chống
> clobber glossary), M1 (retry mất file), M3 (wrap 2 dòng cân bằng), L6 (mlx word `.get`), L7, L3.
> Known/accept: M2 (cancel hard-kill app-wide — descope có chủ đích), L2/L4/L5 (cosmetic/posture).
> Nguồn: tư vấn `/ask` — engine transcript đã chắc, gap nằm ở "last mile" (độ chính xác cấu hình được + review/sửa/xuất).
> Hướng chốt (user): **làm cả 2 gói, tuần tự** — Quick-win độ chính xác trước (tạo sẵn timestamp), rồi Review & Sửa.

## Mục tiêu

Nâng phần "sau khi ra chữ": để user (1) chỉnh vocab theo nội dung → đúng tên riêng; (2) xuất phụ đề chuẩn + nhiều định dạng; (3) nghe lại + sửa + lưu ngay trong app; (4) hủy/thử lại/dọn model gọn gàng.

**Không đụng**: kiến trúc sidecar persistent, kill-tree, orphan-guard, download poller — đang hoạt động tốt.
**Ngoài scope** (chưa làm, cần user xác nhận riêng): speaker diarization (pyannote), tóm tắt bằng LLM, đa ngôn ngữ cho MLX.

## Phases

| Phase | Nội dung | Tier | Trạng thái |
|-------|----------|------|-----------|
| [01](phase-01-glossary-setting.md) | Glossary cấu hình được (bỏ hardcode DevOps) | S | ✅ DONE |
| [02](phase-02-timestamps-exports.md) | `word_timestamps` + resegment SRT chuẩn phụ đề + export VTT/JSON | M | ✅ DONE |
| [03](phase-03-player-editor.md) | Player đồng bộ click-to-seek + editor sửa & lưu lại + list batch per-file | M | ✅ DONE |
| [04](phase-04-cancel-retry-model-mgmt.md) | Retry file lỗi + quản lý/xóa model (cancel giữ hard-kill — descope) | M | ✅ DONE (một phần) |

Thứ tự bắt buộc: **01 → 02 → 03**. Phase 04 độc lập, chèn cuối hoặc xen kẽ.

## Ranh giới file (tránh sửa chồng chéo)

- **Sidecar Python**: `sidecar/transcriber/main.py` — Phase 01, 02, 04.
- **Rust**: `settings.rs` (P01), `transcribe.rs` + `commands.rs` + `lib.rs` (P01/03/04), `capabilities/default.json` + `tauri.conf.json` (P03).
- **Frontend**: `use-transcribe.ts` / `use-record-transcribe.ts` (P01/03/04), `TranscriptView.vue` + component mới (P01/03/04), `asr-models.ts` (không đổi).

## Acceptance (toàn kế hoạch)

1. Đổi glossary trong app → file transcribe kế tiếp bám thuật ngữ mới; để trống → vẫn chạy (không prompt).
2. SRT xuất ra không có dòng > ~42 ký tự / > ~6s; mở bằng player phụ đề bất kỳ hiển thị đúng.
3. Chọn được xuất `.txt`/`.srt`/`.vtt`/`.json`; nội dung khớp nhau.
4. Bấm 1 dòng segment → player nhảy tới đúng mốc; sửa text → Lưu → file `.txt`/`.srt` trên đĩa cập nhật.
5. Batch nhiều file → xem được nội dung TỪNG file trong app (không chỉ file cuối).
6. Hủy 1 lần transcribe → dừng trong ≤1 segment, **model vẫn ấm** (lần sau không tải/nạp lại từ đầu).
7. File lỗi trong batch → có nút thử lại chỉ file đó.
8. Xem được model nào đã tải + dung lượng; xóa được để lấy lại disk.
9. `cargo build` + `vue-tsc --noEmit` sạch; smoke test transcribe 1 file audio VN ra đúng chữ.

## Rủi ro chính

- **P03 asset protocol**: phát file local trong Tauri cần bật `assetProtocol` (`tauri.conf.json`) + permission `core:asset` (capability) + `convertFileSrc()`. File user chọn ở đường dẫn bất kỳ → scope phải đủ rộng; cân nhắc bảo mật (chỉ đọc, desktop offline → chấp nhận được). Đây là điểm dễ vướng nhất.
- **P04 cancel mịn**: sidecar hiện đọc stdin tuần tự ở main loop → lúc transcribe (blocking) KHÔNG đọc được lệnh. Cần tách thread đọc stdin đặt `threading.Event`, vòng segment ct2 check giữa các đoạn. Là thay đổi cấu trúc `main()`, làm cẩn thận + giữ orphan-guard.
