# Review: chống lạm dụng gói free (thay đổi chưa commit, `server/scribe-be`)

Ngày 2026-10-16 (Asia/Saigon). Phạm vi: `git diff` 12 file + `src/notify.rs` mới (~456 dòng thêm / 85 xoá).
Không chạy lại clippy/test (đã được xác nhận: clippy sạch, 33 unit, 5 Mongo thật, e2e 12/12).

## Tổng quan

Phần sổ sách cốt lõi đúng: `reserve` và `settle` cùng đi qua `Charge::books()` nên giữ chỗ, hoàn và quyết toán
luôn chạm cùng một tập sổ. Doc lượt cũ (không có `day`/`ip`) ra đúng tập sổ cũ (tháng user, quỹ free, tổng).
Phần giữ thêm ở worker (`reserve_shortfall`) dùng `t.charge()`, nên trúng đúng sổ ngày/IP của ngày xếp hàng,
kể cả khi chạy qua nửa đêm. Không thấy rò hay đếm đôi trên đường chạy bình thường. Các lỗ còn lại nằm ở đường
lỗi, ở việc khoá tài khoản chưa chặn được việc đang chạy, và ở cách chia sổ IP.

## Cao

### H1. Khoá tài khoản không dừng các lượt đã xếp hàng: tiền OpenAI vẫn bị chi
- `src/admin.rs:103-115` (`block`): chỉ đặt `blocked` và xoá session; không đụng tới `transcripts` đang `queued`/`running`.
- `src/models/transcript.rs:205` (`claim_next`), `src/transcribe/mod.rs:484` (`run`) và `transcribe_file`: không kiểm tra `blocked`.
- Hậu quả: một người dùng bị khoá vì lạm dụng vẫn được chép hết hàng đợi của họ (tối đa `MAX_FILES = 200` file mỗi
  request; gói plus giữ được tới 3000 phút/tháng). Khoá là công cụ ứng cứu sự cố, nên đúng lúc cần thì lại không cắt được chi phí.
- Cách sửa: khi `block`, đóng mọi lượt còn mở của user bằng `finish_failed` (đường có sẵn này quyết toán đúng một lần),
  hoặc rẻ hơn là gọi `user::find_profile` trong `transcribe_file` trước `reserve_shortfall` và dừng nếu `blocked`
  (lỗi này cũng đi qua `close_run`/quyết toán). Nên làm cả hai.

## Trung bình

### M1. `settle` dừng ở sổ đầu tiên lỗi, và lệnh đối soát không sửa được sổ ngày/IP
- `src/models/usage.rs:229-233`: `add(db, &id, delta).await?` trong vòng lặp. Mongo lỗi ở sổ thứ k thì các sổ
  trước đã cộng, các sổ sau thì chưa.
- `src/models/transcript.rs:232-245`: `settle_run` đặt `settled: true` **trước** khi gọi `settle_closed`, nên
  không có lần thử lại. Đường hoàn ở `create` (`src/transcribe/mod.rs:254`) cũng chỉ log.
- `src/admin.rs:142-199` (`recompute`): chỉ dựng lại `u:`, `free:`, `all:`. Sổ `d:` và `ip:` lệch thì cứ lệch tới hết
  ngày. Nếu lệch lên, người dùng free thật (và mọi tài khoản cùng IP) bị chặn tới 0 giờ. Nếu lệch xuống, họ được
  dùng quá giới hạn ngày.
- Trước đây có 3 sổ, giờ có 5, nên khả năng hỏng giữa chừng cao hơn.
- Cách sửa: chạy hết các sổ, log riêng từng sổ hỏng (id + delta), rồi trả lỗi tổng hợp. Ghi trong tài liệu rằng
  `recompute` không sửa sổ ngày/IP (hoặc mở rộng nó).

### M2. Lượt dài hơn giới hạn ngày/IP không bao giờ chạy được, nhưng lỗi bảo "mở lại lúc 0 giờ"
- `src/transcribe/mod.rs:133-142`: `DAILY_LIMIT`/`IP_LIMIT` luôn trả `retryAt = next_day(now)`.
- `src/models/usage.rs:176-186` (`take`): điều kiện `seconds <= limit - need`. Nếu `need > free_daily_seconds`
  (mặc định 1800 giây), lượt bị chặn ngay cả khi sổ đang là 0.
