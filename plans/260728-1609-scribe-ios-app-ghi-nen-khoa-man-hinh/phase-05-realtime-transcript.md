---
phase: 5
title: "Realtime transcript"
status: in-progress
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
| R3 | Key OpenAI **của server** cấp token tạm thời (`POST /api/realtime/token`); key không rời server. Phút realtime tính **chung** hạn mức với chép lời cả file (hạn mức tháng của gói, sổ ngày + quỹ free, trần tổng), giữ chỗ khi cấp token, quyết toán theo số giây app báo. Chỉ email trong biến build `REALTIME_EMAILS` (rỗng = không ai) cho tới khi đo xong trên máy thật | Jev chốt 2026-10-17. Thay quyết định cũ (key người dùng tự nhập, T1/T3 của Phase 6), vì chép lời đã chuyển sang key server có tính phút |
| R4 | Mất mạng thì realtime hụt đoạn đó, **không** chặn ghi | File AAC vẫn nguyên; chép lời cả file sau buổi ghi vẫn chạy như cũ |
| R5 | Công tắc "Chép trực tiếp" trên thẻ ghi, **mặc định tắt**, nhớ lựa chọn trên từng máy | Jev chốt 2026-10-17. Có buổi ghi không được phép gửi audio ra ngoài; tắt thì ghi như Phase 2 |
| R6 | Chữ realtime chỉ là bản xem trước: lưu cạnh phiên trên máy, hiện ở màn ghi, **không** lưu thành bản chép lời | Jev chốt 2026-10-17. Bản chép lời vẫn là kết quả chép cả file sau khi tải lên |
| R7 | **Đổi sang ElevenLabs Scribe v2 Realtime** (`scribe_v2_realtime`, `language_code=vi`, `commit_strategy=vad`, im lặng 1,0 giây), key ElevenLabs của server cấp token dùng một lần. OpenAI Realtime giữ làm dự phòng một bản phát hành qua biến build `REALTIME_PROVIDER=openai`. Thay R1 | Chủ dự án chốt 2026-10-19 (roadmap P1): OpenAI tắt `gpt-4o-mini-transcribe` ngày 26/02/2027; chép lời theo tệp đã chạy ElevenLabs từ 17/10, nên đường giọng nói không còn phụ thuộc OpenAI |

## Gate 0 — đo trước khi viết code (bắt buộc)

1. **Token tạm thời cho phiên transcription.** Từ Mac, gọi API tạo client secret cho phiên `type: "transcription"`, rồi dùng token đó mở WebSocket và stream 2 phút. Phải trả lời được:
   - token sống bao lâu;
   - phiên WebSocket tối đa bao lâu, đủ cho buổi họp 80 phút trở lên không.

   Nếu phiên có trần thời lượng: thiết kế nối lại liên tục, xin token mới trước khi hết hạn và mở phiên mới, không mất câu đang nói. Nếu không tạo được token cho phiên transcription: chuyển sang backend relay WebSocket (app → `scribe-be` → OpenAI). Phương án này tốn RAM trên `.65`, phải đo lại tải.
2. **WebSocket khi khoá màn hình trên iPhone thật.** Mở rộng `ios-spike`: ghi kèm stream 30 phút, khoá liên tục, chạy bằng free provisioning. Đo số lần mất kết nối, số câu nhận được so với số câu nói, và pin.

Gate fail mà không có lối lui khả thi thì dừng và báo chủ sở hữu, không viết tiếp.

### Kết quả Gate 0 bước 1 (đo từ Mac, 2026-10-15)

Tạo token qua `POST /v1/realtime/client_secrets` với `session.type = "transcription"`, mở `wss://api.openai.com/v1/realtime` bằng token đó, stream PCM 24 kHz ở tốc độ thật.

| Câu hỏi | Kết quả |
|---|---|
| Tạo được token tạm cho phiên transcription không | Được |
| Token sống bao lâu | Mặc định 600 giây; đặt được tới 7.200 giây qua `expires_after` (7.201 bị từ chối, HTTP 400) |
| Token hết hạn khi phiên đang mở | Phiên vẫn chạy (token 10 giây, phiên chạy tiếp 2 phút, chép 15 câu) |
| Token hết hạn có mở phiên mới được không | Không: lỗi `Ephemeral token expired`, đóng mã 3000 |
| Một token còn hạn mở nhiều phiên | Được (2 phiên liên tiếp) |
| Phiên dài nhất | Một lần đo: 39,3 phút, 320 câu, rồi đóng mã 1006 (đứt không có khung đóng, không có sự kiện lỗi). Chưa phân biệt được trần phía OpenAI hay mạng phía máy đo |

