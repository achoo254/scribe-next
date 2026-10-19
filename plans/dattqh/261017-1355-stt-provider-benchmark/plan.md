---
title: Đo chép lời tiếng Việt trên 3 nhà cung cấp (OpenAI, ElevenLabs Scribe v2, AssemblyAI)
status: completed
created: 2026-10-17
tier: M
---

# Đo chép lời tiếng Việt trên 3 nhà cung cấp

> **Đóng plan (2026-10-19):** sau số đo và bài nghe mù, chủ dự án chốt ngày 17/10 chuyển chép lời tệp sang
> ElevenLabs Scribe v2 (kèm phân biệt người nói); triển khai ở `../261017-1530-elevenlabs-transcription/`, lên prod
> 17/10 16:07. AssemblyAI không được đo vì chủ dự án chọn không thêm key.

Yêu cầu chủ dự án 17/10/2026 13:55. Bối cảnh: OpenAI tắt `gpt-4o-mini-transcribe` (model đang chạy ở
`openai.rs`), `gpt-4o-transcribe`, `whisper-1` và `gpt-4o-transcribe-diarize` vào **26/02/2027**; model thay thế là
`gpt-transcribe` / `gpt-live-transcribe` (xem `reports/researcher-261017-1240-speaker-diarization.md`). Cần số đo
thật trên cuộc họp tiếng Việt trước khi chọn model chép lời mới và nhà cung cấp phân biệt người nói.

## Quyết định
- Chủ dự án: đo 3 nhà cung cấp trên 3–5 cuộc họp thật của chính mình, đồng ý gửi các bản ghi đó cho nhà cung cấp.
- Chạy OpenAI ngay (`gpt-4o-mini-transcribe` làm mốc, `gpt-transcribe`); ElevenLabs và AssemblyAI chạy khi chủ dự án
  thêm key (`api.elevenlabs.io`, `api.assemblyai.com` trong `$ACCESS_TOKENS_JSON`). Trần chi phí $3.
- Dữ liệu (Jev 0,97 "dùng hai cuộc họp có sẵn trên máy"): prod chỉ còn 2 bản ghi 33 giây và 3 giây (db dựng lại
  16/10), không có cuộc họp nào. Dùng `Hop viettel.m4a` (80 phút, có bản TurboScribe làm văn bản gốc) và
  `Họp anm.m4a` (17 phút, đã gửi OpenAI/Groq ngày 16/10 với sự đồng ý của chủ dự án). Không dùng tệp zip mồ côi
  từ tháng 7 trên prod và không dùng ghi âm trong thư viện Nhạc (chưa được chủ dự án chỉ định).
- Đoạn Viettel 0:00–7:34 bị bỏ vì bản TurboScribe thiếu hẳn phần đó (khoảng trống 194 giây và 118 giây).
- Không sửa code server trong việc này.

## Cách đo
- Harness: `scripts/stt-bench/bench.py` (Python thư viện chuẩn, chạy bằng `~/.claude/skills/.venv/bin/python3 -I`;
  điểm số ở `metrics.py`, gọi API ở `providers.py`, test `test_metrics.py`). Manifest liệt kê tệp audio, đoạn
  `[start, end)` và văn bản gốc (.srt hoặc .txt). Kết quả (audio đã cắt, JSON thô, chữ chép, lượt nói, điểm) chỉ
  nằm trong thư mục `--out` ngoài repo; script từ chối `--out` nằm trong repo.
- OpenAI: cắt mảnh 25–45 giây ở khoảng lặng (`silencedetect −35 dB/0,4 s`), Opus mono 16 kHz 32 kbps, bỏ mảnh đỉnh
  dưới −30 dB, gửi song song 4 mảnh, giống backend. `gpt-4o-mini-transcribe`: `language=vi`, `temperature=0`;
  `gpt-transcribe`: `languages[]=vi` (đã thử: mã sai bị từ chối 400, nên tham số được đọc). Một lần gửi mỗi mảnh,
  không gửi lại khi có dấu hiệu lặp; số mảnh "có vẻ lặp" theo luật của backend được đếm riêng.
- ElevenLabs: cả tệp trong một request, `scribe_v2`, `language_code=vi`, `diarize=true`, timestamp theo từ.
  AssemblyAI: tải lên rồi tạo transcript với `speech_models=["universal-3-5-pro","universal-2"]`,
  `language_code=vi`, `speaker_labels=true`, chờ tới khi xong.
- Điểm: WER/CER sau chuẩn hoá (NFC, chữ thường, bỏ dấu câu, giữ dấu tiếng Việt) và WER bỏ dấu; độ khớp từng
  cặp nhà cung cấp khi không có văn bản gốc; chi phí từ thời lượng/token do API trả × giá niêm yết; thời gian
  chạy; số người nói tìm được.
- Giá dùng để tính (USD/giờ, kiểm 17/10): `gpt-4o-mini-transcribe` theo token ($1,25/$5 cho 1 triệu token vào/ra,
  khoảng 0,18), `gpt-transcribe` 0,27, Scribe v2 0,22, AssemblyAI U-3.5 Pro 0,21 (U-2 0,15) + phân biệt người nói 0,02.

## Phase
| Phase | Tier | Trạng thái |
|---|---|---|
| 1. Harness + test | M | done |
| 2. Chạy OpenAI, báo cáo `reports/stt-bench-261017-1355-openai.md` | S | done |
| 3. Chạy ElevenLabs + AssemblyAI khi có key, chấm phân biệt người nói | S | done với ElevenLabs; AssemblyAI không đo (chủ dự án không thêm key) |
| 4. Chọn model chép lời mới (trước 26/02/2027) và nhà cung cấp phân biệt người nói | M | done 17/10: ElevenLabs Scribe v2 cho chép lời tệp và phân biệt người nói |

## Tiêu chí nhận
- Một lệnh chạy được cả 4 cấu hình; thiếu key thì bỏ qua và báo rõ; key không bao giờ bị in hay ghi ra đĩa.
- Có ít nhất một tệp đo WER so với văn bản gốc; tổng chi phí lượt OpenAI dưới $3.
- Báo cáo có bảng theo tệp và tổng, ví dụ lỗi ngắn (≤ 10), khuyến nghị về `gpt-transcribe` và lệnh chạy tiếp.
- Không commit audio hay chữ chép của cuộc họp (chỉ trích ngắn trong báo cáo).

## Chạy lại
Manifest đang dùng nằm ở thư mục scratch của phiên 17/10 (đường dẫn trong báo cáo). Nếu thư mục đó đã mất, tạo lại:

```json
{"items": [
  {"id": "viettel-a", "audio": "~/Downloads/Hop viettel.m4a", "start": 454, "end": 1770,
   "ref": "~/Downloads/Hop viettel - 00-30m (fixed).srt"},
  {"id": "viettel-b", "audio": "~/Downloads/Hop viettel.m4a", "start": 1770,
   "ref": "~/Downloads/Hop viettel - 2930-end (fixed).srt"},
  {"id": "hop-anm", "audio": "~/Downloads/Họp anm.m4a"}
]}
```
