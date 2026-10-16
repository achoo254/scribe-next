# Review: phần A — key server + sổ phút (2026-10-16)

Reviewer độc lập đọc diff phần A trước deploy. Ổn: đóng sổ đúng một lần, `runId` chặn lượt cũ, khoá trùng `$nin`, ranh giới tháng, key không lọt log/argv, CLI admin tách trước worker, chống chèn regex.

| # | Mức | Phát hiện | Xử lý |
|---|---|---|---|
| 1 | High | Doc dở của bản cũ (không có `open`) kẹt `running` mãi sau deploy | Sửa: khởi động đánh `failed` các doc `queued/running` không có `open` |
| 2 | Medium | Lỗi Mongo sau bước giữ chỗ làm rò phút (không hoàn) | Sửa: `reserve` trả lại mọi bước khi lỗi; `create` không còn `?` sau giữ chỗ, đọc lại theo `runId` trước khi hoàn |
| 3 | Medium | Chạy lại sau restart gửi lại cả file mà không giữ thêm | Sửa: giữ thêm `ceil(duration) + sentS − chargedS`. Thử lại theo temperature và làm tròn mảnh: chấp nhận vượt có giới hạn, ghi trong comment |
| 4 | Medium | `--recompute --apply` đếm hai lần lượt mở và ghi đè sổ khi service chạy | Sửa: lượt mở tính `chargedS`, ghi bằng `$inc` chênh lệch, từ chối `--apply` khi còn lượt mở, duyệt cả user chỉ có sổ |
| 5 | Low/Medium | Ghi Mongo standalone báo lỗi dù đã ghi ⟹ quyết toán bị bỏ | Sửa: đóng lượt hai bước (`open` → `settled`), thử lại an toàn; khởi động quyết toán lượt dở |
| 6 | Low | Thông điệp OpenAI (tổ chức/dự án) lộ cho người dùng | Sửa: người dùng thấy câu chung + mã, chi tiết chỉ ở log |
| 7 | Low | App iPhone đang cài vẫn bắt nhập key (bundle cũ) | Chưa sửa: cần cài lại app qua cáp (Phase 6). Server bỏ qua key gửi lên |
| 8 | Low | `/api/me` không báo khi chép lời tạm ngưng | Sửa: thêm `paused`, web hiện câu tạm ngưng |

Sau sửa: clippy sạch, `cargo test` đạt, 3 test Mongo thật đạt (thêm kịch bản thử lại đóng sổ và doc của bản cũ), web 170/170, E2E dev 11/11 (chép thật bằng key server).
