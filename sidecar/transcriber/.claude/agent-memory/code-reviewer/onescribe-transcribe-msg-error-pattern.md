---
name: onescribe-transcribe-msg-error-pattern
description: OneScribe Tauri app dùng 1 kênh event "transcribe_msg" chung cho nhiều tác vụ async (model download, cuda download, transcribe) — mỗi composable lắng nghe phải tự xử lý case "error", không có xử lý mặc định
metadata:
  type: project
---

Kênh event `transcribe_msg` (emit từ `src-tauri/src/transcribe.rs` reader thread) dùng CHUNG cho mọi
loại tác vụ sidecar (download model, download CUDA runtime, transcribe file). Mỗi composable Vue
(`use-transcribe.ts`, `use-gpu.ts`, ...) tự `listen("transcribe_msg", ...)` riêng và filter theo
`type` — Tauri gọi TẤT CẢ listener đã đăng ký cho cùng 1 event, không có cơ chế "claim"/exclusive.

**Pattern chuẩn đã có ở `use-transcribe.ts` (case "error"):** khi tác vụ lỗi giữa chừng (network
drop, sha256 sai, disk đầy…), sidecar Python luôn kết thúc bằng `{"type":"error","file":...,
"message":...}` (main.py `except Exception` bọc quanh toàn bộ dispatch). Composable PHẢI có case
"error" để reset state `downloading`/`transcribing` về false + set `error.value` — nếu thiếu case
này, các ref boolean set `true` lúc bắt đầu tác vụ (trong hàm `ensure()`/`transcribe()`) sẽ TREO VĨNH
VIỄN vì không còn event nào khác cập nhật lại.

**Why:** phát hiện lúc review feature GPU CUDA (2026-07-09) — `use-gpu.ts::useCudaRuntime().init()`
chỉ handle `cuda_download`/`cuda_ready`, THIẾU case `error` → khi tải CUDA runtime lỗi giữa chừng,
`downloading.value` kẹt `true` mãi, nút tải bị disable vĩnh viễn, không có thông báo lỗi nào hiện ra
(cả 2 listener trên cùng kênh đều bỏ qua event vì `m.file` null + điều kiện tab-ownership không khớp).
Nặng hơn vì `TranscriptView.vue` nằm trong `<keep-alive>` (xem `App.vue`) → composable instance sống
suốt session, không tự reset khi chuyển tab.

**How to apply:** mỗi khi review/thêm 1 composable mới lắng nghe `transcribe_msg` với 1 loại tác vụ
async có thể fail giữa chừng (download, spawn sidecar…) — luôn kiểm tra có case `"error"` (hoặc filter
tương đương) để reset đúng state boolean + hiển thị `error.value`. Đối chiếu với pattern đã có ở
`use-transcribe.ts` dòng ~154-169.

Related: [[onescribe-sidecar-sequential-stdin]]
