---
phase: 5
title: "Realtime transcript"
status: pending
priority: P1
effort: "L"
dependencies: [2, 3]
---

# Phase 5: Realtime transcript

## Overview

Vừa ghi vừa hiện chữ: engine ghi của Phase 2 tách luồng âm thanh ra hai đích, file AAC (nguồn chân lý, upload như cũ) và PCM 24 kHz stream qua WebSocket tới OpenAI Realtime (`gpt-4o-mini-transcribe`). Chữ hiện khoảng 1 giây sau mỗi lần ngừng nói, kể cả phần nhận được lúc máy khoá (hiện khi mở lại).

Bằng chứng: [báo cáo đo Realtime](../dattqh/reports/research-261015-1415-realtime-transcription-probe.md). Trên 5 phút buổi họp Viettel, phát đúng tốc độ thật: 43 câu, 0 lỗi, khớp TurboScribe 78,9%, chữ cuối của câu về sau trung vị 0,82 giây, p90 1,00 giây. `server_vad` chạy được, nên app không cần tự làm VAD.

Chất lượng bản realtime kém bản chép cả file khoảng 5 điểm khớp. Bản hoàn chỉnh sau buổi ghi là việc của [Phase 6](./phase-06-ban-hoan-chinh-sau-buoi-ghi.md).

## Quyết định

| # | Quyết định | Lý do |
|---|---|---|
| R1 | Model `gpt-4o-mini-transcribe`, `language: "vi"`, `turn_detection: server_vad` (`silence_duration_ms: 500`) | Chủ sở hữu chọn (2026-10-15): khoảng $0,003/phút, ngân sách $10/tháng. `gpt-live-transcribe` hiện chữ từng từ nhưng tốn $0,017/phút và phải tự làm VAD |
| R2 | Streaming viết bằng Swift (`URLSessionWebSocketTask`), không viết bằng JS | WKWebView bị đình chỉ khi khoá màn hình (D2). Chữ do Swift giữ; Vue **ghi đè** state từ Swift khi foreground, không merge |
| R3 | ~~API key chỉ nằm trên backend~~ → **thay bởi T1/T3 của Phase 6** (2026-10-15): key do người dùng tự nhập, nằm trong Keychain của iPhone; app dùng key đó xin token tạm thời cho phiên | Chủ sở hữu chốt key tự nhập; không còn key chung để bảo vệ |
| R4 | Mất mạng thì realtime hụt đoạn đó, **không** chặn ghi | File AAC vẫn nguyên; Phase 6 chép lại cả file để bù |
| R5 | Bật/tắt realtime bằng một công tắc trên màn ghi, mặc định bật | Có buổi ghi không được phép gửi audio ra ngoài; tắt thì ghi như Phase 2 |

## Gate 0 — đo trước khi viết code (bắt buộc)

1. **Token tạm thời cho phiên transcription.** Từ Mac, gọi API tạo client secret cho phiên `type: "transcription"`, rồi dùng token đó mở WebSocket và stream 2 phút. Phải trả lời được:
   - token sống bao lâu;
   - phiên WebSocket tối đa bao lâu, đủ cho buổi họp 80 phút trở lên không.

   Nếu phiên có trần thời lượng: thiết kế nối lại liên tục, xin token mới trước khi hết hạn và mở phiên mới, không mất câu đang nói. Nếu không tạo được token cho phiên transcription: chuyển sang backend relay WebSocket (app → `onescribe-be` → OpenAI). Phương án này tốn RAM trên `.65`, phải đo lại tải.
2. **WebSocket khi khoá màn hình trên iPhone thật.** Mở rộng `ios-spike`: ghi kèm stream 30 phút, khoá liên tục, chạy bằng free provisioning. Đo số lần mất kết nối, số câu nhận được so với số câu nói, và pin.

Gate fail mà không có lối lui khả thi thì dừng và báo chủ sở hữu, không viết tiếp.

## Requirements

**Chức năng**
- Chữ của từng câu hiện trong panel "Đang chép" ở màn ghi, có mốc thời gian theo đồng hồ phiên ghi.
- Khoá máy rồi mở lại: thấy đủ các câu nhận được trong lúc khoá.
- Mất mạng: hiện "mất kết nối từ mm:ss", tự nối lại có backoff; câu trong khoảng mất mạng đánh dấu "chờ bản hoàn chỉnh".
- Dừng ghi: đóng WebSocket sạch, lưu transcript realtime cạnh session.

**Phi chức năng**
- Không làm chậm hay hỏng engine ghi. Lỗi bên nhánh stream không bao giờ được dừng file AAC.
- Pin: ghi kèm realtime 60 phút khi khoá máy, tụt pin ≤ 15%. Ngưỡng ghi một mình là 12%; nới thêm 3% cho phần radio.
- Chi phí bị chặn ở backend (xem dưới), app không tự đếm tiền.

## Kiến trúc

