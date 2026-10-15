# Đo OpenAI Realtime transcription cho app mobile

Ngày đo: 2026-10-15. Mục đích: quyết định cách làm transcript realtime cho app iOS (Capacitor kèm plugin Swift, plan `260728-1609-onescribe-ios-app-ghi-nen-khoa-man-hinh`).

## Kết luận

Realtime dùng được cho tiếng Việt, độ trễ khoảng 1 giây sau mỗi lần ngừng nói. Đề xuất dùng **`gpt-4o-mini-transcribe` qua Realtime với VAD phía server**, rồi chép lại cả file sau buổi ghi để có bản hoàn chỉnh. Chi phí khoảng $0,006/phút, tức 80 phút khoảng $0,48; ngân sách $10/tháng đủ cho khoảng 27 giờ.

## Cách đo

- Clip 10:00–15:00 buổi họp Viettel (5 phút), phát ở tốc độ thật, mỗi lần 100 ms PCM16 24 kHz qua `wss://api.openai.com/v1/realtime?intent=transcription`.
- Phiên cấu hình bằng `session.update` với `session.type = "transcription"`.
- Chốt câu bằng VAD phía client (silero, im lặng ≥0,5 giây, hoặc câu dài 15 giây thì chốt thêm), tổng 43 câu.
- Độ khớp so với bản TurboScribe cùng đoạn (1.040 từ), cùng cách đo với các báo cáo trước.
- Script: `scratchpad/exp/rt_probe.py` (tạm, không nằm trong repo).

## Kết quả

| Cách chép | Số từ | Lặp | Khớp TurboScribe | Trễ chữ cuối sau khi chốt câu (trung vị / p90) | Chữ hiện trong lúc đang nói | Giá |
|---|---|---|---|---|---|---|
| Realtime `gpt-live-transcribe` | 1.079 | 0% | 80,8% | 0,66 s / 0,81 s | Có (trung vị sớm hơn 4,7 s so với lúc chốt câu) | $0,017/phút |
| Realtime `gpt-4o-mini-transcribe` | 1.077 | 0% | 78,9% | 0,82 s / 1,00 s | Không, chữ hiện theo từng câu | ~$0,003/phút |
| Batch `gpt-4o-mini-transcribe` (mảnh ≤45 s, đã đo trước) | 1.084 | 0% | 84,4% | không áp dụng | không áp dụng | ~$0,003/phút |

- `gpt-4o-mini-transcribe` chấp nhận `turn_detection: server_vad` (thử 30 giây, server tự chốt 2 câu). Nhờ đó app không cần tự làm VAD.
- `gpt-live-transcribe` không hỗ trợ VAD phía server theo tài liệu, nên app phải tự chốt câu.
- Realtime kém batch khoảng 4–6 điểm khớp, vì mỗi câu ngắn hơn và có ít ngữ cảnh hơn. Đây là lý do nên chép lại cả file sau buổi ghi.

## Hệ quả cho thiết kế iOS

- Phần streaming phải viết bằng Swift (`URLSessionWebSocketTask`), vì WKWebView bị đình chỉ khi khoá màn hình. Engine ghi tách luồng âm thanh ra hai đích: file AAC lưu trên máy (nguồn chân lý) và PCM 24 kHz cho WebSocket.
- Key không nằm trong app. Backend cần cấp token tạm thời cho từng phiên. Cách tạo token cho phiên transcription **chưa xác minh** trong tài liệu, cần thử trước khi viết phase.
- Mất mạng giữa buổi thì hụt chữ đoạn đó, nhưng file ghi vẫn nguyên; bản chép lại sau buổi ghi sẽ bù.

## Câu hỏi còn mở

- Cách tạo token tạm thời (client secret) cho phiên transcription, và thời hạn của token so với buổi họp dài 80 phút trở lên.
- Phiên WebSocket có bị iOS cắt khi khoá màn hình lâu không. Cần đo trên máy thật ở phase iOS.
