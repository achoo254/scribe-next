---
title: Dịch lời chép Việt ↔ Anh
status: completed
created: 2026-10-17
tier: L
---

# Dịch lời chép Việt ↔ Anh

Gói P2 tiếp theo trong `docs/project-roadmap.md`, sau gói xuất tệp.

## Quyết định (Jev, chốt)
- Dịch cả phiên khi người dùng bấm, theo từng câu, giữ mốc giây của câu. Người dùng chọn ngôn ngữ đích: "Tiếng Anh" (`en`)
  hoặc "Tiếng Việt" (`vi`); mỗi phiên lưu tối đa một bản dịch cho mỗi ngôn ngữ.
- Lưu riêng ở collection `transcript_translations` (một doc mỗi `{userId, key, lang}`, unique), không nhét vào
  `transcript_docs` để danh sách và màn hình đọc không phải đọc cả bản dịch. Mỗi câu lưu theo `items.<fileId>.<chỉ số câu>`
  = `{h, t}`: `h` là băm chữ nguồn (16 ký tự hex đầu của SHA-256 chữ đã gộp khoảng trắng), `t` là bản dịch.
- Câu nguồn đổi sau khi dịch (sửa câu, chép lại đoạn): băm khác ⟹ câu đó "bản dịch cũ". "Dịch lại câu đã sửa" chỉ gửi
  câu chưa có bản dịch hoặc bản dịch cũ; cùng một nút dùng để dịch tiếp khi lượt trước lỗi giữa chừng.
- Đo lường: lượt ngày riêng (`TRANSLATE_FREE_DAILY` mặc định 3, `TRANSLATE_PAID_DAILY` mặc định 20, sổ
  `tr:<userId>:<ngày>` trong `summary_usage`), trần chi phí tháng của tài khoản và trần tổng tháng dùng chung với tóm tắt
  và hỏi đáp qua `summary::meter`. Mã lỗi `TRANSLATE_DAILY_LIMIT`, `TRANSLATE_MONTHLY_LIMIT`, `CAP_REACHED` (403, kèm
  `retryAt`). Nhật ký lượt `translate_runs`. Audit `translate.create` / `translate.blocked` / `translate.failed`
  (không ghi chữ). Cờ tạm ngưng khi key hỏng dùng chung với tóm tắt và hỏi đáp.
- Một lượt dịch = một lượt ngày + một lần giữ chỗ chi phí cho mọi câu cần dịch, quyết toán một lần theo tổng token thật
  của mọi lô. Lượt ngày chỉ trả lại khi OpenAI chắc chắn chưa xử lý lô nào.
- Model `SUMMARY_MODEL` (`gpt-4.1-mini`), lô tối đa 100 câu và 6.000 ký tự, JSON đánh số vào/ra (`{i, t}`), kiểm đủ số
  và đúng số thứ tự; lô hỏng thử lại một lần, hỏng nữa thì dừng lượt (phần đã dịch giữ lại). Giữ tên riêng và con số,
  văn nói cuộc họp tự nhiên. Các lô chạy tuần tự trong task nền (giữ khoá một lượt mỗi phiên tới khi xong); web hỏi
  tiến độ `done/total` qua route status.
- Màn hình đọc: nút "Dịch" chọn ngôn ngữ rồi chạy; tiến độ; rồi chuyển chế độ xem Gốc / Bản dịch / Song song (song song:
  bản dịch ngay dưới từng câu trên điện thoại, hai cột từ màn hình rộng). Chạm câu vẫn phát từ giây đó. Câu có bản dịch
  cũ có nhãn nhỏ "bản dịch cũ".
- Xuất: menu "Tải xuống" thêm lựa chọn ngôn ngữ khi phiên có bản dịch (Gốc / Tiếng Anh / Tiếng Việt), áp cho cả TXT,
  SRT, VTT, MD, DOCX (`lang=en|vi`). MD/DOCX dịch cả tiêu đề mục. **Biên bản giữ nguyên ngôn ngữ gốc** (không gọi model
  thêm). Câu chưa có bản dịch thì xuất chữ gốc; câu có bản dịch cũ vẫn xuất bản dịch đó (như màn hình đọc).
- Tìm kiếm và hỏi đáp không đổi.

