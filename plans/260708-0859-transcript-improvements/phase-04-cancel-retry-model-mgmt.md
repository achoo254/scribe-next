# Phase 04 — Cancel mịn + retry file lỗi + quản lý model

> Tier **M** · Phụ thuộc: — (độc lập, làm sau) · Fix gap G6 (hủy = kill mất model ấm) + retry batch + dọn disk.

## Bối cảnh

- Hủy hiện tại: `transcribe.rs:285 shutdown()` → Drop → `kill_tree` → lần sau nạp lại model (chậm). Sidecar đọc stdin tuần tự ở `main.py::main` → lúc transcribe (blocking) KHÔNG nhận được lệnh cancel qua stdin.
- Mode C batch không có nút retry file lỗi (Mode A có qua `enqueue`).
- 3 model ~5GB, không có UI xem/xóa để lấy lại disk.

## Yêu cầu

- **Cancel mịn**: dừng trong ≤1 segment, GIỮ sidecar sống (model ấm). Giữ nút "Hủy cứng" (kill) làm fallback.
- **Retry**: file lỗi trong batch Mode C → nút thử lại chỉ file đó.
- **Model mgmt**: liệt kê model đã tải + dung lượng; xóa 1 model.

## Files sửa/tạo

- `sidecar/transcriber/main.py`:
  - Tách **thread đọc stdin** đặt lệnh vào queue + `threading.Event cancel_flag`; main xử lý queue. Khi nhận `{"cmd":"cancel"}` giữa lúc transcribe → set `cancel_flag`.
  - Vòng segment ct2 (`_transcribe_ct2`) check `cancel_flag` giữa các segment → dừng sớm, emit `{"type":"cancelled","file":...}`, clear flag. (mlx 1 lời gọi blocking → cancel mịn không áp; giữ kill fallback cho mlx.)
  - Giữ orphan-guard + shutdown nguyên vẹn.
- `src-tauri/src/transcribe.rs` + `commands.rs`:
  - `cancel_transcribe` mềm: `send({"cmd":"cancel"})` thay vì kill; thêm `cancel_transcribe_hard` = `shutdown()` cũ (fallback).
  - Command `list_models()` → `[{size, present, bytes}]` (dùng `dir_size` + `model_present` sẵn có).
  - Command `delete_model(size)` → `fs::remove_dir_all(model_dir)`.
  - Đăng ký ở `lib.rs`.
- `src/composables/use-transcribe.ts`: xử lý event `cancelled` (settle file, không dừng cả batch nếu còn); `cancel()` gọi bản mềm; thêm `retry(file)` = `transcribe_files([file])`.
- `src/views/TranscriptView.vue`: nút retry cạnh mỗi `fileErrors`; section "Quản lý model" (list size + dung lượng + nút xóa, disable khi đang transcribe/download).
- (Tuỳ) `src/components/AsrModelControls.vue`: đặt UI model mgmt ở đây cho dùng chung Mode A/C.

## Các bước

1. `main.py`: refactor `main()` → producer (thread đọc stdin → `queue.Queue`) + consumer (vòng chính). Cancel flag `threading.Event`. Test kỹ: shutdown/EOF vẫn thoát sạch, orphan-guard còn chạy.
2. ct2 loop check flag; emit `cancelled`.
3. Rust: `cancel_transcribe` mềm + `_hard` fallback; `list_models`/`delete_model`; đăng ký lib.rs.
4. Frontend: `cancelled` handler; `retry`; UI model mgmt + retry button.

## Test / validate

- Transcribe file dài → Hủy → dừng ≤1 segment; transcribe lại NGAY → **không** thấy log nạp lại model (model ấm giữ nguyên).
- Hủy cứng (fallback) vẫn kill sạch (không tiến trình ngầm — verify `ps`/Task Manager).
- Batch có 1 file hỏng → nút retry chỉ chạy lại file đó, không đụng file khác.
- Xóa model → `model_status` = false + disk giảm; tải lại được.
- `cargo build` + `vue-tsc --noEmit` sạch.

## Rủi ro / rollback

- **Refactor main loop** là chỗ rủi ro nhất — dễ hồi quy kill/orphan/zombie. Test cả 3 lối thoát: shutdown lệnh, EOF stdin, app chết đột ngột. Nếu phức tạp quá → giữ cancel cứng hiện tại, chỉ làm retry + model mgmt (vẫn giá trị).
- `delete_model` khi đang dùng size đó → chặn ở UI (disable) + check `present` lại sau xóa.
- Rollback từng phần: 3 mục độc lập, có thể ship riêng.
