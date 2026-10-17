# Nghiên cứu: Phân biệt người nói (Người 1/2, đổi tên được)

Ngày 2026-10-17 · Người làm: researcher · Phạm vi: chỉ nghiên cứu, không sửa code.

## Kết luận trước

1. **Đừng xây trên `gpt-4o-transcribe-diarize`.** OpenAI đã công bố (26/08/2026) ngừng `whisper-1`,
   `gpt-4o-transcribe`, `gpt-4o-mini-transcribe` **và** `gpt-4o-transcribe-diarize` vào **26/02/2027**,
   thay bằng `gpt-transcribe` / `gpt-live-transcribe`, mà tài liệu `gpt-transcribe` không có phân biệt
   người nói lẫn timestamp. Phát hiện phụ quan trọng: model đang chạy ở `openai.rs`
   (`gpt-4o-mini-transcribe`) cũng hết hạn sau khoảng 4 tháng nữa. Dù có làm phân biệt người nói hay
   không, việc chuyển model vẫn là bắt buộc.
2. **Khuyến nghị:** thêm nút "Phân biệt người nói" chạy **lượt thứ hai, tuỳ chọn**, trên **audio đã ghép
   của cả phiên trong một request** (nhờ vậy nhãn người nói nhất quán, không phải ghép nhãn giữa các
   mảnh). Nhà cung cấp ứng viên số 1 là **ElevenLabs Scribe v2** (0,22 USD/giờ, tối đa 32 người nói,
   file tối đa 10 giờ, có timestamp từng từ, hãng tự xếp tiếng Việt vào nhóm WER ≤ 5%). Ứng viên số 2 là
   **AssemblyAI Universal-3.5 Pro** (0,21 + 0,02 USD/giờ, có hỗ trợ `vi`, gán tên theo danh sách). Trước
   khi chốt, cần thử trên 3–5 cuộc họp thật vì chưa có benchmark tiếng Việt độc lập nào.
3. **Lưu kết quả theo thời gian (lượt nói `turns`) trên doc phiên, không lưu theo chữ.** Người nói của
   mỗi câu được suy ra lúc đọc bằng độ chồng thời gian. Cách này giữ nguyên phần người dùng đã sửa chữ,
   chép lại cũng không mất nhãn, và chỉ cần một hàm dùng chung cho giao diện, xuất file, biên bản và tìm kiếm.

## 1. Số liệu từng lựa chọn (đã kiểm tra tới 10/2026)

| Lựa chọn | Giá/giờ audio | Tiếng Việt | Phân biệt người nói | Giới hạn, hình dạng API | Rủi ro áp dụng |
|---|---|---|---|---|---|
| Hiện tại `gpt-4o-mini-transcribe` | 0,18 USD | Đang dùng | Không | Không có timestamp, mốc thời gian đang ước lượng | **Ngừng 26/02/2027** |
| `gpt-transcribe` (model thay thế) | 0,27 USD | Có tham số `languages` | Không được nêu | 25 MB; tài liệu không nói có timestamp | Model mới, thay thế chính thức |
| `gpt-4o-transcribe-diarize` | 0,36 USD | Không công bố riêng | `diarized_json` gồm `speaker/start/end/text`; tối đa 4 người mẫu (2–10 giây) | 25 MB; trên 30 giây bắt buộc `chunking_strategy`; **không nhận prompt**; tối đa 2.000 token đầu ra | **Ngừng 26/02/2027**; forum báo cắt mất đoạn cuối, bịa chữ ở đoạn ồn, nhãn reset ở mỗi request |
| **ElevenLabs Scribe v2** | **0,22 USD** | Nhóm "Excellent ≤5% WER" (số do hãng tự công bố: FLEURS 3,1%) | `diarize`, `num_speakers` ≤ 32; `words[].speaker_id` | Có timestamp từng từ; file 3 GB/10 giờ; có webhook; nhận diện vai trò +10% | Hãng lớn, model mới; số WER không độc lập |
| **AssemblyAI U-3.5 Pro** (U-2) | 0,21 + 0,02 = **0,23 USD** (U-2: 0,17 USD) | `vi` có trong danh sách của cả hai model | `speaker_labels`, min/max số người; `utterances` (ms) kèm words; Speaker Identification theo tên | Mỗi người nên nói liên tục ≥ 30 giây | Chưa xác nhận diarization chạy với `vi`, cần thử thật |
| Deepgram Nova-3 multilingual | 0,31 USD (đã gồm diarization) | `vi` có ở Nova-3 và Nova-2 | Có sẵn | REST đơn giản | Bảng FLEURS của ElevenLabs ghi Nova-2 11,6% WER tiếng Việt (bên cạnh tranh công bố) |
| Gladia Solaria-3 | từ 0,16 EUR | "100+ ngôn ngữ", không xác nhận có `vi` | Đã gồm (chỉ async) | Tặng 50 EUR | Chưa xác minh tiếng Việt |
| Speechmatics Standard | 0,45 USD | "55+", không xác nhận có `vi` | Có | | Đắt gấp 2,5 lần hiện tại |
| pyannoteAI Precision-3 | 0,112 EUR (Community-1: 0,035 EUR) | Không phụ thuộc ngôn ngữ (chỉ phân biệt người nói, không chép lời) | Lượt nói theo thời gian; voiceprint 0,015 EUR/cái | Phải tự ghép với chữ của OpenAI | Độ chính xác bị chặn bởi mốc câu đang ước lượng (xem mục 5) |