## API
| Method | Route | Ghi chú |
|---|---|---|
| POST | `/api/transcript-docs/{key}/translation` `{lang}` | 202 `{lang, status:"running", done:0, total}`; không còn câu cần dịch → 200 `{status:"done", total:0}` và không tính lượt. 400 `lang` lạ, 409 `NOT_TRANSCRIBED`, 403 `TRANSLATE_DAILY_LIMIT`/`TRANSLATE_MONTHLY_LIMIT`/`CAP_REACHED`, 413 quá dài, 429 đang dịch hoặc bấm quá nhanh, 503 tạm ngưng |
| GET | `/api/transcript-docs/{key}/translation?lang=` | `{lang, status, done, total, error, model, updatedAt, outdated, missing, segments:[{fileId, sentences:[{text, outdated} \| null]}]}`; chưa có bản dịch → `status:"none"` |
| GET | `/api/transcript-docs/{key}/translation/status?lang=` | `{lang, status, done, total, error}` (poll khi đang dịch) |
| GET | `/api/transcript-docs/{key}` | thêm `translations: [{lang, status}]` |
| GET | `/api/transcripts/export?…&lang=en\|vi` | 400 khi `lang` lạ hoặc các file không cùng một phiên, 404 khi phiên chưa có bản dịch ngôn ngữ đó; tên tệp thêm `-en`/`-vi` |

`status`: `none | running | done | failed`. Bản ghi `running` mà tiến trình không còn chạy lượt đó (khởi động lại) được báo
`failed` với lý do "bị ngắt", bấm dịch tiếp để làm nốt.

Xoá dữ liệu: xoá bản chép lời xoá luôn bản dịch; xoá một đoạn gỡ bản dịch của đoạn đó; sweep retention xoá bản dịch của
phiên không còn file.

## Phase
| Phase | Tier | Trạng thái |
|---|---|---|
| 1. Backend: model, meter, dịch theo lô, route, xuất có `lang`, test Rust | L | done |
| 2. Web: điều khiển dịch, chế độ xem, menu tải xuống theo ngôn ngữ, vitest, e2e | M | done |
| 3. Tài liệu, cổng kiểm tra, kiểm thử thật, commit, push | S | done |

## Tiêu chí nhận
- Lô: đầu ra map 1:1 theo số thứ tự; thiếu/thừa/trùng số → lô lỗi, thử lại một lần.
- Câu sửa sau khi dịch được báo cũ; "Dịch lại câu đã sửa" chỉ gửi các câu đó.
- Đo lường đúng trình tự meter; lượt ngày trả lại chỉ khi không lô nào được OpenAI xử lý.
- Xuất có `lang` cho 5 định dạng; MD/DOCX tiêu đề mục theo ngôn ngữ.
- Test Rust (lô, kiểm số, câu cũ, xuất có `lang`), test Mongo dev (bỏ qua mặc định), vitest điều khiển màn hình đọc, e2e
  dịch → đổi chế độ xem → menu tải xuống theo ngôn ngữ, có ảnh chụp.
- Cổng: clippy 0, cargo test, lint, typecheck, vitest, build, e2e.

## Kết quả 17/10
- Chọn ngôn ngữ chưa có bản dịch thì dịch ngay và chuyển sang "Song song"; chọn ngôn ngữ đã có thì chỉ chuyển xem. Mở lại
  bản ghi thì xem "Gốc", nút chuyển chế độ ở ngay đó. Đọc bản dịch chỉ cần đăng nhập; bắt đầu dịch cần quyền chép lời.
- Không còn câu cần dịch thì `POST` trả `done` ngay, không tính lượt (và sửa trạng thái `failed` cũ thành `done`).
- `admin usage --apply` coi lượt dịch mở dưới 60 phút là còn chạy (tóm tắt, hỏi đáp giữ 10 phút).
- Kiểm thử thật một lần (`live_translation`, bỏ qua mặc định): 5 câu Việt → Anh khớp 1:1, 388 token vào / 101 ra (7 giây quy
  đổi). Lần đầu model bỏ dấu tên riêng ("Hung", "Minh Phat"); hướng dẫn thêm yêu cầu giữ nguyên dấu, lần hai giữ đúng.
- Test Mongo dev: `translations_lifecycle` (ghi theo lô vào đường dẫn chưa có, hai lượt đua chỉ một doc, gỡ đoạn, sweep,
  lô muộn không tạo lại doc), `summary_daily` (thêm sổ ngày của dịch), `transcript_docs_lifecycle`: đạt.
- Cổng: clippy 0 cảnh báo; cargo test 145 đạt (18 bỏ qua, cần Mongo/OpenAI); biome sạch; typecheck sạch; vitest 346/346;
  build đạt, không cảnh báo; e2e 89 đạt, 7 bỏ qua theo trình duyệt. Ảnh chụp mới: `translation-controls-*`,
  `translation-both-*`, `translation-download-*`; ảnh màn hình đọc ở `pages.spec` cập nhật vì có thêm nút "Dịch".
- Chưa deploy.

## Deploy 17/10 13:08
Chủ dự án cho phép trước ("deploy luôn khi xong"). Build từ `1e2613c` (code `59432d0`). Sao lưu `/root/scribe-backup-20261017-130838`,
binary sha256 `ff3a7029afb91b9f…`, `index-BRC0kpPu.js`, health ok, `NRestarts=0`, không ERROR/WARN.