Hệ quả thiết kế: phiên 80 phút trở lên phải **tự nối lại liên tục** (mở phiên mới, nối câu theo mốc đồng hồ phiên ghi). App xin token TTL 600 giây từ backend mỗi khi đổi token (theo R3). Bước 2 (WebSocket khi khoá màn hình) vẫn cần iPhone.

Kiểm lại 2026-10-17 từ Mac bằng đúng đường mã của backend (test `live_realtime_mint_returns_a_ten_minute_token`, key dev): `client_secrets` vẫn nhận cấu hình như trên, trả token `ek_…`, còn 600 giây.

## Đã làm (2026-10-17)

- **Backend** `server/scribe-be/src/realtime.rs` + `models/realtime_token.rs`: `POST /api/realtime/token` → `{token, tokenId, expiresAt, model, reservedSeconds}`; `POST /api/realtime/usage {tokenId, seconds}`. Mỗi token giữ chỗ 600 giây (còn ít thì 300/120/60) qua `usage::reserve`; báo số giây thì quyết toán đúng một lần (`settled`), ghi `transcribe_runs` (`kind: "realtime"`); không báo trong 2 giờ sau khi token hết hạn thì tính trọn phần giữ. Giây báo bị chặn trên bởi thời gian thật từ lúc cấp. Tôn trọng cờ tạm ngưng khi key hỏng (503, OpenAI báo key hỏng lúc cấp token cũng ngắt như chép lời), tài khoản free mới phải chờ, mã lỗi hạn mức dùng chung (`QUOTA_EXCEEDED`, `DAILY_LIMIT`, `CAP_REACHED`, `ACCOUNT_TOO_NEW`); ngoài danh sách → 403 `REALTIME_NOT_ALLOWED`. Audit `realtime.token`, `realtime.blocked`. `admin usage --recompute` cộng token chưa quyết toán.
- **Swift** `RealtimeTranscriber.swift` (đổi PCM, WebSocket, đổi token trước khi hết phần giữ, nối lại có backoff, giữ 30 giây audio khi đang nối, đánh dấu đoạn hụt), `TranscriptStore.swift` (`transcript.jsonl` cạnh phiên), `AudioRecorder.setPCMSink`, `RecorderPlugin`: `start({realtime})`, sự kiện `transcript`, `getTranscript()`.
- **Web** (chỉ nhánh native): công tắc "Chép trực tiếp", panel "Đang chép" (`live-transcript-panel.tsx`, state ở `features/recorder/live-transcript.ts`).
- Chi tiết vận hành: [docs/ios-app.md](../../docs/ios-app.md) mục "Chép trực tiếp".

## Chuyển sang ElevenLabs Scribe v2 Realtime (2026-10-19)

### Dữ kiện ElevenLabs đã dùng