Chạy cục bộ (pyannote/WhisperX) bị loại vì host .65 không có GPU, `MemoryMax=1G`, chỉ còn khoảng 3 GB RAM trống.

## 2. Câu hỏi 1: lựa chọn tốt nhất và chi phí

- Hôm nay tốn 0,18 USD/giờ. Nếu dùng **lượt hai bằng Scribe v2**, phiên được phân biệt người nói tốn
  0,18 + 0,22 = **0,40 USD/giờ** (khoảng 2,2 lần). Phiên không bấm nút thì không tốn thêm. Một user free
  dùng hết 120 phút và bấm cho mọi phiên chỉ tốn thêm tối đa khoảng 0,44 USD/tháng.
- Sau khi bắt buộc chuyển sang `gpt-transcribe`, chi phí nền thành 0,27 USD/giờ, cộng Scribe là 0,49 USD/giờ.
  Nếu bài thử cho thấy Scribe v2 chép tiếng Việt **ngang hoặc tốt hơn** OpenAI, có thể dùng **một lượt
  Scribe cho cả chép lời lẫn người nói** với giá 0,22 USD/giờ, **rẻ hơn** `gpt-transcribe` và có mốc thời
  gian thật thay cho mốc ước lượng. Việc này đụng quyết định "giữ OpenAI" (16/10), nên là câu hỏi cho chủ dự án.
- OpenAI diarize đắt nhất trong nhóm khả thi (0,36 USD/giờ), sẽ bị khai tử, không nhận prompt và reset
  nhãn ở mỗi request. **Loại.**

## 3. Câu hỏi 2: giữ nhãn người nói nhất quán giữa các mảnh

- **Cách chọn: phân biệt người nói một lần trên audio đã ghép của cả phiên.** Dùng ffmpeg (luồng có sẵn
  trên server) ghép các đoạn iPhone 1 phút theo `offset`, chèn im lặng vào chỗ hở để mốc giây khớp đồng
  hồ phiên, rồi nén Opus mono 16 kHz khoảng 24 kbps (khoảng 10,8 MB/giờ). Gửi một request. Scribe nhận
  tới 10 giờ, AssemblyAI nhận file dài, nên phiên thực tế không cần cắt mảnh. ffmpeg chạy dạng luồng nên
  vừa trong `MemoryMax=1G`. Khi gửi, nên đọc từ file tạm thay vì giữ cả file trong RAM.
