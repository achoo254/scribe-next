---
title: Chép lời bằng ElevenLabs + phân biệt người nói
status: completed
created: 2026-10-17
tier: L
---

# Chép lời bằng ElevenLabs + phân biệt người nói

OpenAI ngừng `gpt-4o-mini-transcribe` ngày 26/02/2027. Chủ dự án chốt (17/10, sau bài nghe mù trong
`plans/dattqh/reports/stt-bench-261017-1355-openai.md`) chuyển chép lời **theo tệp** sang ElevenLabs Scribe v2, kèm phân
biệt người nói. Nghiên cứu dữ liệu: `plans/dattqh/reports/researcher-261017-1240-speaker-diarization.md`.

## Quyết định (chủ dự án + Jev, chốt)
1. **Nhà cung cấp:** biến build `TRANSCRIBE_PROVIDER` = `elevenlabs` (mặc định) | `openai`. Luồng OpenAI giữ nguyên làm
   dự phòng một bản phát hành. Key ElevenLabs trong tệp `ELEVENLABS_KEY_PATH` (prod gợi ý `/opt/scribe-be/.elevenlabs-key`,
   600), đọc một lần lúc khởi động. Key OpenAI (`OPENAI_KEY_PATH`) vẫn cần cho tóm tắt, hỏi đáp, dịch và chép trực tiếp.
   Key hỏng (ElevenLabs 401/402/403 hoặc `quota_exceeded`) ⟹ cờ tạm ngưng chép lời như OpenAI: ngắt, audit
   `transcript.key_invalid`, báo bot. Key ElevenLabs hỏng **không** ngắt tóm tắt.
2. **Đơn vị gửi:** bản ghi trình duyệt và tệp tải lên = một request cho cả tệp. Phiên iPhone (`s:<sessionId>`) = **một
   request cho cả phiên**: ghép các đoạn theo `index` bằng ffmpeg (concat, nén Opus mono 16 kHz 32 kbps), mốc của đoạn
   sau = tổng thời lượng các đoạn trước (cùng luật cộng dồn với màn hình đọc và xuất tệp), rồi chia từ về lại từng đoạn
   theo mốc nên `transcripts` (mỗi file một doc), câu sửa, index tìm kiếm, xuất tệp, dấu mốc, biên bản giữ nguyên.
   - Bấm "Chép lời" với bất kỳ đoạn nào của phiên ⟹ server xếp **cả phiên** (mọi đoạn hiện có) với cùng một `runId`;
     worker lấy một đoạn ra thì gom các đoạn cùng `runId` thành một lượt.
   - "Phiên đã xong": server không có tín hiệu kết thúc phiên từ iPhone. Lượt phiên **hoãn 20 giây** (tối đa 10 phút
     tính từ lúc đoạn đang tải bắt đầu) khi phiên còn một đoạn đang tải lên (`recording_sessions` mở, tạo trong 10 phút).
   - **Đoạn tới muộn** (tải lên sau khi phiên đã chép): đoạn đó hiện "chưa chép"; bấm chép phần còn lại ⟹ **chép lại cả
     phiên**, tính phút cả phiên lần nữa (vd. phiên 60 phút có 1 đoạn muộn ⟹ thêm 61 phút ≈ 0,22 USD). Màn hình xác nhận
     hiện số phút cả phiên. Đây là cái giá để nhãn người nói nhất quán trong cả phiên.
   - Phiên không có đoạn nào đang chạy mới xếp được (một đoạn đang `queued/running` ⟹ từ chối cả phiên, hoàn phút).
3. **Câu:** dựng từ timestamp từng từ của ElevenLabs (mốc thật, bỏ mốc ước lượng). Tách tại `. ? ! …`, tại chỗ đổi người
   nói, tại khoảng ngừng ≥ 1,2 giây, và câu dài hơn 120 ký tự tách tiếp ở dấu phẩy rồi theo từ (cùng độ dài như hiện
   nay). Mỗi câu có `speaker` (`s0`, `s1`… đánh số lại theo thứ tự xuất hiện trong lượt).
