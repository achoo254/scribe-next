---
phase: 1
title: "Backend: tóm tắt, dấu mốc, liên kết + gửi chat"
status: completed
effort: "L"
---

# Phase 1: Backend

Đọc trước: `docs/codebase-summary.md`, `server/scribe-be/src/{routes.rs,config.rs,notify.rs}`,
`src/transcribe/{docs.rs,export.rs,mod.rs,openai.rs}`, `src/models/{transcript_doc.rs,usage.rs,user.rs}`,
`~/inet-gitlab/chat-next/docs/bot-api/README.md` (API bot: `/updates`, `/messages`, deep link).

## 1. Tóm tắt (biên bản)
- `POST /api/transcript-docs/{key}/summary` tạo (hoặc tạo lại) biên bản; `GET /api/transcript-docs/{key}` trả kèm `summary` đã lưu.
- **Đầu vào:** câu của phiên, đã áp phần người dùng sửa (dùng lại đường dựng chữ của `export.rs` hoặc `docs.rs`), mỗi câu kèm giây bắt đầu tính theo cả phiên. Chưa chép xong đoạn nào thì trả 409 với thông điệp rõ ràng.
- **Gọi model:** OpenAI Chat Completions, `gpt-4.1-mini`, structured output JSON schema `{summary: string, decisions: [{text, at}], actions: [{text, owner?, due?, at}]}`, `at` là giây. Prompt yêu cầu viết tiếng Việt và chỉ dùng điều có trong bản ghi. Key đi qua header, giống `openai.rs`. Timeout và thử lại tương tự lượt chép.
- **Lưu:** `transcript_docs.summary = {model, createdAt, sourceHash, summary, decisions, actions}`. `sourceHash` là băm của chữ đầu vào; API trả `stale: true` khi chữ hiện tại khác chữ đã tóm tắt.
- **Đo lường:**
  - Mỗi người, mỗi ngày giờ VN: free 10 lượt (biến build `SUMMARY_FREE_DAILY`); gói trả phí cao hơn (`SUMMARY_PAID_DAILY`, mặc định 50). Hết lượt trả lỗi Quota có `retry_at`, như chép lời.
  - Ghi token vào `summary_runs`.
  - Quy chi phí token ra "giây tương đương" theo giá chép lời ($0,003/phút; gpt-4.1-mini $0,40/M vào, $1,60/M ra) và cộng vào sổ trần tổng tháng hiện có, để cảnh báo 80/100% và chặn ở 100% vẫn đúng.
  - Người bị `blocked` thì bị từ chối.
- Rate limit theo người chống bấm liên tục (tối đa một lượt đang chạy cho mỗi key).

## 2. Dấu mốc
- `POST /api/transcript-docs/{key}/bookmarks {at, label?}` trả `{id, at, label}`; `DELETE /api/transcript-docs/{key}/bookmarks/{id}`; `GET` doc trả `bookmarks` sắp theo `at`.
- Doc tạo lười bằng upsert, vì lúc đang ghi chưa có doc. Giới hạn 200 mốc mỗi doc, nhãn tối đa 200 ký tự.
- Kiểm quyền sở hữu key giống các route doc hiện có. Với key `f:<fileId>` của bản ghi trình duyệt, file phải thuộc người dùng; nếu file đang ghi chưa có bản ghi thì dùng đúng cách kiểm của `recordings`.

## 3. Liên kết + gửi chat
- `users.chat = {chatId, linkedAt}`. `GET /api/chat/link` trả `{linked}`. `POST /api/chat/link` tạo token một lần ngẫu nhiên, lưu băm, hạn 10 phút, rồi trả `{url: "https://chat.hoandat.com/b/<bot username>?start=<token>"}`; tên bot đọc từ `/bot/v1/me` lúc khởi động hoặc từ biến build. `DELETE /api/chat/link` gỡ liên kết.
- **Task nền** long-poll `GET {NOTIFY_BOT_API}/updates?offset=&timeout=50`:
  - Lưu offset trong Mongo (`kv`/`settings`) để restart không xử lý lại; bỏ trùng theo `update_id`.
  - Gặp `/start <token>` hợp lệ thì gắn `chatId` (chat riêng của người gửi) với user rồi trả lời "Đã liên kết với Scribe". Token sai hoặc hết hạn thì trả lời hướng dẫn mở lại từ Scribe.
  - Lỗi mạng thì chờ lùi dần. Gặp 409 (đang đặt webhook) thì log WARN và chờ.
  - Chỉ chạy khi có token bot.