```
AudioRecorder (Phase 2) ── tap PCM ──▶ RealtimeTranscriber.swift
     │                                   │ AVAudioConverter → PCM16 24 kHz mono, khung 100 ms
     ▼                                   │ URLSessionWebSocketTask + token từ BE
 file AAC (như cũ)                       ▼
                                  TranscriptStore (actor) ── transcript.jsonl (ghi atomic)
                                         │
                                  RecorderPlugin.notifyListeners("transcript") → Vue (foreground)
                                  RecorderPlugin.getTranscript() → Vue đọc lại toàn bộ khi resume
```

- Mỗi câu `completed` lưu `{item_id, text, session_offset_s}`. Ghép theo thứ tự commit, vì sự kiện có thể về lệch thứ tự (đã thấy trong lúc đo).
- `TranscriptStore` là actor riêng, tách khỏi `SegmentStore`, để không đụng manifest upload.

## Backend

- `POST /api/realtime/token` (cần đăng nhập): trả token tạm thời; key đọc từ env của systemd, không bao giờ trả về client.
- **Chặn chi phí là bắt buộc**, vì allowlist hiện đang mở cho mọi tài khoản Google (V4):
  - danh sách email được dùng transcript;
  - trần phút mỗi người mỗi tháng và trần chung khoảng 1.600 phút/tháng ($10 ÷ $0,006 cho cả realtime và Phase 6);
  - ghi số phút vào Mongo.
- Ước lượng phút theo thời lượng phiên do app báo khi đóng phiên, đối chiếu với thời lượng file AAC upload ở Phase 2.

## Related code files

- Create: `web/ios/App/App/Recorder/RealtimeTranscriber.swift`
- Create: `web/ios/App/App/Recorder/TranscriptStore.swift`
- Modify: `web/ios/App/App/Recorder/AudioRecorder.swift` — thêm tap PCM cho nhánh stream
- Modify: `web/ios/App/App/Recorder/RecorderPlugin.swift` + `.m` — event `transcript`, `getTranscript`, cờ bật/tắt realtime
- Modify: `web/src/composables/use-recorder-native.ts`, `web/src/views/RecordView.vue` (chỉ nhánh `isNative`)
- Create: `server/onescribe-be/src/realtime.rs` — endpoint token + chặn chi phí
- Modify: `server/onescribe-be/src/routes.rs`, `src/config.rs` (key, allowlist transcript, trần phút)

## Implementation steps

1. Gate 0 (cả hai mục).
2. Backend: endpoint token, allowlist, trần phút, test bằng curl.
3. `RealtimeTranscriber`: chuyển đổi PCM, WebSocket, `session.update`, nhận `delta`/`completed`, nối lại có backoff.
4. `TranscriptStore` + ghi atomic + đọc lại khi khởi động.
5. Nối vào `AudioRecorder` qua tap. Checkpoint: lỗi nhánh stream (rút mạng, token sai) không ảnh hưởng file AAC.
6. `RecorderPlugin` + Vue panel "Đang chép".
7. Đo trên máy thật theo Success Criteria.

## Success Criteria

- [ ] Gate 0 có số liệu: thời hạn token, trần thời lượng phiên, số lần rớt kết nối trong 30 phút khoá máy.
- [ ] Ghi 60 phút, khoá máy liên tục: số câu nhận được ≥ 95% số câu do `server_vad` chốt; mở máy thấy đủ.
- [ ] Bật chế độ máy bay 2 phút giữa phiên: file AAC nguyên vẹn; realtime tự nối lại; đoạn mất được đánh dấu rõ.
- [ ] Tắt công tắc realtime: không có request nào tới OpenAI (kiểm bằng log backend: không cấp token).
- [ ] Tài khoản ngoài allowlist xin token → 403; chạm trần phút → lỗi rõ ràng, việc ghi vẫn chạy.
- [ ] Pin ≤ 15% cho 60 phút, đo theo *Cài đặt → Pin*.
- [ ] `xcodebuild build` EXIT 0, 0 warning; `cargo clippy` sạch; `vue-tsc` EXIT 0.

## Rủi ro

| Rủi ro | Mức | Giảm thiểu |
|---|---|---|
| Không tạo được token tạm thời cho phiên transcription | Cao | Gate 0 bước 1; lối lui là relay qua backend (đo RAM `.65`) |
| iOS cắt WebSocket khi khoá máy lâu | Cao | Gate 0 bước 2; audio session đang chạy giữ app sống, nối lại có backoff, Phase 6 bù phần hụt |
| Phiên realtime có trần thời lượng nhỏ hơn buổi họp | Trung bình | Nối lại liên tục với token mới (Gate 0 bước 1) |
| Chi phí vượt ngân sách | Trung bình | Trần phút ở backend + allowlist email |
| Audio rời máy | Chấp nhận có điều kiện | Công tắc realtime (R5); chỉ chạy khi bật |
