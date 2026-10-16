# Transcript offline bị lặp nặng — nguyên nhân và cách sửa

Tier: **S** (2 file: `sidecar/transcriber/main.py`, có thể thêm `src-tauri/src/settings.rs` hoặc UI glossary).
Trạng thái: đã áp đề xuất 1–3 vào repo (chưa commit, chưa build lại sidecar); đã kiểm chứng lại 30 phút đầu bằng chính code đã sửa.

## Kết luận

Bản raw được chép bằng **large-v3-turbo (ct2, CPU int8)**, chạy qua `BatchedInferencePipeline`. Vòng lặp do hai yếu tố cộng lại:

1. **Glossary quá dài.** Glossary trong `settings.json` dài 1.643 ký tự, tức 601 token, gồm toàn danh sách gạch đầu dòng về Block Storage. faster-whisper chỉ giữ 223 token cuối của prompt (`get_prompt`: `previous_tokens[-(448//2-1):]`). Phần còn lại toàn là dòng `* Volume …` lặp đi lặp lại, nên mồi cho model sinh văn bản dạng lặp. Ngoài ra, prompt chiếm khoảng một nửa `max_length=448`, nên mỗi chunk 30 giây chỉ còn khoảng 220 token để chép, dẫn tới mất chữ.
2. **Chế độ batched không có cơ chế tự phục hồi.** `BatchedInferencePipeline.transcribe` ép `condition_on_previous_text=False` và chỉ dùng `temperature[0]` (= 0, greedy). Vì thế không có temperature fallback, và `compression_ratio_threshold` chỉ được tính ra chứ không dùng để decode lại. Một khi model rơi vào vòng lặp, vòng lặp chạy tới hết ngân sách token.

Hai giả thuyết ban đầu không đúng:
- `condition_on_previous_text`: chế độ batched vốn đã tắt sẵn.
- Chưa bật VAD: code đã có `vad_filter=True`.

## Bằng chứng

Các phép thử chạy với cùng model turbo, trên đoạn 05:00–10:00, so với TurboScribe (524 từ):

| Cấu hình | Số từ | % lặp | Khớp TurboScribe |
|---|---|---|---|
| Bản raw của người dùng | 453 | 69,9 | 9,5% |
| Hiện tại: batched + glossary (tái hiện lỗi) | 389 | 86,6 | 5,2% |
| Batched, không prompt | 337 | 0 | 49,0% |
| Batched, prompt ngắn | 235 | 0 | 32,1% |
| Tuần tự + glossary dài | 336 | 0 | 49,0% |
| **Tuần tự, không prompt** | **457** | **0** | **67,4%** |
| Tuần tự, prompt ngắn khoảng 30 token | 453 | 0 | 66,2% |
| Tuần tự, không prompt, beam 5 | 440 | 0 | 65,1% |

Các thông số dùng chung cho chế độ "tuần tự": `WhisperModel.transcribe`, `vad_filter=True`, `word_timestamps=True`, `condition_on_previous_text=False`, `hallucination_silence_threshold=2.0`, giữ temperature fallback mặc định, `beam_size=1`.

Các phép kiểm chứng khác:
- Đoạn 29:30–34:30: cấu hình hiện tại lặp 75,1% (699 từ); cấu hình tuần tự không prompt lặp 0% (1.001 từ).
- Toàn bộ đoạn 29:30 đến 1:19:48 với cấu hình đã sửa: 9.297 từ, lặp 0,1%, mất 535 giây trên CPU. Bản raw cùng đoạn: 7.695 từ, lặp 79,3%. Từng khúc 5 phút đều lặp ≤0,8%.
- Tốc độ trên turbo CPU: tuần tự mất 26 giây cho 5 phút audio, batched mất 21 giây. Chi phí tăng khoảng 25%.

Đối chiếu với PhoWhisper-large (batched + glossary): không lặp, nhưng không có dấu câu và mốc thời gian segment hỏng (một segment kéo dài 0–200 giây). Vì vậy PhoWhisper không thay được turbo cho file này.

## Đề xuất sửa

1. **Bắt buộc.** Sửa `_transcribe_ct2` để dùng thẳng `WhisperModel.transcribe` thay cho `BatchedInferencePipeline`, giữ VAD và word timestamps, thêm `condition_on_previous_text=False` và `hallucination_silence_threshold=2.0`. Riêng thay đổi này đã chặn được vòng lặp, kể cả khi vẫn dùng glossary dài.
2. **Nên làm.** Giới hạn độ dài prompt ở sidecar, khoảng ≤100 token, giữ phần đầu. Kèm theo đó, UI thêm gợi ý "từ khoá ngắn, phân cách bằng dấu phẩy". Lý do: glossary dài vẫn làm mất khoảng 27% chữ (49% so với 67% khớp) dù không còn lặp.
3. **MLX.** `_transcribe_mlx` dùng mặc định của mlx_whisper là `condition_on_previous_text=True`. Nên thêm `condition_on_previous_text=False` và `hallucination_silence_threshold=2.0`, đồng thời áp cùng giới hạn prompt. Phần này chưa kiểm chứng vì máy chưa tải `large-v3-mlx`.
4. Hậu xử lý cắt n-gram lặp: chưa cần, vì bằng chứng cho thấy tỷ lệ lặp còn 0,1%.