- `POST /api/transcript-docs/{key}/share` gửi biên bản (tiêu đề, thời điểm, tóm tắt, quyết định, việc cần làm, link `https://scribe.hoandat.com/transcripts/<key>`) tới `chatId`, dạng markdown của bot API.
  - Chưa liên kết trả 409 `CHAT_NOT_LINKED`; chưa có biên bản trả 409.
  - Bot trả 403 (người dùng chặn bot) thì gỡ liên kết và báo lỗi rõ ràng.
  - Rate limit: 10 lượt mỗi giờ mỗi người.
- Audit: `summary.create`, `chat.link`, `chat.unlink`, `transcript.share` (không ghi nội dung).

## Kiểm tra
- Unit test: dựng đầu vào có mốc giây, quy đổi chi phí, bộ đếm ngày, parse `/start`, token hết hạn hoặc dùng lại, giới hạn dấu mốc.
- Gọi thật trên dev: một lượt tóm tắt với một bản ghi dev và một lượt liên kết + gửi tới bot (chat id 12 là của chủ sản phẩm).
- `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.

## Kết quả (16/10/2026)
- Đã làm đủ ba phần: tóm tắt có đo lường, dấu mốc, liên kết và gửi chat. Mã mới ở `server/scribe-be/src/summary/`, `src/chat_link/`, `src/transcribe/bookmarks.rs`, `src/models/{summary,chat_link}.rs`.
- Biến build mới: `SUMMARY_FREE_DAILY` (10), `SUMMARY_PAID_DAILY` (50), `SUMMARY_MODEL` (`gpt-4.1-mini`), `CHAT_LINK_POLL` (mặc định tắt), `CHAT_BOT_USERNAME` (không bắt buộc, thiếu thì đọc `/bot/v1/me`). Gửi chat và liên kết dùng tệp token `NOTIFY_BOT_TOKEN_PATH` sẵn có.
- `POST /api/recordings/init` trả thêm `fileId` giữ trước, để web biết khoá `f:<fileId>` và đánh dấu ngay lúc đang ghi.
- Sweep mồ côi chừa `transcript_docs` tạo chưa tới 24 giờ, để dấu mốc có trước khi có file không bị xoá.
- `admin usage --recompute` cộng chi phí tóm tắt (`summary_runs.costS`) vào trần tổng.
- Kiểm tra: clippy sạch, `cargo test --workspace` đạt 73 test (9 test Mongo/OpenAI thật để `--ignored`, 8 test Mongo đã chạy đạt trên DB dev). Gọi thật trên dev: một bản ghi 50 giây (giọng đọc tiếng Việt) được chép lời rồi tóm tắt đúng mốc giây (631 token vào, 272 ra, quy ra 14 giây); bấm lại khi hết lượt trả `SUMMARY_DAILY_LIMIT` kèm giờ mở lại; gửi biên bản tới chat 12 thành công; long-poll chạy thử khoảng một phút, không lỗi.
- Chưa thử đầu cuối bước bấm deep link rồi gửi `/start` (cần tài khoản chat của chủ sản phẩm), và chưa thử đường bot trả 403. Hai đường này đã có unit test.

## Sửa sau review (16/10/2026)
- **Tóm tắt chạy trong task riêng:** phần có tiền (lấy lượt ngày → giữ chỗ → gọi model → quyết toán → lưu biên bản) chạy bằng `tokio::spawn`, guard "đang chạy" đi theo task; handler chờ kết quả nên `POST …/summary` vẫn trả biên bản đồng bộ như cũ. Client ngắt kết nối thì task vẫn chạy hết, không treo phần giữ chỗ. Gọi model có hạn tổng 80 giây (không thử lại sau khi quá giờ, chỉ thử lại khi chắc OpenAI chưa xử lý và còn ít nhất 25 giây).
- **Hoàn lượt ngày:** chỉ khi OpenAI chắc chắn chưa xử lý gì (không kết nối được, 401, 5xx/429 hết lượt thử). Bị cắt, bị từ chối hay sai dạng thì tính theo token thật và không hoàn; quá giờ sau khi đã gửi thì giữ nguyên phần ước lượng.
- **Trần chi phí tóm tắt mỗi tháng của từng tài khoản:** biến build `SUMMARY_FREE_MONTHLY_MINUTES` (60) và `SUMMARY_PAID_MONTHLY_MINUTES` (600), tính bằng phút chép lời tương đương, sổ ở `summary_usage` doc `cost:<userId>:<YYYY-MM>`.
- **Lỗi key ở lượt tóm tắt** chỉ tạm ngưng tóm tắt (cờ riêng, báo quản trị một lần), không ngắt chép lời.
- **`summary_runs`** mở trước khi gọi (`open`, `reservedS`) và đóng khi quyết toán. `admin usage --recompute --apply` từ chối khi còn lượt tóm tắt mở chưa quá 10 phút; lượt mở bỏ dở tính theo `reservedS`.
- **Liên kết chat có bước xác nhận:** `/start <token>` chưa liên kết; bot hỏi lại kèm email che bớt (`a***@domain`) và hai nút "Xác nhận liên kết" và "Huỷ". Token gắn với chat đầu tiên gửi nó, chỉ lần bấm từ đúng chat đó mới liên kết, và lời hỏi hết hạn cùng token (10 phút). Liên kết xong bot nhắc `/unlink`, bot hỗ trợ lệnh `/unlink`, và chat cũ được báo khi tài khoản liên kết sang chat khác. Sau khi token đã bị dùng, lỗi được báo là "Tạo đường dẫn mới trong Scribe".
- **Long-poll:** đọc từng update riêng (update sai dạng bị bỏ, offset vẫn tiến), bỏ qua tin cũ hơn 10 phút và đăng ký `allowed_updates=message,callback`.
- **Xoá đoạn ghi âm** thì bỏ cả biên bản, phiên hết file thì xoá cả doc. Sweep mồ côi chỉ chừa 24 giờ cho doc chưa có biên bản.
- **Gửi biên bản:** tài khoản bị khoá bị từ chối; biên bản đã cũ vẫn gửi được, kèm dòng "(biên bản có thể đã cũ)". `admin block` gỡ luôn liên kết chat.
- **Dấu mốc:** mỗi người tối đa 120 lần thêm mỗi phút.

### API thay đổi cho web (chỉ thêm)
| Route | Thay đổi |
|---|---|
| `GET /api/chat/link` | Thêm `linkedAt` (RFC 3339 hoặc `null`) và `enabled` (boolean: máy chủ có nhận liên kết mới không). |
| `POST /api/chat/link` | Khi liên kết tắt (`CHAT_LINK_POLL` tắt hoặc thiếu token bot): 503 `{error, code: "CHAT_LINK_DISABLED"}`. |
| `POST /api/transcript-docs/{key}/summary` | Mã mới 403 `SUMMARY_MONTHLY_LIMIT` (dạng Quota như `SUMMARY_DAILY_LIMIT`; `usedMinutes`/`limitMinutes` là phút tương đương, `retryAt` = đầu tháng sau). Body thành công không đổi. |
| `POST /api/transcript-docs/{key}/share` | 403 khi tài khoản bị khoá. |
| `POST /api/transcript-docs/{key}/bookmarks` | 429 khi vượt 120 lần mỗi phút. |
| Hành vi | Liên kết chỉ xong sau khi người dùng bấm "Xác nhận liên kết" trong chat, nên web nên tiếp tục poll `GET /api/chat/link` tới khi `linked: true`. Xoá đoạn ghi âm thì `summary` của phiên biến mất. |

Kiểm tra: `cargo clippy --workspace --all-targets -- -D warnings` sạch; `cargo test --workspace` đạt (scribe-be 80 test, 9 test Mongo/OpenAI thật để `--ignored`); 4 test Mongo liên quan (token liên kết, sổ ngày và sổ tháng, vòng đời doc, trần dấu mốc) chạy đạt trên DB dev. Chưa gọi thật bot và OpenAI cho phần sửa này.
