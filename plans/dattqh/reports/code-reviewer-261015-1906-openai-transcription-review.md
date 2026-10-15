# Review: Chép lời bằng OpenAI (BYOK) — thay đổi chưa commit trên `main`

Ngày: 2026-10-15 · Người review: code-reviewer · Spec: `plans/260728-1609-onescribe-ios-app-ghi-nen-khoa-man-hinh/phase-06-ban-hoan-chinh-sau-buoi-ghi.md` (T1–T6 coi là quyết định chủ sở hữu, chỉ kiểm code có làm đúng không)

## Phạm vi

- Backend: `src/transcribe/{mod,chunking,openai,export}.rs` (~924 dòng), `src/models/transcript.rs` (236), diff ở `routes/state/config/main/auth/files/jobs/worker/models/file.rs`, `build.rs`, `Cargo.toml`.
- Web: `features/transcript/*`, `pages/record/{transcript-panel,api-key-modal,recording-row,session-row,recording-history}.tsx`, `components/media-player.tsx`, `auth/*`, `lib/{native,download}.ts`, `features/files/files-api.ts`.
- iOS: `SecretsPlugin.swift`, `SessionAuth.swift`, `MainViewController.swift`.
- Không chạy lại build/test (đã được xác nhận). Chỉ chạy một thử ffmpeg cục bộ (không gọi OpenAI) để kiểm hành vi cắt mảnh quá cuối file.

## Đánh giá chung

Đường đi của key nhìn chung sạch: key chỉ đi qua header `Authorization` (`openai.rs:91`), không nằm trong argv ffmpeg, không vào doc `transcripts` (`transcript.rs:63-74`), audit `transcript.create` chỉ ghi số file, access log chỉ ghi path, lỗi reqwest không được in, 401 được map sang hằng số. Ownership đúng trên cả 3 route. Allowlist được kiểm ở server, rỗng = tắt, so sánh không phân biệt hoa thường. `fail_unfinished` được `await` trước khi mở cổng nghe (`main.rs:88-97`). Panic một lượt không giết vòng hàng đợi.

Lỗ hổng chính nằm ở **vòng đời key trong RAM**: có hai đường để key ở lại `PendingKeys` vĩnh viễn (tới khi restart), trái với T3 "backend chỉ giữ trong RAM tới khi chép xong". Ngoài ra độ dài file dùng để cắt mảnh có thể đến từ client, làm hỏng cả file hoặc cắt cụt bản chép mà không báo.

---

## Critical

Không có lỗi rò key ra Mongo/log/argv hay vượt quyền người dùng.

## High

### H1. Race giữa `enqueue` và `put`: lượt chép báo lỗi sai VÀ key bị kẹt vĩnh viễn trong RAM
`server/onescribe-be/src/transcribe/mod.rs:111-115` (cùng `:246-249`)

```rust
transcript::enqueue(&state.db, oid, &user.0, &run_id, openai::MODEL).await?;   // doc đã "queued" trên Mongo
state.transcribe_keys.put(oid, run_id, key);                                     // key vào RAM SAU đó
```

Kịch bản: worker vừa xong một file khác (hoặc hết `timeout` 5 giây) và gọi `claim_next` đúng lúc Mongo đã ghi doc `queued` nhưng phản hồi `update_one` chưa về tới handler (cửa sổ = một RTT Mongo, lặp lại cho mỗi file trong vòng 200 file). Worker claim doc → `take()` trả `None` → file bị đánh dấu failed với thông báo sai "Máy chủ đã khởi động lại…". Sau đó handler mới `put` key → **không còn ai `take` key đó nữa**: key nằm trong RAM tới khi tiến trình dừng.

Sửa: đưa `put` lên trước `enqueue`, và gỡ key nếu `enqueue` lỗi:
```rust
state.transcribe_keys.put(oid, run_id.clone(), key);
if let Err(e) = transcript::enqueue(&state.db, oid, &user.0, &run_id, openai::MODEL).await {
    state.transcribe_keys.take(oid, &run_id);
    return Err(e);
}
```

### H2. Key không bao giờ được dọn khi file bị xoá lúc đang chờ (xoá tay, retention, cắt bản ghi)
`server/onescribe-be/src/models/transcript.rs:175-181`, `models/file.rs:213-220`, `transcribe/mod.rs:43-65`

`file::delete_doc` → `transcript::delete` xoá doc `transcripts`, nhưng `PendingKeys` không hề biết. Doc đã mất nên không bao giờ được claim → entry `(run_id, ApiKey)` ở lại mãi. Cũng xảy ra với job "Cắt bản ghi" (`jobs.rs:274`) và retention sweep (`retention.rs:65`). Cùng loại với H1: vi phạm T3 và là rò bộ nhớ chứa bí mật (một phiên 180 đoạn = 180 bản sao key).

