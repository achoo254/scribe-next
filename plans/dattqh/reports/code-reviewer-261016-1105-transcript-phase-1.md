# Review: Transcript giai đoạn 1 (2026-10-16)

Reviewer độc lập đọc toàn bộ diff (backend, web, iOS) trước deploy prod. Kết luận: backend đúng về quyền sở hữu, chống injection (regex/field path) và các race chính; có một lỗi High và một lỗi Medium ở web, đã sửa trước deploy.

| # | Mức | Phát hiện | Xử lý |
|---|---|---|---|
| H1 | High | Bản nháp sửa lấy `runId` lúc bấm Lưu thay vì lúc bắt đầu sửa: đoạn được chép lại giữa chừng thì chữ của lượt cũ lưu được lên lượt mới (`reader-segment.tsx`, `transcript-reader.tsx`) | Sửa: bản nháp giữ `runId` lúc bắt đầu; khác `runId` hiện tại thì khoá Lưu và báo. Test mới trong `transcript-reader.test.tsx` |
| M1 | Medium | Hộp xác nhận chép lại đếm toàn bộ câu của đoạn có sửa, không phải số câu đã sửa | Sửa: backend trả `editedSentences` (số câu khác câu máy); web đếm theo đó |
| L1 | Low | Nhánh 409 của PUT gỡ `edits.<fileId>` không điều kiện, có thể xoá phần sửa của lượt mới | Sửa: `unset_edits_of_run` chỉ gỡ khi `runId` còn khớp |
| L2 | Low | Web nhận tên đoạn `{8,}`, backend `{8,64}` và index vừa u32 | Sửa: cả hai dùng id phiên 8–64 ký tự, index 1–9 chữ số |
| L3 | Low | Sweep mồ côi N+1 truy vấn mỗi giờ | Giữ: quy mô hiện tại nhỏ; đổi sang `$lookup` khi dữ liệu lớn |
| L4 | Low | iOS: `clear()` đua với lần tải dở; `.part` có thể sót khi `replaceItemAt` lỗi | Giữ: rủi ro thấp (khoá cache là id file khó đoán) |
| L5 | Low | Mất quyền khi đang ở tab Transcript thì màn hình trống | Sửa: tab hiệu lực về Ghi âm |

Câu hỏi còn mở từ reviewer: WKWebView có cho `autoPlay` đoạn kế mà không cần thao tác không — kiểm ở QA iPhone (Phase 4).

Kiểm lại sau sửa: `cargo clippy -D warnings` sạch, `cargo test --workspace` đạt, 2 test Mongo thật đạt; web lint/typecheck/check:tokens sạch, 169/169 test.