4. **Người nói:** `transcript_docs.speakers = {names: {s0: "Người 1", …}}`, đặt lại về nhãn mặc định khi một lượt chép xong
   (lượt mới gán lại id). Đọc: id không có tên lưu ⟹ "Người N+1". Đổi tên: `PATCH …/speakers` áp cho mọi chỗ; v1 chỉ đổi
   tên, không gán lại người nói cho từng câu. Màn hình đọc hiện nhãn khi người nói đổi; hộp "Người nói" liệt kê và đổi tên.
   Xuất: TXT/MD/DOCX thêm "Tên: " ở chỗ đổi người nói; SRT thêm "Tên: " mỗi cue; VTT `<v Tên>`. Biên bản: đầu vào có tên
   người nói, băm nguồn tính **không** gồm tên ⟹ đổi tên không làm biên bản "cũ". Index tìm kiếm không đổi.
5. **Đo lường:** giá theo nhà cung cấp — ElevenLabs 0,22/60 USD mỗi phút, OpenAI 0,003 (đổi trần USD ra giây và quy đổi chi
   phí tóm tắt). Ghi giây đã gửi TRƯỚC khi gửi (mỗi đoạn theo thời lượng đo lại), sau khi ElevenLabs trả
   `audio_duration_secs` thì chỉnh về số đó. Sổ phút, hạn mức, trần giữ nguyên hình dạng. Luồng ElevenLabs không dùng
   "phát hiện lặp + thử lại temperature" (giữ cho OpenAI). Tệp/phiên hoàn toàn im lặng (đỉnh < −30 dB) không gửi, không
   tính phút; im lặng bên trong phiên ghép thì vẫn gửi.
6. **Chép trực tiếp** trên iPhone vẫn dùng OpenAI Realtime (`gpt-4o-mini-transcribe`, Jev 0.59), không đụng. **Phải
   chuyển trước 26/02/2027** (mục riêng trong roadmap).
7. **Tiết kiệm (chủ dự án, 17/10, gói Starter $6/tháng ≈ 27 giờ):** trần giờ ElevenLabs riêng `ELEVENLABS_MONTHLY_HOURS`
   (mặc định 25), sổ `el:<tháng giờ VN>` trong `transcribe_usage`, giữ chỗ trước khi gửi theo thời lượng tệp/phiên, quyết
   toán theo `audio_duration_secs`. Hết trần ⟹ 403 `PROVIDER_HOURS` "Đã dùng hết giờ chép lời của tháng này, mở lại ngày
   1/MM". Độc lập với trần USD (trần USD không hạ). `admin usage` in mục `elevenlabs`; `admin elevenlabs-quota [--clear]`.
   ElevenLabs hết credit (402 `insufficient_credits`, hoặc 401 kèm `quota_exceeded`) là **hết quota tháng**, không phải
   key hỏng: dấu `el_exhausted:<tháng>` chặn tới tháng sau hoặc tới khi quản trị gỡ, audit `transcript.provider_quota`, báo
   bot một lần. Audio gửi đi là Opus mono 16 kHz 32 kbps (≈ 14 MB/giờ, nhỏ hơn AAC gốc), không bao giờ gửi tệp im lặng.
8. `tag_audio_events=false` (không có "[tiếng cười]"), `timestamps_granularity=word`, `diarize=true`, `language_code=vi`,
   không đặt `num_speakers` (để model tự đoán, tối đa 32). `keyterms` để sau (phụ phí 20%).

## Hợp đồng API ElevenLabs (kiểm tài liệu 17/10/2026)
- `POST https://api.elevenlabs.io/v1/speech-to-text`, header `xi-api-key`, multipart: `model_id=scribe_v2`, `file`
  (≤ 5 GB, ≥ 100 ms), `language_code=vi`, `diarize=true`, `timestamps_granularity=word`, `tag_audio_events=false`.
  Tải tệp lên (không dùng `cloud_storage_url`/`source_url`: server không có URL công khai cho audio).
- 200: `{language_code, language_probability, text, words:[{text, type: word|spacing|audio_event, start, end,
  speaker_id, logprob}], audio_duration_secs?, transcription_id?}`.
- Lỗi `{detail:{type, code, message, status, request_id}}`. 401 (`invalid_api_key`…), 402 `insufficient_credits`, 403
  ⟹ key hỏng (tạm ngưng). `quota_exceeded` (cũ) ⟹ hết credit. 429 `rate_limit_exceeded` / `concurrent_limit_exceeded` /
  `system_busy`, 500, 502, 503 ⟹ thử lại 3 lần, chờ 3/6 giây. 400/422 ⟹ lỗi tệp, báo người dùng.
- Server chạy tuần tự một lượt mỗi lúc (hàng đợi sẵn có) nên không vượt giới hạn đồng thời của gói.