Sửa (đơn giản, không cần đưa `AppState` vào tầng model): ghi kèm `Instant` khi `put`; ở nhánh `Ok(None)` của vòng worker (hàng đợi rỗng) xoá mọi entry cũ hơn ~60 giây — khi hàng đợi rỗng thì không key hợp lệ nào phải chờ lâu như vậy. Kết hợp với H1 (put trước enqueue) thì ngưỡng tuổi tránh được việc xoá nhầm key vừa put. Nên đổi sang `Arc<ApiKey>` dùng chung cho cả lô thay vì `key.to_string()` cho từng file.

## Medium

### M1. Độ dài file (có thể do client gửi) quyết định cách cắt mảnh: dài hơn thực tế → hỏng cả file, ngắn hơn → mất chữ không báo
`transcribe/mod.rs:278-281`, `openai.rs:22-54`, nguồn dữ liệu `recordings.rs:206-208`

`duration_s` của bản ghi là giá trị ffprobe, **nếu ffprobe trả 0 thì lấy `req.duration_s` từ client** (webm của MediaRecorder thường không có duration). Vì vậy:
- Client báo dài hơn thực tế: `speech_from_silences` kéo vùng nói tới `duration` → có mảnh nằm sau EOF. Đã thử: `ffmpeg -ss 10 -t 5 -i <file 5s> … -f ogg pipe:1` trả **exit 0, 257 byte** (chỉ header Ogg), lọt qua kiểm `out.stdout.is_empty()` ở `openai.rs:49` → gửi OpenAI → 400 → **cả file failed** (`texts.push(text?)`).
- Client báo ngắn hơn: phần đuôi không được chép, trạng thái vẫn "done".
- Duration = 0 ở cả hai nguồn: không có mảnh nào → `done` với chữ rỗng, người dùng thấy "Đã chép lời" nhưng không có dòng nào.

Sửa: lấy độ dài thật từ chính lượt giải mã `silencedetect` (bỏ `-nostats` và đọc `time=` cuối, hoặc dùng `-progress pipe:2` và lấy `out_time_ms` cuối), rồi dùng `min/thay thế` cho `duration`. Trong `encode_chunk`, coi output dưới ~1 KB là mảnh rỗng (bỏ qua, không gửi). Khi không có mảnh nào thì báo failed "không tìm thấy tiếng nói" thay vì done rỗng.

### M2. Mảnh ngắn luôn bị coi là "lặp" → trả tiền 3 lần và giữ bản ở temperature 0.8
`chunking.rs:92-102`, `openai.rs:61-75`, `chunking.rs:65-84`

`looks_looped` kiểm tốc độ `words / (seconds/60) > 260`. Mảnh đuôi ngắn (plan_chunks có thể sinh mảnh cuối ~0,2–5 giây khi điểm cắt rơi gần cuối) chỉ cần 2–3 từ là vượt ngưỡng → gọi lại 3 lần. Vòng `for temperature in RETRY_TEMPS` giữ **lần cuối** (`0.8`) kể cả khi lần này vẫn "lặp" — tức là ưu tiên bản ngẫu nhiên nhất. Tốn gấp 3 tiền của người dùng và tăng nguy cơ bịa chữ.

Sửa: chỉ áp ngưỡng tốc độ khi `seconds >= ~10`; giữ bản có điểm lặp thấp nhất (hoặc lần đầu) khi cả ba đều bị đánh dấu; trong `plan_chunks` không chọn điểm cắt khiến phần còn lại < `CHUNK_MIN`/vài giây (gộp đuôi vào mảnh trước, vẫn ≤ CHUNK_MAX + vài giây chấp nhận được).

### M3. Lượt cũ vẫn chạy tiếp sau khi bị xếp lại hoặc file bị xoá → tiêu tiền người dùng vô ích
`transcript.rs:95-101`, `mod.rs:318-324`

`update_run` lọc `{_id, runId}` nên kết quả cũ không đè được (đúng), nhưng nó luôn trả `Ok(())` dù `matched_count == 0`. Worker không biết mình đã bị thay thế, tiếp tục gọi OpenAI cho tới hết file (file 80 phút ≈ 120 mảnh), rồi lượt mới phải chờ phía sau (hàng đợi tuần tự). Bấm "Chép lời" hai lần nhanh (nút vẫn hiện trong lúc `await loadApiKey()`, `transcript-panel.tsx:80-87`) là một cách kích hoạt.

Sửa: `update_run` trả `matched_count`; trong vòng `while let Some(text)` nếu `set_progress` khớp 0 doc thì `return Err("đã huỷ")` (drop stream sẽ huỷ các request đang bay).