- Hậu quả: một bản ghi 31 phút, hoặc nhiều bản ghi chọn cùng lúc cộng lại hơn 30 phút, bị chặn mãi, dù hạn mức
  tháng của gói free là 120 phút. Client tin `retryAt` sẽ thử lại vô ích, và mỗi lần thử còn tăng `count_hit`,
  kéo theo tin Telegram "lần thứ 3".
- Cách sửa: khi `need > limit_of(blocked, ..)`, trả một mã riêng (vd `TOO_LONG_FOR_FREE`) không kèm `retryAt`, và
  câu báo nói rõ bản ghi dài hơn giới hạn ngày. Có cho bản ghi dài vượt giới hạn ngày hay không là quyết định sản phẩm.

### M3. Sổ IP dễ né trên IPv6 và dual-stack; dễ chặn nhầm khi nhiều người chung CGNAT
- `src/models/usage.rs:51-62` (`ip_bucket`): IPv6 gộp theo /64. Nhà mạng gia đình và VPS thường cấp /56 hoặc /48,
  nên một người tự xoay được 256 tới 65536 sổ IP. Một máy dual-stack có sẵn hai sổ (IPv4 + /64), tức 120 phút/ngày.
- Ngược lại, IPv4 lấy nguyên địa chỉ: người dùng di động sau CGNAT chung một IP thì chung 60 phút/ngày.
- Lớp chặn thật sự vẫn là giới hạn ngày từng tài khoản + chờ 24 giờ + quỹ free, nên đây là hạn chế thiết kế chứ
  không phải lỗi code. Nên ghi vào plan để không ai coi sổ IP là chốt chặn cứng. Có thể cân nhắc gộp IPv6 theo /56
  (đổi lại dễ chặn nhầm hơn).

## Thấp

- **L1.** `src/models/user.rs:134`: `blocked = d.get_document("blocked").is_ok()`. Nếu ai đó đặt `blocked: true`
  (kiểu boolean) bằng mongo shell hoặc script, tài khoản vẫn được coi là **không** bị khoá (fail-open). Nên coi mọi
  giá trị khác `null`/`false` là đã khoá.
- **L2.** `src/main.rs:67-69` và `src/admin.rs:32`: reqwest client không có timeout. Telegram treo thì các task
  `notify::send` treo theo, và `admin notify-test` treo mãi. Nên thêm `.timeout(...)` khi gửi trong `deliver`.
- **L3.** `src/notify.rs:32`: `tracing::warn!(message = %text, "…{e}")`. `message` là tên field dành riêng của
  tracing, nên trường bị trùng và log khó đọc. Nên đổi thành `text = %text`. (Token không lộ ở đây: lỗi gửi dùng
  `without_url()`, lỗi đọc tệp chỉ in `e.kind()`, đã kiểm.)
- **L4.** `src/config.rs:70-71`: giá trị `0` mang hai nghĩa khác nhau. `TRANSCRIBE_FREE_DAILY_MINUTES=0` hoặc
  `..._IP_DAILY_MINUTES=0` chặn toàn bộ người dùng free, còn `TRANSCRIBE_FREE_DELAY_HOURS=0` thì tắt chờ. Người cấu
  hình đặt 0 để "tắt" sẽ khoá cả gói free. Nên ghi rõ hoặc thống nhất một nghĩa.
- **L5.** `src/models/usage.rs:176-177` và `count_hit` (`:245-261`): mỗi ngày sinh doc `d:<user>:<ngày>`,
  `ip:<ip>:<ngày>` và `hit:*`, kể cả ở bước bị chặn (`ensure` upsert trước khi kiểm). Không có TTL hay chỗ dọn, nên
  hai collection phình theo số người × số ngày. Dữ liệu nhỏ, nhưng nên có TTL trên `updatedAt`/`at` cho các prefix theo ngày.
