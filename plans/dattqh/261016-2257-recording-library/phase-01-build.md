---
phase: 1
title: "Backend + web: index câu, tìm kiếm, hỏi đáp"
status: completed
effort: "L"
---

# Phase 1

Đọc trước: `docs/codebase-summary.md`; `server/scribe-be/src/{summary,transcribe/docs.rs,transcribe/export.rs,models/transcript.rs,models/transcript_doc.rs,models/file.rs,retention.rs}`; plan gói "Sau cuộc họp" `plans/dattqh/261016-2133-after-meeting-package/` (pattern đo lường, chạy LLM trong task riêng, chỉ hoàn lượt khi chưa bị tính tiền).

## Backend
- **Collection `transcript_sentences`:**
  - Mỗi câu hiệu lực một doc (đã áp phần sửa; `at` là giây tính theo cả phiên, giống biên bản): `{userId, key, fileId, idx, at, text, norm, expiresAt}`.
  - Index: `{userId, key}`, text index `norm` (`default_language: "none"`), TTL hoặc sweep theo file hết hạn.
  - Hàm `reindex(userId, key)` thay toàn bộ câu của phiên. Gọi khi lượt chép xong, khi lưu câu sửa, khi xoá đoạn hoặc phiên, và khi cắt file.
  - Lúc khởi động backfill một lần (cờ trong `kv`).
- **Chuẩn hoá `norm`:** viết thường, bỏ dấu, đ thành d, gộp khoảng trắng. Có unit test.
- **`GET /api/search?q=&limit=&key=`:**
  - Chuẩn hoá `q`; trước hết lọc bằng `$text` trên các từ, sau đó trong code đòi khớp cả cụm (`norm` chứa `q_norm`). Nếu không có kết quả thì nới ra khớp đủ các từ. Từ cuối khớp theo tiền tố.
  - Trả `{results: [{key, title, fileId, at, text, before?, after?}], total}`, sắp theo độ liên quan rồi mới tới thời gian; tối đa 50.
  - Rate limit nhẹ.
- **`POST /api/ask {question, key?}`:**
  - Lấy ngữ cảnh: với `key` của bản ghi ngắn (dưới khoảng 120k ký tự) thì lấy cả bản; còn lại thì tìm theo từ khoá của câu hỏi (bỏ stopword tiếng Việt, đã bỏ dấu), lấy top khoảng 40 câu, mỗi câu kèm 2 câu trước và 2 câu sau.
  - Gọi `gpt-4.1-mini` với structured output `{answer, citations: [{key, at}]}`. Prompt yêu cầu chỉ trả lời theo ngữ cảnh và trích `[n]`; không có thì trả lời "Không tìm thấy trong bản ghi".
  - Đo lường: `ASK_FREE_DAILY` 30, `ASK_PAID_DAILY` 200; dùng chung trần chi phí tháng của tóm tắt và trần tổng; chạy trong `tokio::spawn`; chỉ hoàn lượt khi chắc chắn chưa bị tính tiền; ghi `ask_runs` (không lưu câu hỏi hay câu trả lời, chỉ token và chi phí); audit `ask.create`.
  - Tài khoản bị khoá hoặc không được chép lời thì bị từ chối.
  - Trả `{answer, citations: [{key, title, at, text}]}`. Server tự điền `title` và `text` từ index; citation không có trong ngữ cảnh thì bỏ.

## Web
- **Ô tìm kiếm ở top bar** (hoặc trên trang "Lời đã chép"; chọn chỗ hợp với khung Localman): route `/search?q=`, gõ xong chờ một chút mới tìm, đánh dấu phần khớp, bấm vào mở `/transcripts/:key?t=<giây>` để trang đọc nhảy tới.
- **Hỏi đáp:** khối "Hỏi Scribe" trên trang tìm kiếm (cả kho) và trên trang đọc (một bản ghi). Có trạng thái chờ, lỗi hạn mức kèm giờ mở lại, và trích dẫn dạng chip bấm được. Không lưu lịch sử; giữ các câu đã hỏi trong phiên trang.
- Trang đọc nhận `?t=` để nhảy tới giây đó (nếu chưa có).
- Dùng i18n vi; test vitest; Playwright stub cho tìm kiếm và hỏi đáp, ảnh sáng và tối, axe.