### M4. Lỗi ghi kết quả bị nuốt → doc kẹt `running`, web poll 2 giây mãi
`mod.rs:229`, `:253`, `:263` (`let _ = transcript::finish_*`)

Mongo chập chờn đúng lúc ghi kết quả → doc ở `running` tới lần restart; `useTranscriptsQuery` poll mỗi 2 giây không dừng. Sửa: log lỗi và thử lại 1–2 lần với backoff; tối thiểu là `tracing::error!`.

### M5. Không có giới hạn thời gian cho một file; ffmpeg không `kill_on_drop`
`openai.rs:24-29`, `:41-48`, `:77-123`, `mod.rs:227`

Mỗi mảnh tệ nhất: 3 temperature × 3 lần thử × 180 giây + backoff ≈ 27+ phút; ffmpeg không có timeout. Hàng đợi chạy tuần tự nên một file treo chặn mọi người dùng khác (hiện chỉ một email được dùng nên ảnh hưởng thấp, nhưng sẽ lộ khi mở rộng T2). Khi một mảnh lỗi, `buffered` bị drop nhưng tiến trình ffmpeg con vẫn chạy tiếp vì `tokio::process::Command` mặc định `kill_on_drop(false)`.

Sửa: `.kill_on_drop(true)` cho hai lệnh ffmpeg; bọc `run` trong `tokio::time::timeout` theo độ dài file (ví dụ `max(10 phút, 2 × duration)`).

### M6. Tải dữ liệu thừa khi poll và N+1 khi xuất
- `mod.rs:149-161` + `transcript-queries.ts:32-38`: mỗi 2 giây GET trả **toàn bộ** `text` + `sentences` của mọi file trong phiên (chữ bị gửi hai lần: `text` và `sentences`). Phiên 180 đoạn đã chép xong một phần → vài trăm KB mỗi 2 giây trên mạng di động. Sửa: bỏ `text` khỏi GET (web không dùng — `merge-transcripts.ts` chỉ đọc `sentences`), hoặc thêm `?fields=status` khi đang poll.
- `mod.rs:183-191`: `export` gọi `file::get_owned` tuần tự cho từng id (tới 200 truy vấn). Sửa: một truy vấn `find({_id: {$in: ids}, userId})` rồi tra theo map. Tương tự `create` (`mod.rs:101-110`).

### M7. Phiên trên 200 đoạn hỏng hẳn trên web; mọi lỗi 400 đều bật hộp nhập key
`transcript-panel.tsx:66-71`, `:86-93`; `mod.rs:96-98`, `:135-137`

Phiên dài hơn 200 phút (đoạn 1 phút) → cả GET (`parse_ids`) lẫn POST trả 400. Panel không tải được trạng thái, còn `start()` coi mọi 400 là "key sai dạng" và mở modal nhập key với lý do "chọn từ 1 đến 200 bản ghi" — người dùng nhập lại key vô ích. Sửa: web chia lô ≤200 id cho cả GET và POST; chỉ mở modal khi lỗi thực sự về key (ví dụ backend trả mã lỗi riêng, hoặc kiểm `looksLikeApiKey` ở client trước).

## Low