| Hạng mục | Giá trị | Nguồn |
|---|---|---|
| WebSocket | `wss://api.elevenlabs.io/v1/speech-to-text/realtime`, query `model_id=scribe_v2_realtime`, `audio_format` (`pcm_8000`…`pcm_48000`, `ulaw_8000`), `language_code`, `commit_strategy` (`manual`/`vad`), `vad_silence_threshold_secs`, `include_timestamps`, `token` | [API reference](https://elevenlabs.io/docs/api-reference/speech-to-text/v-1-speech-to-text-realtime) |
| Gửi audio | `{"message_type":"input_audio_chunk","audio_base_64":…,"commit":false,"sample_rate":24000}`; `commit:true` kèm audio rỗng chốt phần đang dở (đã thử) | như trên |
| Nhận | `session_started`; `partial_transcript` (toàn văn đoạn đang dở, thay thế); `committed_transcript` rồi `committed_transcript_with_timestamps` (từ có `start`/`end`); lỗi theo `message_type` (`auth_error`, `quota_exceeded`, `rate_limited`, `session_time_limit_exceeded`…) | như trên |
| Token phía client | `POST https://api.elevenlabs.io/v1/single-use-token/realtime_scribe`, header `xi-api-key`, trả `{"token":"sutkn_…"}`; hết hạn 15 phút; dùng một lần | [Tokens](https://elevenlabs.io/docs/api-reference/tokens/create), [Client-side streaming](https://elevenlabs.io/docs/eleven-api/guides/how-to/speech-to-text/realtime/client-side-streaming.md) |
| Đóng khi im | Server đóng kết nối sau 15 giây không có tin nhắn nào của client; tự chốt sau khoảng 36 giây audio; nên gửi khúc 0,1–1 giây; chỉ mono | [Transcripts and commit strategies](https://elevenlabs.io/docs/eleven-api/guides/how-to/speech-to-text/realtime/transcripts-and-commit-strategies.md) |
| Giá | 0,39 USD/giờ (Scribe v2 theo tệp: 0,22 USD/giờ); giá không đổi theo gói, gói chỉ khác số giờ kèm theo | [API pricing](https://elevenlabs.io/pricing/api) |

Đo từ Mac (2026-10-19, key prod của chủ dự án, tổng khoảng 1 phút audio):

- `language_code=vi` và `pcm_24000` được nhận (`session_started` báo lại đúng cấu hình).
- Mốc từ tính từ khung audio **đầu tiên của kết nối**, cộng dồn qua các lần chốt (đoạn thứ hai bắt đầu ở 20,26 giây).
- Có `include_timestamps=true` thì mỗi lần chốt về cả `committed_transcript` lẫn `committed_transcript_with_timestamps` (cách nhau khoảng 40 ms).
- Mở lại WebSocket bằng token đã dùng → `auth_error`: **một token = một kết nối**.
- Cấp token không có thân request mà thiếu `Content-Length: 0` → HTTP 411 (đã sửa, test giả lập kiểm header này).
- Đường mã của backend (test `live_elevenlabs_realtime_mint_returns_a_single_use_token`) cấp token và dựng `url`; script trên Mac mở đúng `url` đó, stream 10 giây buổi họp Viettel từ phút 10:00 ở tốc độ thật → `committed_transcript_with_timestamps` tiếng Việt về ở giây thứ 11, khoảng 1 giây sau lệnh chốt cuối ("Nếu như do chính VCS tự phát triển, bọn em có một team…").

### Đã đổi

- **Backend** (`realtime.rs`): `REALTIME_PROVIDER` (mặc định ElevenLabs, `openai` = OpenAI như cũ). ElevenLabs: cấp token dùng một lần bằng key `ELEVENLABS_KEY_PATH` (key được đọc cả khi chỉ chép trực tiếp dùng ElevenLabs), `expiresAt` = lúc xin + 900 giây. Phản hồi giữ nguyên các trường cũ, thêm `provider`, `url` (WebSocket kèm cấu hình phiên, app chỉ thêm `token`), `sampleRate` (24000). Sổ quy theo USD (quỹ free, trần tổng) tính theo giá chép trực tiếp (`costRatio` = 0,39/0,22 với ElevenLabs); sổ giờ ElevenLabs `el:<tháng>` giữ chỗ, quyết toán, hoàn như sổ phút, quy đổi theo `elRatio` = 0,39/0,22 vì realtime trừ credit nhanh hơn 1,77 lần (`ELEVENLABS_MONTHLY_HOURS` là giờ Scribe v2 tương đương). Sổ giờ đầy hoặc ElevenLabs đã báo hết quota → 403 `PROVIDER_HOURS`. Cấp token gặp 402 → dấu hết quota tháng như chép lời theo tệp; 401/403 → ngắt key (cùng key với chép lời theo tệp thì ngắt cả hai). App báo `quotaExceeded` khi WebSocket trả `quota_exceeded` → cũng đặt dấu hết quota tháng. `transcribe_runs` của lượt realtime ghi thêm `provider`, `costS`, `elS`; `admin usage --recompute` dùng `costS` cho quỹ free và trần tổng.
- **Swift** (`RealtimeTranscriber.swift`): hai giao thức theo `provider` của từng token (thiếu = OpenAI). Vẫn PCM16 24 kHz khung 100 ms cho cả hai (ElevenLabs nhận `pcm_24000`, khỏi đổi tần số lấy mẫu giữa chừng khi backend lùi về OpenAI). ElevenLabs: token đã mở một socket thì nối lại hay đổi token đều xin token mới; câu = `committed_transcript_with_timestamps`, id `<tokenId>-<n>`, mốc = vị trí khung đầu của socket + `start` của từ đầu; chữ tạm = `partial_transcript`; khi nghỉ socket gửi chốt rỗng. Lỗi của socket → bỏ socket, nối lại có backoff bằng token mới; `quota_exceeded` → dừng chép trực tiếp (ghi vẫn chạy). Giữ nguyên: backoff, 30 giây audio chờ khi nối lại, dấu "mất đoạn này", không đụng tệp AAC.
- **Web**: chữ giúp của công tắc không nhắc OpenAI ("Gửi âm thanh tới dịch vụ chép lời…").
- **App cũ đang cài** (bản 17/10) không biết `provider`: gặp backend mặc định ElevenLabs sẽ mở OpenAI bằng token ElevenLabs và báo lỗi (ghi vẫn chạy). Cài app mới trước hoặc cùng lúc deploy backend; hoặc build backend với `REALTIME_PROVIDER=openai` tới khi cài app mới.

## Cần iPhone thật

Gate 0 bước 2 và toàn bộ Success Criteria dưới đây (trừ dòng build) — danh sách bấm thử ở mục "Lần cắm iPhone tới" của [plan](./plan.md), làm lại với ElevenLabs (mục 14–18). Trước khi build prod: thêm email chủ sản phẩm vào `REALTIME_EMAILS` trong `.env.production`.

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

- `POST /api/realtime/token` (cần đăng nhập): trả token tạm thời; key đọc từ tệp `OPENAI_KEY_PATH` lúc khởi động, không bao giờ trả về client.
- Chặn chi phí: danh sách `REALTIME_EMAILS`; phút tính chung sổ `transcribe_usage` với chép lời cả file (R3).
- Số phút theo số giây app báo khi đổi token hoặc dừng ghi (`POST /api/realtime/usage`); không báo thì tính trọn phần đã giữ.

## Related code files

- Create: `web/ios/App/App/Recorder/RealtimeTranscriber.swift`
- Create: `web/ios/App/App/Recorder/TranscriptStore.swift`
- Modify: `web/ios/App/App/Recorder/AudioRecorder.swift` — thêm tap PCM cho nhánh stream
- Modify: `web/ios/App/App/Recorder/RecorderPlugin.swift` + `.m` — event `transcript`, `getTranscript`, cờ bật/tắt realtime
- Modify: `web/src/features/recorder/native-recorder.ts`, `web/src/pages/record/record-state.tsx`, `web/src/pages/record/native-record-card.tsx`; create `web/src/features/recorder/live-transcript.ts`, `use-live-transcript.ts`, `web/src/pages/record/live-transcript-panel.tsx` (chỉ nhánh native)
- Create: `server/scribe-be/src/realtime.rs`, `src/models/realtime_token.rs` — endpoint token + tính phút
- Modify: `server/scribe-be/src/routes.rs`, `src/config.rs` + `build.rs` (`REALTIME_EMAILS`), `src/admin.rs` (đối soát), `src/transcribe/mod.rs` (dùng chung lỗi hạn mức)

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
- [x] `xcodebuild build` EXIT 0, 0 warning Swift; `cargo clippy` sạch; `tsc` EXIT 0 (2026-10-17).
- [x] Bản ElevenLabs: `xcodebuild` Simulator EXIT 0, 0 warning Swift; Rust test token giả lập + sổ phút/sổ giờ trên Mongo dev; thử thật từ Mac ra câu tiếng Việt (2026-10-19).

## Rủi ro

| Rủi ro | Mức | Giảm thiểu |
|---|---|---|
| Không tạo được token tạm thời cho phiên transcription | Cao | Gate 0 bước 1; lối lui là relay qua backend (đo RAM `.65`) |
| iOS cắt WebSocket khi khoá máy lâu | Cao | Gate 0 bước 2; audio session đang chạy giữ app sống, nối lại có backoff, Phase 6 bù phần hụt |
| Phiên realtime có trần thời lượng nhỏ hơn buổi họp | Trung bình | Nối lại liên tục với token mới (Gate 0 bước 1) |
| Chi phí vượt ngân sách | Trung bình | Phút chung hạn mức chép lời + `REALTIME_EMAILS`; sổ giờ ElevenLabs tính realtime theo giá quy đổi |
| App bị tắt trước khi báo số giây | Thấp | Phần giữ chỗ (tối đa 10 phút mỗi token) tính trọn — lệch về phía an toàn cho chi phí |
| Audio rời máy | Chấp nhận có điều kiện | Công tắc realtime (R5); chỉ chạy khi bật |

## Deploy 17/10 11:11
Backend + web lên prod từ `d409cc8` (chủ dự án cho phép). `REALTIME_EMAILS` = tài khoản chủ dự án. Sao lưu `/root/scribe-backup-20261017-111120`,
binary sha256 `c8cc2c429fb21907…`, `index-C_t9Qk5G.js`, health ok, `NRestarts=0`, `POST /api/realtime/token` chưa đăng nhập trả 401.
App iPhone có chép trực tiếp đã cài lên iPhone 15 Pro Max lúc 11:10 (cài hết hạn ~24/10). Còn đo trên máy thật: mục 14–17 của checklist.

## Deploy 19/10 08:54 (ElevenLabs Realtime)
Chủ dự án cho phép ("bạn tự làm cho tôi luôn các việc"). Backend + web từ `9305861` (lần build đầu dùng lại binary cũ do cache CARGO_TARGET_DIR,
phát hiện qua sha trùng → `cargo clean -p` rồi build lại). Sao lưu `/root/scribe-backup-20261019-085243`, binary sha256 `2e216e74c060160f…`,
`index-ClbfkLLj.js`, health ok, `NRestarts=0`. Từ server xin single-use token realtime: HTTP 200. App mới cài lên iPhone 15 Pro Max lúc 08:56
(hết hạn ~26/10). Giữ hai quyết định của agent, chờ chủ dự án xác nhận: giờ realtime tính 1,77× trên sổ giờ ElevenLabs (theo giá 0,39/0,22 USD), và app báo
`quotaExceeded` được dừng ElevenLabs tới tháng sau. Còn đo trên máy: mục 14–18 của checklist.
