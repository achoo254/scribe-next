---
phase: 2
title: "Review, deploy, docs"
status: completed
effort: "M"
---

# Phase 2
- Review (code-reviewer, chỉ đọc), sửa các phát hiện.
- Deploy theo skill `scribe-ops-prod`: hàng đợi và phiên đều 0, sao lưu, giữ `assets/`.
- Kiểm: health; log backfill; tìm kiếm và hỏi đáp thật trên prod với bản ghi hiện có.
- Docs: `docs/codebase-summary.md`, `docs/deploy-web-production.md` (biến build `ASK_*`), `docs/project-roadmap.md`; journal.

## Review 936e9fb (16/10/2026 23:35)
- **H1.** Xoá transcript hoặc bản ghi chỉ dọn index trong task nền. Task lỗi hoặc tiến trình bị restart thì câu đã xoá vẫn tìm được và vẫn bị gửi cho `/api/ask`. Sửa: `delete_many` đồng bộ ở `transcribe/docs.rs:472` và `models/file.rs:244`; thêm quét câu mồ côi vào `retention::sweep_once`.
- **M1.** Thay câu của phiên không nguyên tử (`models/sentence.rs:101-126`). Sửa: dấu thế hệ `gen`, cộng đối soát định kỳ (phiên có transcript done mà không có câu nào).
- **M2.** Đoạn iPhone tới muộn làm lệch `at`. Sửa: gọi `search::index::schedule` sau `file::create` ở `recordings.rs:220` và `upload.rs:202`.
- **M3.** Giới hạn 2000 ứng viên lấy theo thứ tự khoá, nên bỏ sót phiên mới. Sửa: thêm `createdAt` vào doc câu, sort giảm dần trước `limit`, index `{userId, createdAt}`; đòi ít nhất 2 ký tự cho tiền tố.
- **M4.** `session_meta` đọc mọi bản ghi ở mỗi request. Sửa: chỉ đọc các khoá cần, có projection.
- **M5 (cần Jev).** Hỏi trên bản ghi dài (tới 120k ký tự, khoảng 535 giây mỗi câu) làm cạn trần tháng free 3600 giây. Hướng: hạ `FULL_CONTEXT_CHARS` cho free hoặc tách trần riêng cho hỏi đáp.
- **Low.**
  - L1: tạo text index đồng bộ ở `main.rs`.
  - L2: cờ backfill khi chỉ một phiên lỗi.
  - L3: gộp các task dựng lại khi retention xoá hàng loạt.
  - L4: `?t=` đổi khi không remount.
  - L5: highlight khi `keepPreviousData`.

## Đã sửa sau review (17/10/2026, ec381b6)
- **H1.** Xoá transcript (`transcribe/docs.rs`) gọi `search::index::clear`: xoá câu của phiên ngay, trong khoá của phiên nên lượt dựng đang chạy không ghi lại câu đã xoá; lỗi thì trả lỗi. `file::delete_doc` gọi `index::forget_file`: xoá ngay câu của file rồi mới dựng lại nền (để mốc giây của đoạn còn lại dời theo); lỗi chỉ cảnh báo. `retention::sweep_once` thêm `sentence::delete_orphans`: xoá câu của file không còn doc file hoặc không còn transcript.
- **M1.** `replace_session` ghi thế hệ mới (`gen`) trước rồi mới xoá thế hệ cũ. Ghi lỗi giữa chừng thì gỡ phần vừa ghi; người đọc bỏ câu trùng `(key, idx)`. Retention mỗi giờ chạy `index::reconcile` (bỏ qua khi chưa backfill) để dựng lại ba loại phiên: có đoạn chép xong ra câu nhưng index rỗng, index có hơn một thế hệ hoặc doc chưa có `gen`, và index còn câu dù phiên không còn đoạn chép xong. Đoạn chép xong mà không có câu nào (im lặng) không bị tính là thiếu; trên dev, lỗi này từng làm một phiên bị dựng lại ở mỗi lượt quét.
- **M2.** `recordings::finalize` dựng lại index khi đoạn iPhone (`s:`) tới. `upload.rs` không đổi vì file ở đó là `kind: "upload"` (Cắt file), không bao giờ thuộc phiên ghi nên không có câu nào trong index.
- **M3.** Mỗi doc câu có thêm `createdAt` (lúc bắt đầu phiên) và index mới `{userId, createdAt: -1}`. Tìm theo tiền tố và `$text` của tìm kiếm lấy ứng viên từ phiên mới nhất trước (`Order::Newest`); hỏi đáp vẫn lấy theo điểm (`Order::Relevance`). Câu tìm chỉ có một từ thì cần ít nhất 2 ký tự; web báo "Gõ ít nhất 2 ký tự để tìm." và không gửi request. Thêm index `{fileId}` để xoá theo file và quét mồ côi.
- **M4.** `session_meta` chỉ đọc bản ghi của đúng các phiên cần (`file::session_heads`: `$or` theo `_id` hoặc tên đoạn, projection `_id/name/createdAt`) và tiêu đề cần (`transcript_doc::titles`, projection `key/title`).
- **M5 (Jev 0,98).** `full_context_chars(plan)`: tài khoản free chỉ gửi nguyên bản khi bản ghi dưới 30.000 ký tự, dài hơn thì lấy câu theo từ khoá trong bản ghi đó; gói trả phí giữ mốc 120.000.
- **L1.** Text index tạo đồng bộ ở `main.rs` trước khi nhận request; lỗi thì dừng khởi động. `start_backfill` không tạo index nữa.
- **L2.** Backfill ghi cờ cả khi vài phiên lỗi (thêm trường `failed`); phiên lỗi do đối soát định kỳ dựng lại.
- **L3.** Gọi `schedule` khi phiên đã có lượt chờ chạy thì không tạo thêm lượt (`PENDING`, gỡ sau khi đã giữ khoá và trước khi đọc dữ liệu); `schedule_file` đi qua `schedule`.
- **L4.** Trang đọc nhớ giây đã nhảy tới chứ không nhớ cờ đã nhảy, nên `?t=` đổi trên cùng bản ghi (back/forward) thì nhảy lại mà không cần remount.
- **L5.** Kết quả tìm mang theo câu tìm đã sinh ra nó (`SearchReplyFor.query`); phần tô dùng câu đó, nên lúc `keepPreviousData` thì không tô theo câu đang gõ.