## Có nên thêm nguồn OpenAI API

Khuyến nghị: **chưa nên**. Bản offline sau khi sửa đã cùng hạng chất lượng với TurboScribe, mà TurboScribe cũng dùng họ Whisper large-v3.

| Mặt | Nhận định |
|---|---|
| Dữ liệu | Audio họp nội bộ về an ninh mạng với đối tác sẽ rời khỏi máy. Đây là rủi ro lớn nhất, cần người dùng hoặc pháp chế quyết. |
| Cắt file | Giới hạn 25 MB mỗi request. Nếu nén Opus mono 32 kbps, 80 phút chỉ khoảng 19 MB, gần như gửi một lần là đủ. Nếu cần cắt, cắt tại khoảng lặng VAD (khoảng 10 phút một đoạn) rồi cộng offset. |
| Timestamp | `gpt-4o-transcribe` không trả mốc thời gian theo segment. `whisper-1` có `verbose_json` với segment và word, nhưng chất lượng kém hơn. Muốn có SRT thì phải dùng whisper-1, hoặc căn lại mốc thời gian bằng một model local. |
| Lưu API key | Lưu trong OS keychain (Keychain hoặc Credential Manager), không lưu trong `settings.json` dạng plaintext. |
| Chi phí | Khoảng $0,36/giờ (giá chưa kiểm tra trên trang chính thức). |

## Đầu ra

- `~/Downloads/Hop viettel - 2930-end (fixed).srt` (602 cue, chia cue bằng `resegment_cues` của sidecar)
- `~/Downloads/Hop viettel - 2930-end (fixed).txt`

## Đã triển khai (2026-10-15)

- `sidecar/transcriber/main.py`: bỏ `BatchedInferencePipeline`, ct2 decode tuần tự. Hằng `ANTI_LOOP_OPTS` (`condition_on_previous_text=False`, `hallucination_silence_threshold=2.0`) dùng chung cho cả ct2 và MLX. Đã kiểm tra mlx-whisper 0.4.3 có đủ hai tham số này.
- `src/composables/use-glossary.ts` và `src/views/TranscriptView.vue`: thêm ngưỡng mềm `GLOSSARY_SOFT_LIMIT = 280` ký tự (khoảng 100 token). Ô glossary hiện bộ đếm và cảnh báo khi vượt ngưỡng. Glossary không bị cắt ngầm.
- Kiểm tra: `vue-tsc --noEmit` exit 0; `py_compile main.py` ok.

## Kiểm chứng 30 phút đầu bằng code đã sửa (glossary rỗng)

Chạy trực tiếp `main.transcribe` của repo, mất 303 giây trên CPU.

| Khúc | Bản mới: từ / lặp / khớp TurboScribe | Bản raw: từ / lặp / khớp |
|---|---|---|
| 0–5' | 278 / 0% / 35,7% | 41 / 0% / 5,0% |
| 5–10' | 516 / 0% / 69,7% | 309 / 69,9% / 9,5% |
| 10–15' | 1.065 / 0% / 78,9% | 780 / 79,6% / 8,7% |
| 15–20' | 1.039 / 0% / 79,2% | 789 / 82,5% / 8,0% |
| 20–25' | 933 / 0% / 75,8% | 802 / 66,0% / 13,7% |
| 25–30' | 1.027 / 0% / 73,6% | 864 / 76,7% / 9,8% |
| **Tổng** | **4.858 / 0% / 70,9%** | 3.649 / ~75% / 8,8% |

Riêng khúc 0–5' khớp thấp, nhưng chủ yếu không phải lời thật bị mất:
- Đoạn 0:57–1:35 của TurboScribe là chữ bịa từ tiếng ồn ("bánh tráng", "đồ ăn"…).
- Đoạn 1:53–3:14 là phần giới thiệu nói nhỏ, TurboScribe cũng chép sai nhiều. Bản mới gộp cả cửa sổ 0:00–3:17 thành 19 từ. VAD chỉ nhận ra khoảng 62 giây tiếng nói trong 5 phút.
- Đã thử thêm ba cách cho khúc này, không cách nào đáng đổi: tắt VAD (khớp 47% nhưng chậm gấp 5 lần), chuẩn hoá âm lượng `dynaudnorm` (42%), bỏ `no_speech_threshold` (không đổi).

## Câu hỏi còn mở

- App đang cài (`/Applications/Scribe.app`) vẫn chạy sidecar cũ. Cần build lại sidecar bằng PyInstaller và build lại app thì bản sửa mới có hiệu lực.
- Glossary Block Storage hiện tại (1.643 ký tự) sẽ bị cảnh báo. Người dùng nên rút gọn còn vài từ khoá hoặc để trống.
