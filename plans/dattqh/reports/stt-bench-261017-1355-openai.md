# Đo chép lời tiếng Việt: `gpt-4o-mini-transcribe` so với `gpt-transcribe`

Thời điểm đo: 17/10/2026 14:00–14:40 (Asia/Saigon), trên máy dev, không ghi gì lên prod. Plan:
`dattqh/261017-1355-stt-provider-benchmark`. Harness: `scripts/stt-bench/`.

## Kết luận

- **`gpt-transcribe` chép tốt hơn model đang chạy.** Trên 72 phút có văn bản gốc, WER giảm từ 38,5% xuống 34,6%
  (ít lỗi hơn khoảng 10%), CER từ 33,1% xuống 29,3%. Cả hai phần cuộc họp đều tốt hơn. Tốc độ gần như nhau.
- **Giá cao hơn khoảng 2,4 lần:** đo thật được 0,115 USD/giờ với `gpt-4o-mini-transcribe` (tính theo token, rẻ
  hơn mức ước lượng 0,18 của bảng giá) và 0,272 USD/giờ với `gpt-transcribe` (tính theo giây).
- **Khuyến nghị:** lấy `gpt-transcribe` làm phương án mặc định để thay model sắp bị tắt, nhưng chỉ chốt sau khi đo
  thêm ElevenLabs Scribe v2 (0,22 USD/giờ, có người nói) và AssemblyAI trên cùng bộ dữ liệu. Việc chuyển phải
  xong trước 26/02/2027. Jev, hỏi "báo cáo nên khuyến nghị gì": chờ đo đủ 3 nhà cung cấp rồi chốt, mặc định là
  `gpt-transcribe` 0,94; chuyển ngay 0,03; giữ tới sát hạn 0,03.
- Tổng tiền đã tiêu cho lần đo này khoảng **0,59 USD** (0,576 USD cho lượt chính, phần còn lại là chạy thử).

## Dữ liệu

Prod không có cuộc họp nào để đo: db được dựng lại ngày 16/10 và chỉ còn 2 bản ghi dài 33 giây và 3 giây (đã kiểm
bằng truy vấn chỉ đọc). Vì vậy tôi dùng hai cuộc họp thật đã có trên máy, cả hai đều đã được chủ dự án cho gửi
OpenAI (Jev 0,97 chọn phương án này):

| Tệp | Nguồn | Thời lượng | Văn bản gốc |
|---|---|---|---|
| `viettel-a` | `Hop viettel.m4a`, 7:34–29:30 | 21,9 phút | TurboScribe đã sửa (`00-30m (fixed).srt`), 4.432 từ |
| `viettel-b` | `Hop viettel.m4a`, 29:30–hết | 50,3 phút | TurboScribe đã sửa (`2930-end (fixed).srt`), 9.297 từ |
| `hop-anm` | `Họp anm.m4a` | 17,1 phút | không có, chỉ đo độ khớp |

Tôi bỏ đoạn 0:00–7:34 của buổi Viettel vì bản TurboScribe thiếu hẳn phần đó.

**Lưu ý về văn bản gốc:** bản TurboScribe cũng do máy chép (Whisper) nên có lỗi ("tư vấn sản pháp") và bỏ sót
vài đoạn. Vì vậy WER tuyệt đối bị đẩy lên cao. Con số đáng tin là **chênh lệch giữa hai model**, vì cả hai được so
với cùng một bản gốc.

## Cách đo

Giống backend: cắt mảnh 25–45 giây ở khoảng lặng, nén Opus mono 16 kHz 32 kbps, gửi song song 4 mảnh, không có
mảnh nào bị bỏ vì im lặng. Mỗi mảnh chỉ gửi một lần (backend gửi lại mảnh có dấu hiệu lặp, harness chỉ đếm số mảnh
đó). `gpt-4o-mini-transcribe` dùng `language=vi`, `temperature=0`. `gpt-transcribe` dùng `languages[]=vi`: mã ngôn
ngữ sai bị trả 400, nên API thật sự đọc tham số này. Model này không nhận `temperature`. WER/CER tính sau khi
chuẩn hoá NFC, chữ thường và bỏ dấu câu. Có tính cả bản giữ dấu tiếng Việt và bản bỏ dấu.

## Kết quả theo tệp

| Tệp | Model | WER | WER bỏ dấu | CER | Số từ | USD/giờ | Thời gian chạy | Mảnh "có vẻ lặp" |
|---|---|---|---|---|---|---|---|---|
| viettel-a | `gpt-4o-mini-transcribe` | 30,6% | 30,1% | 25,6% | 4.478 | 0,118 | 18 s | 1/35 |
| viettel-a | `gpt-transcribe` | **27,7%** | 27,1% | 23,0% | 4.487 | 0,271 | 20 s | 0/35 |
| viettel-b | `gpt-4o-mini-transcribe` | 42,3% | 41,8% | 36,7% | 9.416 | 0,113 | 38 s | 4/82 |
| viettel-b | `gpt-transcribe` | **37,8%** | 37,3% | 32,3% | 9.817 | 0,272 | 43 s | 3/82 |
| hop-anm | `gpt-4o-mini-transcribe` | — | — | — | 3.261 | 0,115 | 14 s | 0/30 |
| hop-anm | `gpt-transcribe` | — | — | — | 3.240 | 0,273 | 16 s | 0/30 |

Độ khớp giữa hai model (khi không có văn bản gốc, đây là thước đo duy nhất): viettel-a 82,3%, viettel-b 72,9%,
hop-anm 78,2%. Hai model khác nhau khoảng một phần năm số từ.