### Kiểm tra
- `cargo clippy --workspace --all-targets -- -D warnings`: 0 cảnh báo. `cargo test --workspace`: 110 đạt (scribe-be 97), 12 bỏ qua.
- Test Mongo thật trên DB dev, chạy lần lượt: `sentence_index`, `reindex_follows`, `transcript_docs_lifecycle`, `bookmarks_move_to_split_pieces`, `transcript_queue_lifecycle`, `summary_daily`, cả 6 đạt. Các test này kiểm thế hệ, `delete_file`/`delete_session`/`delete_orphans`, phiên mới đứng trước khi lấy ứng viên, đối soát dựng lại phiên mất câu, `delete_doc` xoá câu ngay, `clear`, và `session_meta`.
- Backend dev (cổng 8787, đã dừng) khởi động bình thường và text index tạo đồng bộ. Trước khi sửa phần đoạn im lặng, mỗi lần khởi động đều dựng lại 1 phiên lệch; sau khi sửa thì không còn phiên lệch nào.
- Web: lint 0 lỗi (232 tệp), typecheck 0 lỗi, check:tokens đạt (82 cặp; 9 cặp tư vấn APCA có từ trước), vitest 57 tệp và 300 test đạt, build đạt và `check:dist` sạch, e2e 77 đạt và 7 bỏ qua (như trước). Không đổi ảnh chụp. Hai test web mới (tô theo câu tìm cũ, nhảy lại khi `?t=` đổi) đã được thử trên code cũ và đều trượt.

## Deploy (17/10/2026)
- **06:23 (+07)**, theo skill `scribe-ops-prod`. Sao lưu `/root/scribe-backup-20261017-062309`. Binary sha256 `d15d004579927cc3…`, `index-BrYuE0r6.js`.
- **Sau deploy:** health ok, `NRestarts=0`.
- **Backfill:** journal có dòng "đã dựng index tìm kiếm cho dữ liệu cũ" với `sessions=0 sentences=0 failed=0`. Đúng với dữ liệu: prod chỉ có một transcript đã chép xong và transcript đó không có câu nào. Dòng này chỉ in sau khi ghi xong cờ `search.backfill` (`src/search/index.rs`).
- **Index trên `transcript_sentences`:** `userId_1_key_1_idx_1`, `userId_1_createdAt_-1`, `fileId_1`, `userId_norm_text`, `expiresAt_1`.
- **Chưa kiểm trên prod (chờ chủ sản phẩm đăng nhập):** một lượt tìm kiếm và một lượt hỏi đáp thật.
- Docs: `docs/codebase-summary.md`, `docs/deploy-web-production.md`, `docs/project-roadmap.md`; journal `plans/dattqh/reports/journal-261017-0625-recording-library.md`.