- **Ghép nhãn giữa các mảnh (để dự phòng, không cần ở bản đầu):** chỉ cần khi phiên vượt giới hạn của nhà
  cung cấp. Có hai cách. (a) Cắt mảnh chồng nhau khoảng 30 giây, rồi khớp nhãn theo vùng chồng. (b) Dùng
  "người mẫu": OpenAI cho tối đa 4 clip, pyannoteAI dùng voiceprint. OpenAI không giữ nhãn qua các
  request, và forum OpenAI xác nhận nhãn có thể bị đổi chỗ giữa các mảnh.
- Mảnh ffmpeg 25–45 giây của luồng chép lời hiện tại **không liên quan**: lượt phân biệt người nói không
  dùng các mảnh đó.

## 4. Câu hỏi 3: thay lượt chép hay làm lượt hai tuỳ chọn?

**Làm lượt hai tuỳ chọn, chỉ trên phiên đã ghi xong.** Lý do:
- Phần lớn bản ghi là một người nói (ghi chú, bài giảng). Bật mặc định thì trả thêm tiền vô ích.
- Không phải đụng luồng chép đang ổn định (lọc lặp, bỏ đoạn im lặng, tính quota theo `sentS`).
- Phiên iPhone đang ghi dở thì chưa có audio ghép, nên chỉ chạy được khi phiên đã xong.
- Ngoại lệ đáng cân nhắc: nếu trước tháng 02/2027 quyết chuyển hẳn sang Scribe, thì bật phân biệt người
  nói ngay trong lượt chép chính và bỏ lượt hai (theo DRY, chỉ còn một luồng). Việc này cần chủ dự án quyết.
- Tính quota: lượt hai nên trừ phút như một lượt chép (dùng chung `usage::Charge`), hoặc chỉ mở cho gói
  trả phí. Chủ dự án quyết.

## 5. Câu hỏi 4: phác thảo dữ liệu

Trên `transcript_docs` (doc theo phiên, khoá `s:`/`f:`), thêm:

```text
speakers: {
  model, createdAt,
  turns: [{ start, end, s }],          // giây tính từ đầu phiên, s = "1","2",...
  names: { "1": "Anh Hùng", "2": null } // null → hiển thị "Người 2"
}
```

- **Người nói của câu = lượt nói chồng thời gian nhiều nhất với câu `[start, end]`** (đã cộng `offset_s`
  của đoạn). Tính lúc đọc bằng một hàm thuần duy nhất, dùng chung cho API phiên, xuất file, biên bản và
  hỏi đáp. Không cần migration cho `transcripts` và `transcript_sentences`.
- **Sửa chữ** (`SegmentEdit` theo `runId`): không bị ảnh hưởng vì người nói gắn với thời gian, không gắn với chữ.
  Chép lại một đoạn vẫn giữ nguyên lượt nói, vì audio không đổi.
- **Xoá đoạn hoặc phiên:** bỏ `speakers` giống cách `forget_segment` đang bỏ biên bản, vì kết quả đi theo audio.
- **Đổi tên:** chỉ sửa `names` (đổi một chỗ thì áp cho cả phiên). Có thể thêm `overrides: [{at, s}]` để
  sửa tay người nói của từng câu, nhưng nên để giai đoạn 2 nếu bài thử cho thấy cần.
- **Xuất file:** txt/md/docx thêm "Anh Hùng: …"; VTT dùng thẻ giọng `<v Anh Hùng>` (chuẩn WebVTT);
  SRT thêm tiền tố tên. Chỉ cần sửa `Part` hoặc `export.rs` để nhận thêm tên người nói của mỗi câu.
- **Biên bản:** `build_input` thêm tên người nói, giúp LLM điền `Action.owner` đúng hơn. Nên tính
  `sourceHash` trên **mã** người nói (`s`), không tính trên tên, để đổi tên không làm biên bản bị coi là cũ.
- **Tìm kiếm:** không cần đánh lại index. Kết quả tìm gắn tên người nói lúc đọc. Lọc theo người nói thì
  để sau (YAGNI).
- **Điểm yếu đã biết:** mốc câu hiện nay **chỉ là ước lượng** trong phạm vi một mảnh khoảng 40 giây
  (chia theo độ dài chữ, xem `chunking.rs`). Với các câu đối đáp ngắn, câu có thể bị gán nhầm người.
  Cách xử lý: khi dùng Scribe hoặc AssemblyAI, lấy luôn **timestamp từng từ** của họ để căn lại câu, hoặc
  dùng chữ của họ cho phiên đó. Bài thử phải đo đúng chỉ số này.