## Hợp đồng API của app
| Method | Route | Thay đổi |
|---|---|---|
| POST | `/api/transcripts` `{fileIds}` | Với ElevenLabs: id thuộc phiên iPhone được mở rộng ra mọi đoạn của phiên, giữ chỗ phút cả phiên. Trả thêm `files` (số file đã xếp). |
| GET | `/api/transcripts`, `/api/transcript-docs/{key}` | câu có thêm `speaker` (`"s0"`…, không có với bản OpenAI) |
| GET | `/api/transcript-docs/{key}` | thêm `speakers: {names: {s0: "Người 1"}}` (chỉ id xuất hiện trong câu) |
| PATCH | `/api/transcript-docs/{key}/speakers` `{names: {s0: "Lan"}}` | đổi tên; tên rỗng ⟹ về nhãn mặc định; tối đa 40 ký tự; id dạng `s0`…`s99`. 200 `{names}` |
| GET | `/api/me` | `transcription.wholeSession: true` khi chép cả phiên (web gửi đủ đoạn và hiện đúng số phút) |
| GET | `/api/transcripts/export` | tên người nói trong TXT/MD/DOCX/SRT/VTT |

## Phase
| Phase | Tier | Trạng thái |
|---|---|---|
| 1. Backend: cấu hình nhà cung cấp, client ElevenLabs, dựng câu từ từ, lượt phiên (ghép, chia lại), người nói, xuất, biên bản, đo lường, trần giờ, test Rust | L | done |
| 2. Web: nhãn người nói, hộp đổi tên, gửi cả phiên, vitest, e2e | M | done |
| 3. Tài liệu, cổng kiểm tra, chạy thật một lần, commit, push | S | done |

## Kết quả (17/10)
- Chạy thật qua đúng đường của server (`whole::tests::live_elevenlabs_transcribes_a_two_segment_session`): 2 đoạn × 60 giây
  cắt từ một cuộc họp thật, ghép một request 460 KB, 16 + 17 câu chia đúng về hai đoạn, 1 người nói trong đoạn đó,
  ElevenLabs tính 120,02 giây ≈ 0,0073 USD, 6,6 giây. Tổng chi phí thử thật trong gói ≈ 0,026 USD (lần thử đầu rơi vào
  đoạn chưa họp, ElevenLabs trả 0 từ — luồng xử lý đúng: đoạn không có câu).
- Cổng: clippy sạch; `cargo test --workspace` 169 qua (21 ignored); Mongo dev 16 ignored qua; web lint/typecheck sạch,
  vitest 354 qua, build sạch, Playwright 93 qua (7 skip sẵn có).

## Tiêu chí nghiệm thu
- `TRANSCRIBE_PROVIDER` không đặt ⟹ ElevenLabs; `openai` ⟹ luồng cũ chạy y như trước (test cũ vẫn qua).
- Một tệp ⟹ một request; phiên iPhone ⟹ một request, câu chia đúng đoạn với mốc tính trong đoạn.
- Câu có mốc thật từ timestamp từ, có `speaker`; nhãn "Người N" theo thứ tự xuất hiện.
- Đổi tên người nói áp cho màn hình đọc, mọi định dạng xuất, đầu vào biên bản; biên bản không bị đánh dấu cũ.
- Key ElevenLabs 401/403 ⟹ tạm ngưng chép lời, audit, báo bot; tóm tắt vẫn chạy.
- Giá 0,22/60 USD mỗi phút; giây tính theo `audio_duration_secs`.
- Test: Rust (từ → câu, người nói, chia phiên về đoạn, xuất có người nói, giá), HTTP giả lập ElevenLabs, Mongo dev
  (ignored), chạy thật một lần < 0,05 USD; vitest + e2e cho nhãn và đổi tên.
- Cổng: clippy `-D warnings`, `cargo test --workspace`, web lint/typecheck/test/build/e2e đều sạch.

## Việc của chủ sản phẩm
- Mua gói trả phí ElevenLabs (gói miễn phí không được dùng thương mại) và tạo key riêng cho server.
- Cài tệp key lên server qua stdin (xem `docs/deploy-web-production.md`), rồi build lại với `TRANSCRIBE_PROVIDER` (mặc định
  đã là `elevenlabs`) và khởi động lại.
- Chép trực tiếp (OpenAI Realtime) phải chuyển trước 26/02/2027.
