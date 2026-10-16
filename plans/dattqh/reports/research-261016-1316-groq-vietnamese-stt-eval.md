# Đo chép lời tiếng Việt: Groq Whisper so với OpenAI

Thời điểm đo: 16/10/2026 13:16 (Asia/Saigon), trên máy dev, không đụng prod. Tiếp nối
`research-261016-1154-9router-cost-and-free-stt.md`.

## Kết luận

**Không nên dùng Groq Whisper để chép lời tiếng Việt ở Scribe.** Trên bản ghi họp thật 17 phút, Groq chèn câu bịa
kiểu YouTube ("Các bạn hãy đăng ký kênh…") vào giữa lời nói thật, có lúc thay luôn nội dung của đoạn. Lọc im lặng
không sửa được lỗi này mà còn làm tệ hơn. Ngoài ra Groq chép sai nhiều từ hơn và gần như không có dấu câu. Groq chỉ
hơn ở tốc độ (khoảng 2,5 lần) và ở chỗ miễn phí. Chi tiết ở mục "Đo trên bản ghi họp thật".

## Cách đo

- 3 đoạn do giọng TTS tiếng Việt "Linh" của macOS đọc (~19 giây mỗi đoạn, có văn bản gốc): hội thoại họp, số liệu và
  ngày tháng, từ kỹ thuật tiếng Anh. Thêm 1 file `.webm` thật dài 5,7 giây, gần im lặng (trung bình −52 dB).
- Cả ba model đều dùng tham số giống backend: ogg/opus mono 16 kHz, `language=vi`, `temperature=0`.
- Tính WER/CER sau khi chuẩn hoá NFC, chữ thường và bỏ dấu câu. Script `run.sh` và `score.py` nằm trong thư mục
  scratchpad của phiên, không đưa vào repo.

## Kết quả

| | OpenAI `gpt-4o-mini-transcribe` | Groq `whisper-large-v3-turbo` | Groq `whisper-large-v3` |
|---|---|---|---|
| WER tổng (thô) | 3,9% | 19,9% | 18,8% |
| Hội thoại | 0 lỗi | 2 lỗi chính tả ("vướng mắt", "rồn") | giống turbo |
| Số liệu | 0 lỗi | đúng nội dung, viết bằng chữ số (bị WER phạt oan ~36%) | giống turbo |
| Từ kỹ thuật | 3 chỗ sai (database, API, timeout) | 5 chỗ sai, gồm cả từ Việt ("ẩu cứng", "vi âm") | 5 chỗ sai |
| File im lặng | trả về rỗng | **"Cảm ơn các bạn đã theo dõi và hẹn gặp lại."** | "ừ ừ" |
| Thời gian mỗi đoạn | 1,4–2,2 giây | 0,6–0,9 giây | 0,4–0,7 giây |

WER thô của Groq bị đẩy lên vì chữ số. Kể cả khi bỏ qua phần đó, Groq vẫn sai chính tả tiếng Việt ở những câu
OpenAI chép đúng hoàn toàn. Nguy hiểm nhất là câu tự bịa khi im lặng: bản ghi iPhone của Scribe gồm nhiều đoạn, nên
câu bịa sẽ lọt vào transcript của người dùng.

## Tư vấn của Jev

Hỏi "dựa trên số đo, nên khuyến nghị gì": dùng Groq có điều kiện (lọc im lặng, ghi rõ chất lượng thấp hơn) 0,85;
đo thêm trước khi quyết 0,12; không dùng Groq 0,03.

## Đo lại với lọc im lặng

Thêm mẫu `s4-tron-im-lang`: đoạn hội thoại, rồi 8 giây tiếng nền −48 dB, rồi đoạn số liệu (47 giây).

- **Lọc sau khi nhận không dùng được với Groq.** Gửi `verbose_json` rồi bỏ đoạn có `no_speech_prob > 0,6` hoặc
  `avg_logprob < −1` (ngưỡng mặc định của Whisper). Câu bịa "Cảm ơn các bạn đã theo dõi…" vẫn có
  `no_speech_prob = 0,00` và `avg_logprob = −0,24`, nên không bị lọc.
- **Lọc trước khi gửi bằng ffmpeg thì chặn được câu bịa.** Bước 1: bỏ hẳn đoạn có âm lượng đỉnh dưới −30 dB. Bước 2:
  cắt đoạn dưới −40 dB dài quá 1 giây (`silenceremove`). Chỉ cắt im lặng thì chưa đủ: file thật còn lại 2 giây
  tiếng "tách" và Groq vẫn chép thành "Cảm ơn.". Phải có thêm điều kiện về âm lượng đỉnh.
- 8 giây tiếng nền nằm giữa lời nói không làm model nào bịa chữ, kể cả khi không lọc.
- Lọc không làm Groq chép tiếng Việt tốt hơn bao nhiêu. Đếm lỗi thật bằng tay (bỏ qua khác biệt chữ số và dấu câu):