## Cổng
Trong lúc làm, chạy test có chọn lọc. Cuối cùng chạy lần lượt: `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, rồi trong `web/`: `pnpm lint && pnpm typecheck && pnpm check:tokens && pnpm vitest run --maxWorkers=2 && pnpm build && pnpm test:e2e`. Gọi thật trên dev một lượt hỏi đáp với bản ghi thử.

## Kết quả (16/10/2026)

### Backend
- **Index câu** `transcript_sentences` (`server/scribe-be/src/models/sentence.rs`): mỗi câu hiệu lực một doc `{userId, key, fileId, idx, at, text, norm, expiresAt}`, dựng từ đúng đường ghép của màn hình đọc (`docs::load_db`: câu máy đã áp câu sửa, giây theo cả phiên). Index `{userId, key, idx}`, text index `{userId, norm: "text"}` với `default_language: "none"`, TTL trên `expiresAt` (= hạn của file chứa câu).
- **Dựng lại** (`src/search/index.rs`): `reindex(userId, key)` thay cả phiên; gọi nền (`tokio::spawn`, khoá theo phiên trong tiến trình) khi lượt chép kết thúc (xong hay lỗi), khi lưu câu sửa, khi xoá transcript, và trong `file::delete_doc` (xoá tay, hết hạn, cắt bản ghi). Đổi tên không cần dựng lại: tiêu đề đọc lúc tìm.
- **Backfill** lúc khởi động: tạo index rồi dựng mọi phiên có đoạn đã chép xong; xong thì ghi `kv` `{_id: "search.backfill", doneAt, sessions, sentences}`. Có phiên lỗi thì không ghi cờ, lần khởi động sau chạy lại (idempotent).
- **Chuẩn hoá** (`src/search/norm.rs`): NFD rồi bỏ dấu kết hợp, đ thành d, viết thường, ký tự không phải chữ/số thành khoảng trắng, gộp khoảng trắng (thêm crate `unicode-normalization`, thuần Rust). Từ khoá câu hỏi bỏ từ đệm tiếng Việt (đã bỏ dấu).
- **Tìm** (`src/search/mod.rs`): một từ thì lọc theo tiền tố trong phạm vi câu của người đó; nhiều từ thì `$text` trên các từ đã gõ xong, rồi trong code đòi khớp cả cụm liền nhau (từ cuối theo tiền tố); không có câu nào thì nới ra câu có đủ các từ. Xếp theo số lần khớp, rồi phiên mới hơn, rồi thời gian. Tối đa 2000 ứng viên mỗi lần; 120 lần mỗi phút mỗi người.
- **Hỏi đáp** (`src/search/ask.rs`): như đã chốt. Có `key` và bản ghi dưới 120.000 ký tự thì gửi nguyên bản; còn lại lấy 40 câu liên quan nhất theo từ khoá, mỗi câu kèm 2 câu trước và sau, gộp khoảng chồng nhau. Không có câu liên quan thì trả "Không tìm thấy trong bản ghi." mà không gọi model và không tính lượt. Model trả `{answer, citations: [n]}` với `n` là số dòng ngữ cảnh (đáng tin hơn bắt model chép lại khoá phiên); server đổi `n` thành `{key, title, at, text}` lấy từ ngữ cảnh, bỏ số ngoài ngữ cảnh, đánh số lại 1, 2, … theo thứ tự xuất hiện, và nếu câu trả lời không có `[n]` nào thì thêm các số vào cuối. Tối đa 20 trích dẫn, câu hỏi tối đa 500 ký tự, 10 lượt mỗi phút và một câu đang chạy mỗi người.
- **Đo lường dùng chung** (`src/summary/meter.rs`): trình tự lấy lượt ngày → giữ chỗ sổ tháng của tài khoản → giữ chỗ trần tổng → mở lượt → gọi → quyết toán → đóng lượt → báo mốc trần, dùng cho cả tóm tắt lẫn hỏi đáp; chạy trong task riêng; chỉ hoàn lượt khi OpenAI chắc chắn chưa xử lý. `models/summary.rs` nhận `Feature::{Summary, Ask}`: sổ ngày hỏi đáp `summary_usage` `ask:<userId>:<YYYY-MM-DD>`, sổ chi phí tháng `cost:<userId>:<YYYY-MM>` dùng chung, nhật ký `ask_runs` (không có câu hỏi hay câu trả lời). Cờ tạm ngưng khi key hỏng dùng chung cho tóm tắt và hỏi đáp (cùng API), không ngắt chép lời. `summary/llm.rs` thành client structured output dùng chung (`complete::<T>`).
- Biến build mới: `ASK_FREE_DAILY` (30), `ASK_PAID_DAILY` (200). Model hỏi đáp dùng `SUMMARY_MODEL` (giá token quy đổi là của `gpt-4.1-mini`).
- `admin usage --recompute` cộng thêm `ask_runs.costS` vào trần tổng; `--apply` từ chối khi còn lượt hỏi đáp đang mở; kết quả có thêm `runningAsks`.
- Audit: `ask.create` (`scope`, `context`, token, `costS`, số trích dẫn), `ask.blocked` (`DailyLimit`/`MonthlyLimit`/`TotalCap`), `ask.failed`. Câu tìm không vào audit.

### Web
- Ô tìm trên trang "Lời đã chép" (`pages/search/search-entry.tsx`, chọn chỗ này thay vì top bar để top bar giữ đúng khung Localman và không chật trên iPhone) mở `/search?q=`. Trang `/search` (`pages/search/search-page.tsx`): tìm sau 300 ms ngừng gõ, URL giữ câu tìm, tô phần khớp ngay trên chữ có dấu (`features/search/normalize.ts` chuẩn hoá giống backend từng ký tự), kèm câu trước và sau; bấm một câu mở `/transcripts/:key?t=<giây>`.
- Trang đọc nhận `?t=`: nạp xong thì nhảy tới câu, đặt focus và phát từ giây đó.
- Khối "Hỏi Scribe" (`pages/search/ask-card.tsx`) trên trang tìm (cả kho) và trên trang đọc (một bản ghi, chỉ hiện khi có đoạn đã chép): trạng thái chờ, lỗi `ASK_DAILY_LIMIT`/`ASK_MONTHLY_LIMIT`/`CAP_REACHED` kèm giờ mở lại, `[n]` trong câu trả lời là chip bấm được, danh sách "Nguồn" bên dưới; các câu đã hỏi giữ trong trang, không lưu.
- Câu báo hết hạn mức tháng của tóm tắt đổi thành "hạn mức tóm tắt và hỏi đáp" vì trần đó giờ dùng chung.

### Kiểm tra
- `cargo clippy --workspace --all-targets -- -D warnings`: 0 cảnh báo. `cargo test --workspace`: 108 đạt (scribe-be 95), 12 test Mongo/OpenAI thật để `--ignored`. Chạy trên DB dev đạt: `sentence_index`, `reindex_follows` (chép xong, câu sửa, xoá đoạn qua `delete_doc`, phiên hết file), `summary_daily` (sổ ngày hỏi đáp tách khỏi tóm tắt), `transcript_docs_lifecycle`.
- Web: lint 0 lỗi (231 tệp); typecheck 0 lỗi; check:tokens đạt (82 cặp tương phản; 9 cặp tư vấn APCA có từ trước); vitest 56 tệp, 296 test đạt; build đạt, 0 cảnh báo, `check:dist` sạch; e2e 77 đạt, 7 bỏ qua (nhánh chỉ có ở Chromium hoặc iPhone, như trước). Ảnh chụp mới `search-*` (sáng/tối, desktop và iPhone); cập nhật `transcripts-*` (ô tìm), `reader-*` và `reader-minutes-*` (khối "Hỏi Scribe"); đã xem từng ảnh. axe sạch trên trang tìm có kết quả và câu trả lời.
- **Gọi thật trên dev** (backend dev cổng 8787, `CHAT_LINK_POLL` tắt; đã dừng sau khi thử, phiên đăng nhập tạm đã xoá): backfill lúc khởi động dựng 2 phiên, 8 câu; khởi động lại không chạy lần hai (cờ). Tìm "ra mat" ra 2 câu có "ra mắt" (câu nhắc hai lần đứng đầu), "chot ngay ra m" ra đúng câu có "chốt ngày ra mắt", "huong tai" (không liền nhau) nới ra đúng câu "tài liệu hướng dẫn", mỗi kết quả có mốc giây và câu trước. Hỏi cả kho "Ngày ra mắt bản iPhone là khi nào?" → "thứ hai ngày 20 tháng 10", trích đúng câu ở giây 6,5; hỏi trong bản ghi "Ai làm tài liệu hướng dẫn, hạn khi nào?" → "Hùng…, hạn là cuối tháng", trích câu ở giây 35,0; "Doanh thu quý trước là bao nhiêu?" (trong bản ghi) và "Thời tiết Đà Lạt ra sao?" (cả kho, không có câu liên quan, không gọi model) đều trả "Không tìm thấy trong bản ghi.". Mỗi lượt gọi model khoảng 560–590 token vào, 14–30 token ra, quy ra 6 giây (giữ chỗ 58 giây rồi quyết toán); 4 lượt trong `ask_runs` đều đã đóng. Đặt sổ ngày lên 30/30 thì `POST /api/ask` trả 403 `ASK_DAILY_LIMIT` kèm `retryAt` 0 giờ hôm sau; đã trả sổ về số cũ.

### API cho web (chỉ thêm)
| Route | Hợp đồng |
|---|---|
| `GET /api/search?q=&limit=&key=` | Đăng nhập là đủ. `q` tối đa 200 ký tự (400 nếu dài hơn); rỗng sau khi chuẩn hoá thì `{results: [], total: 0}`. `limit` 1–50, mặc định 20. `key` (tuỳ chọn, khoá phiên hợp lệ, sai dạng thì 400) giới hạn trong một bản ghi. Trả `{results: [{key, title, createdAt, fileId, at, text, before, after}], total}`: `title` là tiêu đề người dùng đặt hoặc tên file của bản ghi một file, `null` với phiên iPhone chưa đặt tên; `createdAt` là lúc bắt đầu phiên (RFC 3339); `at` là giây tính theo cả phiên; `before`/`after` là câu ngay trước/sau hoặc `null`; `total` là số câu khớp. 429 khi quá 120 lần mỗi phút. |
| `POST /api/ask {question, key?}` | Cần quyền chép lời; tài khoản bị khoá 403. `question` 1–500 ký tự (400). Có `key` mà bản ghi chưa có câu nào: 409 `NOT_TRANSCRIBED`. Trả đồng bộ `{answer, citations: [{n, key, title, createdAt, at, text}]}`; `[n]` trong `answer` là `citations[n-1]`. Không có thông tin thì `answer` là "Không tìm thấy trong bản ghi." và `citations` rỗng. 403 `ASK_DAILY_LIMIT` (`usedMinutes`/`limitMinutes` là số lượt), `ASK_MONTHLY_LIMIT` (phút quy đổi, trần dùng chung với tóm tắt), `CAP_REACHED`, đều kèm `retryAt`; 429 khi bấm quá nhanh hoặc đang có một câu chạy; 503 khi tạm ngưng hoặc lỗi model. |
| `POST /api/transcript-docs/{key}/summary` | Không đổi body; câu báo 503 tạm ngưng và 403 `SUMMARY_MONTHLY_LIMIT` nay nói "tóm tắt và hỏi đáp" vì cờ tạm ngưng và trần tháng dùng chung. |

### Còn để ngỏ
- Ứng viên tìm kiếm giới hạn 2000 câu mỗi lần; người có kho rất lớn và câu tìm một từ rất phổ biến có thể không thấy hết (`total` cũng tính trong phạm vi đó).
- Model đôi khi không ghi `[n]` trong câu trả lời; server thêm số vào cuối nên nguồn vẫn bấm được, nhưng chip không nằm ngay sau từng ý.
