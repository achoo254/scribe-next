---
name: onescribe-sidecar-sequential-stdin
description: sidecar/transcriber/main.py xử lý stdin JSON tuần tự trên 1 thread — không có race giữa các cmd nhưng 1 cmd chạy lâu (download) sẽ block mọi cmd sau
metadata:
  type: project
---

`sidecar/transcriber/main.py::main()` đọc `for line in sys.stdin` và xử lý mỗi `cmd` XONG HẲN (blocking,
đồng bộ) mới đọc dòng tiếp theo — KHÔNG có thread/async nào xử lý song song nhiều cmd. Hệ quả khi
review code liên quan sidecar:

- Không có race condition giữa 2 cmd Python (ví dụ `ensure_cuda` đang tải và `transcribe` gửi tới) vì
  chúng luôn chạy tuần tự trên 1 thread — cmd sau chỉ được đọc từ stdin buffer sau khi cmd trước
  return/raise.
- Nhược điểm: nếu 1 cmd chạy lâu (tải CUDA runtime ~1.3GB, tải model lớn), MỌI cmd gửi sau đó
  (kể cả `transcribe` của file khác/tab khác) đều bị delay tới khi cmd đang chạy xong — không deadlock,
  chỉ là hàng đợi (queue), UX có thể trông như "treo" nếu không có thông báo rõ.
- Phía Rust (`src-tauri/src/transcribe.rs`) dùng `Mutex<Option<Proc>>` bọc stdin/child — nhiều lệnh
  gọi `Transcriber::send()` đồng thời từ nhiều Tauri command chỉ serialize việc VIẾT vào stdin (không
  đảm bảo thứ tự xử lý phía Python khác gì so với thứ tự ghi, nhưng do ghi có lock nên thứ tự ổn định).

**Why:** xác nhận lúc review feature GPU CUDA (2026-07-09) — dùng để loại trừ giả thuyết race giữa
poller `ensure_cuda`/`transcribe` ở tầng Python; race (nếu có) chỉ nằm ở tầng Rust (2 `AtomicBool`
poller threads) hoặc tầng UI (Vue composable), không phải ở sidecar.

**How to apply:** khi nghi ngờ race condition trong luồng transcribe/sidecar, kiểm tra tầng Rust
(`Transcriber` struct, `AtomicBool` flags, poller threads) và tầng UI composable TRƯỚC — sidecar Python
tự nó không có race nội bộ do đơn luồng xử lý cmd.

Related: [[onescribe-transcribe-msg-error-pattern]]