## 6. Câu hỏi 5: rủi ro

- **Nói chồng lên nhau:** mọi nhà cung cấp đều yếu. AssemblyAI ghi rõ giọng chồng, giọng giống nhau và
  vọng âm làm giảm độ chính xác, và mỗi người nên nói ít nhất 30 giây. Máy iPhone đặt giữa bàn họp thì
  nhiễu và vọng là chuyện thường.
- **Tiếng Việt:** không có benchmark độc lập. Số của ElevenLabs là hãng tự công bố và khác nhau giữa các
  phiên bản trang (FLEURS 3,1% so với 3,5%). Diarization chủ yếu dựa vào âm học nên ít phụ thuộc ngôn
  ngữ hơn chép lời, nhưng vẫn phải thử thật.
- **Đoán sai số người** (tách một người thành hai, hoặc gộp hai người): cho người dùng nhập "khoảng N người"
  (min/max) và đổi tên hoặc gộp nhãn bằng tay.
- **Chi phí:** chỉ tốn khi bấm nút, có trần theo quota. Rủi ro lớn hơn nằm ở việc OpenAI khai tử model chép lời hiện tại.
- **Quyền riêng tư:** audio cuộc họp được gửi thêm cho một bên xử lý thứ hai ở nước ngoài. Luật Bảo vệ dữ
  liệu cá nhân 91/2025/QH15 có hiệu lực từ 01/01/2026, có điều riêng về dữ liệu sinh trắc học (Điều 31),
  kèm Nghị định 356/2025. Phân biệt người nói **không lưu vân tay giọng nói** thì rủi ro thấp hơn hẳn so
  với nhận diện bằng voiceprint hay người mẫu lưu lâu dài. Bản đầu nên **không** lưu voiceprint. Cần đọc
  DPA và chính sách lưu dữ liệu của nhà cung cấp, và cập nhật chính sách quyền riêng tư của Scribe.

## 7. Khuyến nghị và các phương án thay thế

**Khuyến nghị (tier M):** thử trước, sau đó làm lượt hai tuỳ chọn trên audio ghép của phiên, dùng
ElevenLabs Scribe v2, lưu `speakers.turns` cùng `names` trên doc phiên, và gán người nói cho câu lúc đọc.
Bài thử gồm 3–5 cuộc họp thật (2–6 người, có đoạn nói chồng), chạy qua Scribe v2, AssemblyAI U-3.5 Pro
và OpenAI diarize, rồi chấm: tỉ lệ câu gán đúng người, số người đoán được, và chất lượng chữ tiếng Việt
so với `gpt-4o-mini-transcribe`/`gpt-transcribe`.

| Phương án | Ưu | Nhược |
|---|---|---|
| A. AssemblyAI U-3.5 Pro thay cho Scribe | Rẻ tương đương (0,23 USD/giờ), có min/max số người, gán tên theo danh sách, tài liệu tốt | Chưa xác nhận diarization chạy với `vi`; U-3.5 Pro chỉ hỗ trợ 18 ngôn ngữ |
| B. Chuyển hẳn lượt chép chính sang Scribe v2 (có người nói luôn) | Một luồng duy nhất, 0,22 USD/giờ (rẻ hơn `gpt-transcribe`), mốc thời gian thật, giải quyết luôn hạn 02/2027 | Đảo quyết định "giữ OpenAI"; phải viết lại phần lọc lặp và tính quota; rủi ro chất lượng chữ |
| C. pyannoteAI chỉ phân biệt người nói, giữ chữ OpenAI | Rẻ nhất (0,112 EUR/giờ), không đổi bên chép lời, không phụ thuộc ngôn ngữ | Gán câu dựa trên mốc ước lượng nên dễ sai khi đối đáp nhanh; thêm một nhà cung cấp |

