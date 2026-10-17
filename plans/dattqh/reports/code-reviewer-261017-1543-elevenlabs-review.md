# Review chuyển chép lời sang ElevenLabs (c06f8ec, 77f6abd, cf4d017)

Review đọc mã, không chạy test. Không Critical. Ba lỗi High chặn deploy prod.

## High
- **H1** `whole.rs:113-123`: chờ đoạn đang tải lên nhưng không thêm đoạn đó vào lượt chạy → lượt sau chép lại cả phiên, tính tiền hai lần, mất tên người nói. Sửa: sau khi chờ, liệt kê lại phiên, giữ phút và xếp các đoạn thiếu vào cùng `runId`.
- **H2** `whole.rs:212-236`: ElevenLabs từ chối (401/403/402/5xx/4xx) vẫn tính đủ phút cho người dùng và trần USD (chỉ hoàn giờ ElevenLabs). Sửa: hoàn `sentS` cho mọi thành viên khi bị từ chối chắc chắn (trừ timeout); cả nhánh CANCELLED sau `count_sent`.
- **H3** `whole.rs:242-247`: một đoạn bị xoá trong lúc chờ kết quả → cả lượt `Failed(PARTIAL)`, kết quả đã trả tiền bị bỏ. Sửa: bỏ qua CANCELLED từng thành viên, ghi Done cho phần còn lại.

## Medium
- **M1** `mod.rs:330-347`: worker có thể nhận lượt khi chưa xếp xong các đoạn → một phiên tách thành nhiều request, nhãn người nói lệch. Sửa: xếp ở trạng thái chờ rồi bật cùng lúc bằng một `update_many`.
- **M2** `whole.rs:159-161`: một đoạn không đọc được làm hỏng mãi cả phiên. Sửa: đo trước, loại đoạn hỏng (đánh lỗi riêng), ghép phần còn lại.
- **M3** `transcript-panel.tsx:101-105`, `whole.rs:61-64`: chép lại cả phiên từ panel không hỏi trước khi mất câu đã sửa; tên người nói bị đặt lại mà không báo.
- **M4** `whole.rs:142-148,283-292`: mốc đầu đoạn có thể trôi trên phiên dài (độ trễ AAC). Sửa: `atrim`/`apad` từng đầu vào theo thời lượng đo.

## Low
L1 không thử lại sau khi đã gửi xong thân request (tránh bị tính hai lần) · L2 panic chỉ đánh lỗi đoạn đầu, các đoạn khác kẹt `running` · L3 crash giữa lượt không hoàn sổ `el:` · L4 lỗi ffmpeg lộ đường dẫn có userId ra người dùng · L5 `PAUSE_SPLIT_S` 1,5 s lệch plan 1,2 s · L6 API không trả `files` như plan · L7 hộp đổi tên bị xoá chữ đang gõ khi refetch · L8 deploy khi hàng đợi rỗng.