## Tổng (1,49 giờ audio)

| Model | Chi phí | USD/giờ | WER (72 phút có gốc) | WER bỏ dấu | CER | Thời gian chạy / thời lượng | Trễ trung vị mỗi mảnh |
|---|---|---|---|---|---|---|---|
| `gpt-4o-mini-transcribe` | 0,171 USD | 0,115 | 38,5% | 38,0% | 33,1% | 1,3% | 1,65 s |
| `gpt-transcribe` | 0,405 USD | 0,272 | **34,6%** | 34,0% | 29,3% | 1,5% | 1,89 s |

- Có 1.044 từ của văn bản gốc mà chỉ `gpt-transcribe` chép đúng, so với 560 từ chỉ `gpt-4o-mini-transcribe` chép
  đúng.
- WER bỏ dấu chỉ thấp hơn WER giữ dấu khoảng 0,5 điểm. Lỗi chủ yếu là sai từ, thiếu hoặc thừa từ, không phải
  sai dấu.
- **Mảnh "có vẻ lặp" đều là báo nhầm.** Luật của backend (`looks_looped`: quá 260 từ/phút) bắt cả những đoạn người
  nói nhanh, khoảng 270 từ/phút, chứ không phải model lặp thật. Trên prod, các mảnh này sẽ bị gửi lại tới 3 lần ở
  temperature cao hơn, vừa tốn tiền vừa dễ chép kém đi. Với `gpt-transcribe`, cách gửi lại này còn không dùng được
  vì model không có `temperature`. Khi chuyển model cần xem lại luật này.

## Ví dụ lỗi (trích ngắn)

Đúng hay sai ở đây được đánh giá theo ngữ cảnh và thuật ngữ chuyên ngành, chưa nghe lại audio.

`gpt-transcribe` đúng, `gpt-4o-mini-transcribe` sai:
1. "triển khai từng **lửa** cấu hình" thay vì "từng **đợt** cấu hình".
2. "các **doanh vị** họ" thay vì "các **đơn vị** họ".
3. "nó sẽ **đắp liền kết** khá nhiều" thay vì "nó sẽ **duplicate** khá nhiều".
4. "con người **các tác** thủ công" thay vì "con người **khai thác** thủ công".
5. "**edr edr** và siem" thay vì "**edr ndr** và siem".

`gpt-transcribe` sai, `gpt-4o-mini-transcribe` đúng:
6. "bất cứ chuẩn **a701**" thay vì "chuẩn **iso 9001**".
7. "**soc2** hay là isd" bị chép thành "**2** hay là nist" (mất "SOC").

Cả hai đều có:
8. Cách viết số không thống nhất: một model viết "hơn 700 nhân sự", model kia viết "hơn bảy trăm nhân sự". WER phạt
   cả hai cách dù nội dung đúng.
9. Đoạn nói chồng hoặc nói nhỏ bị cả hai chép lệch theo hai kiểu khác nhau (ví dụ "hướng ngược cái ra cái quyết
    định" và "thường của cái giai ký quyết định"). Đây là chỗ cần đo phân biệt người nói ở bước sau.

## Chạy tiếp khi có key ElevenLabs và AssemblyAI

1. Thêm vào `$ACCESS_TOKENS_JSON`, mục `.tokens[]`, hai phần tử `{"service": "api.elevenlabs.io", "token": "…"}` và
   `{"service": "api.assemblyai.com", "token": "…"}`.
2. Từ gốc repo, chạy:

   ```bash
   S=/private/tmp/claude-501/-Users-hoandat-inet-gitlab-one-scribe/7af8b8e5-71e6-4066-8b09-60890f208a76/scratchpad/stt-bench
   ~/.claude/skills/.venv/bin/python3 -I scripts/stt-bench/bench.py "$S/manifest.json" --out "$S/out"
   ```

   Lệnh này dùng lại kết quả OpenAI đã có và chỉ gọi hai nhà cung cấp mới. Ước tính 0,67 USD cho 1,49 giờ audio.
   Bảng điểm nằm ở `$S/out/summary.md`, lượt nói ở `$S/out/<tệp>/<nhà cung cấp>.turns.json`.
3. Nếu thư mục scratch đã bị dọn, tạo lại manifest theo mục "Chạy lại" của plan, rồi chạy cùng lệnh với một thư
   mục `--out` mới ngoài repo. Khi đó cả 4 cấu hình chạy lại, tổng khoảng 1,3 USD. Script dừng trước khi gửi
   audio nếu ước tính vượt `--budget` (mặc định 3 USD).

Harness gọi ElevenLabs và AssemblyAI theo tài liệu API hiện hành nhưng **chưa chạy thật** vì chưa có key. Lỗi của
một nhà cung cấp được ghi vào `<tệp>/<nhà cung cấp>.error.json` và không chặn các nhà cung cấp khác. Riêng
AssemblyAI có thể trả lỗi nếu `speaker_labels` không hỗ trợ `vi`. Tài liệu chưa nói rõ điểm này.

## Hạn chế

- Chỉ có 2 cuộc họp, chưa đủ 3–5 như kế hoạch, vì prod không còn bản ghi dài. Cần chủ dự án gửi thêm bản ghi
  họp, nhất là cuộc họp có 3 người nói trở lên, để chấm phân biệt người nói.
- Văn bản gốc là bản máy chép (TurboScribe), không phải bản người gõ lại.
- Một lần chạy cho mỗi model. Theo lần đo 16/10, chạy lại cùng tệp với `temperature=0` vẫn có thể ra chữ hơi khác.