- **L1. Chi tiết lỗi nội bộ lọt vào trường `error` người dùng thấy và audit** — `mod.rs:276`: `format!("{e:?}")` của `AppError` ra dạng `Internal("find file: <lỗi Mongo>")`. Đường `AppError::into_response` cố ý giấu lỗi nội bộ, ở đây lại lộ qua `GET /api/transcripts`. Sửa: map sang "lỗi nội bộ khi đọc bản ghi" và `tracing::warn!` chi tiết.
- **L2. Mutex bị poison → `put` lặng lẽ không làm gì** — `mod.rs:52`, `:59`: mọi lượt sau đó báo "Máy chủ đã khởi động lại" sai. Dùng `lock().unwrap_or_else(PoisonError::into_inner)`.
- **L3. `Keychain.save` bỏ qua `OSStatus`** — `SecretsPlugin.swift:17-24`, `:81-82`: `set` luôn resolve; nếu `SecItemAdd` lỗi thì lần sau `loadApiKey` trả rỗng và hỏi lại key mà không rõ lý do. Trả về status và `call.reject` khi khác `errSecSuccess`. (Refactor `SessionAuth` giữ nguyên hành vi cũ, không hồi quy.)
- **L4. Không thu gọn được danh sách đoạn khi đang phát** — `session-row.tsx:58`, `:89`: `shown = open || đang phát một đoạn`; bấm "Thu gọn" gọi `setOpen(!shown)` = `setOpen(false)` nhưng `shown` vẫn `true` vì còn đang phát → nút không có tác dụng cho tới khi dừng phát.
- **L5. Tua trước khi media có metadata** — `media-player.tsx:32-42`: effect đặt `currentTime` ngay khi phần tử vừa mount (`readyState` = 0). Chrome lưu thành "default playback start position"; WKWebView cần kiểm trên máy thật. Phòng xa: nếu `media.readyState < 1` thì đặt trong `loadedmetadata` (once). Ngoài ra `seek` còn giữ trong state nên player mount lại (ẩn/hiện) sẽ tua lại về mốc cũ.
- **L6. Lệch hợp đồng tài liệu** — spec ghi `segments: [{start,end,text}]`, code trả `sentences`. Sửa spec cho khớp.
- **L7. Đăng xuất là đường duy nhất xoá key** — phiên hết hạn rồi đăng nhập bằng tài khoản khác trên cùng iPhone sẽ dùng lại key trong Keychain. Hiện chỉ một email được chép lời nên rủi ro thấp; ghi nhận để xử lý khi mở rộng T2 (ví dụ gắn key theo email: account = `openaiApiKey:<email>`).
- **L8. Cắt bản ghi làm mất bản chép lời** — `jobs.rs:274` xoá doc gốc → `transcript::delete`. Đúng spec ("xoá file thì xoá transcript") nhưng người dùng có thể không ngờ; cân nhắc cảnh báo ở UI cắt file.

## Kiểm theo checklist

| Mục | Kết quả |
|---|---|
| Key vào Mongo | Không (`transcript.rs:63-74` không có trường key) |
| Key vào log/tracing/access log/audit | Không. `create` không log; access log chỉ path; TraceLayer mặc định không log body; lỗi reqwest không in; 401 → hằng số |
| Key vào argv | Không — chỉ `bearer_auth` |
| Người dùng khác đọc/dùng key | Không — map khoá theo `_id` file (duy nhất) + `runId`, file đã kiểm ownership |
| Key dọn đúng lúc | **Không** — H1, H2 |
| Ownership `POST/GET/export` | Đúng: `get_owned` từng id + `kind == "recording"`; `list_owned` lọc `userId`; export lọc cả hai |
| Allowlist T2 | Kiểm ở server trong `create`; rỗng = tắt (`config.rs`); `canTranscribe` ở `/api/me` chỉ để ẩn nút |
| `runId` chống ghi đè | Đúng cho kết quả; lượt cũ vẫn tiêu tiền (M3) |
| Restart | `fail_unfinished` chạy trước khi nghe cổng — đúng |
| Panic | Được cô lập bằng `tokio::spawn(...).await` — đúng |
| Xoá file giữa lượt | Ghi kết quả no-op (không upsert) — đúng; key mồ côi nếu đang chờ (H2) |
| Poll dừng | Dừng khi không còn queued/running; không dừng nếu doc kẹt (M4) |
| Đăng xuất xoá key | Có (`auth-context.tsx:79-80`), `deleteSession` nuốt lỗi nên luôn tới bước xoá |

## Hành động đề xuất (theo thứ tự)

1. H1: `put` trước `enqueue`, gỡ key khi enqueue lỗi.
2. H2: dọn key mồ côi theo tuổi khi hàng đợi rỗng; dùng `Arc<ApiKey>` cho cả lô.
3. M1: đo độ dài thật từ lượt silencedetect; bỏ mảnh rỗng; không trả `done` rỗng.
4. M3 + M5: huỷ lượt bị thay thế, `kill_on_drop(true)`, timeout theo file.
5. M2: sửa ngưỡng tốc độ cho mảnh ngắn và cách chọn bản khi thử lại.
6. M4, M6, M7, rồi các mục Low.

## Số liệu

- Type coverage: Rust tĩnh; TS không có `any` trong file mới (đã đọc).
- Test: 14 unit + 2 live (đã xác nhận trước, không chạy lại). Chưa có test cho thứ tự put/enqueue, dọn key mồ côi, độ dài sai.
- Lint: clippy sạch theo xác nhận trước (4 cảnh báo cũ).

## Câu hỏi còn mở

- Biến `TRANSCRIBE_EMAILS` đã được tạo trong GitLab CI (protected) chưa? Nếu chưa, bản deploy tiếp theo sẽ tắt tính năng cho mọi người (đúng thiết kế nhưng dễ tưởng là lỗi).
- Web Mode A (MediaRecorder webm) có thường rơi vào nhánh `duration_s` do client gửi không? Nếu có, M1 nên nâng lên High.
- Đã thử tua câu trên WKWebView (iPhone) với file chưa tải metadata chưa (L5)?