- **L6.** `src/transcribe/mod.rs:170-175`: tin Telegram chứa email và IP người dùng (PII gửi qua Telegram). Bot thuộc
  chủ sản phẩm nên chấp nhận được, chỉ cần ghi nhận.
- **L7.** `src/transcribe/mod.rs:120-123` (`vn_time`) dựng lại múi +07:00, trùng với `usage::vietnam()`. Nên dùng chung.

## Đã kiểm, không phải lỗi
- Doc cũ không có `day`/`ip`: `TranscriptDoc::charge()` (`transcript.rs:79-87`) ra đúng tập sổ đã giữ lúc đầu,
  khi quyết toán, khi chạy `requeue_interrupted` và khi giữ thêm.
- Giữ thêm ở worker: `extra` được giữ trên đủ 5 sổ, có giới hạn. Nếu `add_charge` thất bại thì hoàn đúng tập sổ đó
  (`mod.rs:561-566`). Qua nửa đêm vẫn tính vào sổ của ngày xếp hàng, nhất quán với lúc quyết toán.
- Đua nhau: mỗi `take` là một lệnh có điều kiện trên một doc, nên không vượt giới hạn. Phần đã giữ được hoàn đủ khi
  bị chặn. Khi tranh chấp có thể chặn nhầm tạm thời (đã giữ rồi hoàn), nhưng không rò.
- IP: `CLIENT_IP` được đặt ở lớp ngoài cùng (`routes.rs:82`). nginx ghi đè `X-Real-IP` bằng `$remote_addr` sau
  `real_ip_header CF-Connecting-IP` và chỉ tin dải IP Cloudflare, nên client không giả được IP. Nếu gọi thẳng vào
  origin thì vẫn ra IP thật. Backend chỉ bind 127.0.0.1, nên nhánh dự phòng XFF trong `client_ip` không bị gọi
  được từ ngoài.
- Khoá tài khoản: `create` đọc thẳng `blocked` từ DB, nên chép lời không có cửa sổ 30 giây. Với các endpoint khác,
  `require_auth` kiểm lại khi cache hết hạn (≤30 giây). Đăng nhập web/native và `native_refresh` đều qua `admit`, nên
  bị từ chối và thiết bị bị thu hồi.
- Thông báo: `count_hit == 3` dùng `$inc` nguyên tử, nên mỗi khoá chỉ báo một lần mỗi ngày. Các mốc tháng được chặn
  trùng bằng `mark_once`. Báo key OpenAI hỏng chỉ một lần mỗi process (`trip`), và nội dung là hằng số, không chứa key.

## Việc nên làm (theo thứ tự)
1. H1: dừng hoặc đóng các lượt còn mở của user bị khoá (trong `block` và/hoặc trong worker).
2. M1: `settle` chạy hết các sổ, log từng sổ lỗi; ghi rõ `recompute` không sửa sổ ngày/IP.
3. M2: thêm mã lỗi riêng cho lượt dài hơn giới hạn ngày/IP, không kèm `retryAt`.
4. L1, L2, L4: các chỉnh nhỏ (đọc `blocked` không fail-open, timeout cho reqwest, nghĩa của giá trị 0).

## Câu hỏi còn mở
- Gói free có được chép bản ghi dài hơn 30 phút không? Câu trả lời quyết định cách sửa M2.
- Chặn nhầm vì CGNAT (M3) có chấp nhận được không, hay nới giới hạn IP / gộp IPv6 theo /56?

Status: DONE_WITH_CONCERNS
Summary: Sổ phút giữ/hoàn/quyết toán nhất quán trên mọi sổ, kể cả doc cũ và phần giữ thêm ở worker. Còn 1 lỗi Cao (khoá tài khoản không dừng các lượt đã xếp hàng nên vẫn tốn tiền OpenAI) và 3 lỗi Trung bình (`settle` hỏng giữa chừng không sửa được trên sổ ngày/IP, `retryAt` sai cho lượt dài hơn giới hạn ngày, sổ IPv6 /64 dễ né).