Bị loại: OpenAI `gpt-4o-transcribe-diarize` (khai tử 02/2027, đắt nhất, nhãn reset ở mỗi request,
không nhận prompt) và chạy ML cục bộ (không đủ tài nguyên).

## Hạn chế của nghiên cứu này

- Không chạy thử API nào. Mọi nhận định về chất lượng tiếng Việt đến từ tài liệu và số do hãng công bố.
- Chưa đọc DPA hay chính sách lưu dữ liệu của từng nhà cung cấp, và chưa đọc nguyên văn Điều 31 cùng
  Nghị định 356.
- Giá EUR của pyannoteAI và Gladia chưa quy đổi. Giá của Speechmatics và Gladia chưa kiểm chứng phần tiếng Việt.

## Câu hỏi cho chủ dự án (gửi Jev)

1. Trước 26/02/2027 phải bỏ `gpt-4o-mini-transcribe`. Chọn chuyển sang `gpt-transcribe` (0,27 USD/giờ,
   giữ OpenAI) hay mở bài thử Scribe v2 để cân nhắc phương án B (0,22 USD/giờ, có sẵn người nói)?
2. Lượt "Phân biệt người nói" có trừ phút quota không, hay chỉ mở cho gói trả phí?
3. Có chấp nhận gửi audio cho một bên xử lý thứ hai (ElevenLabs hoặc AssemblyAI, máy chủ ở Mỹ/EU) không?
   Có cần cập nhật chính sách quyền riêng tư không?
4. Bản đầu có cần sửa tay người nói của từng câu không, hay chỉ cần đổi tên nhãn?

## Nguồn

- OpenAI, hướng dẫn chép lời (phần Speaker diarization): https://developers.openai.com/api/docs/guides/speech-to-text
- OpenAI, bảng giá: https://developers.openai.com/api/docs/pricing
- OpenAI, trang model diarize: https://developers.openai.com/api/docs/models/gpt-4o-transcribe-diarize
- OpenAI, trang model gpt-transcribe: https://developers.openai.com/api/docs/models/gpt-transcribe
- OpenAI, lịch khai tử model (26/02/2027): https://developers.openai.com/api/docs/deprecations
- Forum OpenAI, nhãn reset giữa các mảnh: https://community.openai.com/t/best-practices-for-maintaining-speaker-identity-across-chunks-with-gpt-4o-transcribe-diarize/1364126
- Forum OpenAI, lỗi cắt mất đoạn cuối: https://community.openai.com/t/gpt-4o-transcribe-truncation-issue-after-the-diarize-api-update/1364344 ; https://community.openai.com/t/gpt-4o-transcribe-diarize-only-transcribe-the-first-32-sec/1367208
- ElevenLabs, khả năng chép lời: https://elevenlabs.io/docs/capabilities/speech-to-text ; API: https://elevenlabs.io/docs/api-reference/speech-to-text/convert ; giá: https://elevenlabs.io/pricing/api ; số tiếng Việt do hãng công bố: https://elevenlabs.io/speech-to-text/vietnamese
- AssemblyAI, giá: https://www.assemblyai.com/pricing ; ngôn ngữ hỗ trợ: https://www.assemblyai.com/docs/pre-recorded-audio/supported-languages ; phân biệt người nói: https://www.assemblyai.com/docs/pre-recorded-audio/speaker-diarization
- Deepgram, giá: https://deepgram.com/pricing ; ngôn ngữ: https://developers.deepgram.com/docs/models-languages-overview
- pyannoteAI, giá: https://www.pyannote.ai/pricing ; Speechmatics: https://www.speechmatics.com/pricing ; Gladia: https://www.gladia.io/pricing
- Luật Bảo vệ dữ liệu cá nhân 91/2025/QH15: https://gvlawyers.com.vn/wp-content/uploads/2025/09/VN_Legal-alert-_Law-on-PDP-2025.pdf ; Nghị định 356/2025: https://thuvienphapluat.vn/van-ban/Quyen-dan-su/Nghi-dinh-356-2025-ND-CP-huong-dan-Luat-Bao-ve-du-lieu-ca-nhan-687428.aspx