| Mẫu | OpenAI | Groq turbo, không lọc | Groq turbo, có lọc |
|---|---|---|---|
| Hội thoại | 0 | 2 | 2 |
| Số liệu | 0 | 0 | 0 |
| Kỹ thuật | 3–4 (từ tiếng Anh) | 5 | 4–5 |
| Trộn im lặng | 0 | 2 | 1 |
| File im lặng | rỗng | bịa cả câu | không gửi, rỗng |

WER thô ở mẫu trộn bị lệch: lần này OpenAI cũng viết số bằng chữ số, nên bị phạt khoảng 18% dù không sai chữ nào.
Chạy lại cùng file với `temperature=0` cũng cho kết quả hơi khác (OpenAI lần hai chép "chat lời" thay vì "chép
lời").

Ngưỡng −30 dB có thể bỏ sót người nói nhỏ hoặc ngồi xa micro. Chưa thử trên ghi âm thật.

Jev, hỏi "bước tiếp theo nên là gì": đo thêm trên ghi âm thật trước khi lên plan 0,96; lên plan ngay 0,04; dừng 0,00.

## Đo trên bản ghi họp thật (`Họp anm.m4a`, 17 phút)

Chủ sở hữu đồng ý gửi file này sang Groq và OpenAI. Tôi cắt thành 26 đoạn 40 giây (backend cắt 25–45 giây) và gửi
cùng các đoạn đó cho cả ba cách. Không có văn bản gốc, nên tôi lấy OpenAI làm mốc rồi đọc từng chỗ hai bên khác
nhau. Báo cáo không chép nội dung cuộc họp.

| | OpenAI | Groq turbo | Groq turbo + lọc im lặng |
|---|---|---|---|
| Đoạn có câu bịa kiểu YouTube ("đăng ký kênh", "subscribe…") | 0/26 | 2/26 | **5/26** |
| Đoạn mất gần hết nội dung | 0 | 1 (25 chữ so với 157) | 1 |
| Đoạn lặp vô tận | 1 (2.047 chữ / 40 giây) | 0 | 0 |
| Khác OpenAI ở các đoạn bình thường | — | 32,6% số chữ | 34,2% số chữ |
| Dấu câu | có | gần như không có | gần như không có |
| Thời gian mỗi đoạn | 1,3–2,8 giây (một đoạn 9,4 giây) | 0,6–1,2 giây | 0,6–1,1 giây |

- Trên ghi âm thật, câu bịa của Groq **nằm ngay giữa lời nói**, không chỉ ở chỗ im lặng. Câu bịa có thể thay luôn
  nội dung thật của đoạn đó.
- Lọc im lặng làm tệ hơn: những chỗ nối sau khi cắt có vẻ khiến Whisper bịa thêm. Lọc chỉ có ích với đoạn hoàn toàn im lặng.
- Đọc thử một đoạn bình thường: Groq sai nhiều từ mà OpenAI chép đúng ("gắn hạn", "dâu dài", "class-meat nền đảng"
  thay vì "cloud service nền tảng") và không có dấu câu, nên rất khó đọc.
- OpenAI cũng có lỗi: lặp vô tận ở 1 đoạn (backend đã phát hiện lặp và chép lại, `openai.rs:80`) và bỏ sót phần
  đầu của 1 đoạn mà Groq lại chép được.

Jev, hỏi "nên khuyến nghị gì": không dùng Groq cho chép lời 0,56; cho người dùng tự chọn chế độ "bản nháp nhanh
miễn phí" bằng Groq 0,42; Groq làm mặc định 0,02. Độ tin cậy thấp (0,34).

**Kết luận sau khi đo thật:** Groq Whisper không đủ tốt cho tiếng Việt ở Scribe. Câu bịa chèn vào giữa transcript là
lỗi người dùng khó phát hiện, nguy hiểm hơn việc chép sai chính tả.

## Nếu làm tiếp

1. Đo trên 3–5 ghi âm thật có văn bản gốc (giọng người, có tiếng ồn, nhiều người nói).
2. Chống bịa chữ: lọc trước khi gửi (bỏ đoạn có đỉnh dưới −30 dB, cắt im lặng dưới −40 dB), không dựa vào
   `no_speech_prob` của Groq. Nếu ngưỡng âm lượng bỏ sót giọng nhỏ thì thay bằng VAD thật (ví dụ crate
   `webrtc-vad` trong backend Rust).
3. Chỉ dùng Groq cho phần vượt hạn mức miễn phí của OpenAI, hoặc cho người dùng tự chọn "nhanh, miễn phí" và "chính
   xác"; vẫn đo phút như hiện nay và ghi thêm provider vào `transcribe_runs`.

## Câu hỏi còn mở

- Người dùng có chấp nhận chất lượng thấp hơn để đổi lấy nhiều phút miễn phí hơn không?
- Ai cung cấp được ghi âm thật có văn bản gốc để đo tiếp?
