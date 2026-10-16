---
phase: 2
title: "Recorder + upload"
status: pending
priority: P1
effort: "L"
dependencies: [1]
---

# Phase 2: Recorder + upload

# (gộp Phase 2 + Phase 3 của bản 1)

## Overview

Engine ghi âm nền và đường upload trong **một** phase. Bản 1 tách đôi để "nghiệm thu engine độc lập với mạng", nhưng Phase 0 đã chứng minh engine trên máy thật rồi — tách ra chỉ tốn ba lượt sửa `RecordView.vue` và một UI tạm rồi xoá. Giữ **checkpoint nội bộ** cho engine, không giữ ranh giới phase.

## Requirements

**Chức năng**
- `start(opts)` / `stop()`, xoay đoạn theo phút (**mặc định 1 phút** — D6)
- Sống sót: khoá màn hình, chuyển app, cuộc gọi, Siri, rút/cắm AirPods, media services reset
- Mỗi đoạn đóng xong → upload nền trong lúc vẫn đang ghi đoạn kế
- Bền vững: app bị kill / mất mạng / offline **> 24h** → tự tiếp tục khi mở lại
- Upload xong → xoá file cục bộ, giải phóng đĩa máy
- Đoạn đang mở bị mất do OS kill → **báo cáo rõ**, không im lặng bỏ qua

**Phi chức năng**
- `AudioRecorder` thuần AVFoundation, **không import Capacitor** (điều kiện để sau này gắn UI native)
- `SegmentStore` là **actor / serial queue duy nhất** — không luồng nào ghi thẳng vào `session.json`
- Không `!` force-unwrap, không `try!`; 0 warning
- **Không** dựng `RecorderDelegate` protocol, **không** tách `ApiClient` — dùng closure `onEvent` và method riêng trong `UploadQueue` (D7)

## Architecture

```
AudioRecorder.swift    AVAudioSession + AVAudioEngine + xoay đoạn
   │ onEvent closure
   ├──▶ RecorderPlugin.swift   cầu Capacitor → event lên WebView
   └──▶ UploadQueue.swift      serial, đọc/ghi qua SegmentStore
              │
        SegmentStore (actor)   chủ sở hữu DUY NHẤT của session.json
```

### SegmentStore — sửa lỗi Critical #3 của bản 1

Bản 1 để `AudioRecorder` (chạy trên **audio render thread**) và `UploadQueue` cùng read-modify-write `session.json` không khoá. Hậu quả: mất đoạn khỏi manifest, hoặc mất cờ `uploaded` → upload trùng → **trùng bản ghi + đúp quota** (BE không có dedupe, `file::create` là `insert_one` thuần).

Bắt buộc:
- Swift `actor`, phơi thao tác hẹp: `appendSegment`, `markUploaded(index:fileId:)`, `markFailed`. Không có API đọc-cả-ghi-cả.
- Ghi **atomic**: `Data.write(to:options:[.atomic])`, protection `completeUntilFirstUserAuthentication`.
- Có `schemaVersion`; decode khoan dung với khoá thiếu/lạ (để rollback code không làm manifest cũ thành không đọc được).
- **Startup reconcile**: hợp nhất `session.json` với danh sách `seg-*.m4a` thật trên đĩa. Manifest hỏng không được phép giấu audio đang tồn tại.

```
Documents/recordings/<userId>/<sessionId>/
  session.json   { schemaVersion, sessionId, startedAt, segmentSeconds,
                   segments: [{ index, file, durationS, bytes, uploaded, remoteFileId, attempts }] }
  seg-000.m4a
```

`<userId>` trong đường dẫn: nếu đổi tài khoản trên cùng máy, đoạn của người trước không được upload vào tài khoản mới.

### Giao thức upload — bỏ resumable (D6)

```
POST /api/recordings/init     { name: "<tên phiên> — đoạn N" }  → { sessionId }
POST /api/recordings/chunk?session=&offset=                      (1 MB/lần, body limit BE 8MB)
POST /api/recordings/finalize { session, durationS }             → { fileId, name, durationS }
```

`durationS` lấy từ **số frame đã ghi**, không phải ffprobe.

Tên đoạn **tất định**: `<sessionId>#<index>` — phục vụ hai việc: đối chiếu với `GET /api/files` khi `finalize` trả 404, và **cho phép Phase 3 gom các đoạn cùng phiên thành một dòng trong Lịch sử ghi** (V3). Định dạng này là hợp đồng giữa Phase 2 và Phase 3, không được đổi tuỳ tiện. <!-- Updated: Validation Session 1 - V3 gom nhóm theo phiên -->

### Phân loại lỗi — BA nhóm, không phải hai

| Nhóm | Mã | Xử lý |
|---|---|---|
| **Tạm thời** | 5xx, timeout, mất mạng | Backoff luỹ tiến, retry |
| **Làm lại được** | `400 offset không liền mạch` · `404 session ghi không tồn tại` · `400 chưa có dữ liệu ghi` | Bỏ session server, `init` lại từ offset 0. **Trước khi làm lại `finalize`: đối chiếu `GET /api/files` theo tên tất định** — 404 có thể nghĩa là nó ĐÃ commit và mình mất response |
| **Kết thúc** | 403, audio hỏng | Dừng, báo UI, **giữ file cục bộ** |

`401` **không** thuộc nhóm nào ở trên: nó kích hoạt `SessionAuth.refreshSession()` rồi retry một lần (D5.1). Bản 1 xếp 401 vào "vĩnh viễn" — sai, và là lỗi chắc chắn xảy ra sau mỗi 12 giờ.

`413` quota: BE chặn ở **cả** `init` và `chunk`, và `chunk` **xoá `.part`** — nên sau 413 phải coi là "làm lại được", không phải resume theo offset cũ.

### Đoạn đang mở khi bị kill

`AVAudioFile` chỉ ghi moov atom khi nhả tham chiếu. OS kill → file cụt, không đọc được, `infer` cũng từ chối. Giảm thiểu: đoạn 1 phút. Khi khởi động, phát hiện `seg-NNN.m4a` cuối không đọc được bằng `AVAudioFile(forReading:)` → **báo cho người dùng biết mất khoảng nào**, xoá file, không đẩy vào queue.

### Bốn bài học spike

Phải hiện diện trong code kèm comment giải thích: `aacBitRate(sampleRate:channels:)` + fallback bỏ `AVEncoderBitRateKey` · `setPreferredInput(builtInMic)` · retry 0.5s khi khởi động lỗi · `completeUntilFirstUserAuthentication`. Thêm: **assert 4 byte ftyp = `M4A `** sau khi đóng đoạn đầu tiên (nếu writer đổi, brand đổi, `infer` có thể nhận thành mp4 vì Video đăng ký trước Audio).

### Hợp đồng interface với Vue — viết trước khi code Swift

`RecordView.vue` hiện tiêu thụ (`use-recorder.ts:420`): `{ recording, starting, finalizing, elapsedS, audioLevel, previewStream, error, start, stop, dispose }` + `supportsScreenCapture()` ở module scope.

Hai chỗ **không** drop-in được (bản 1 chỉ so tên field, không so chữ ký):

| Thành viên | Web | Native | Cách xử lý |
|---|---|---|---|
| `start` | `start(src: RecordSources)` — `RecordView.vue:106` | cần `segmentMinutes` | Hợp đồng chung: `start(opts: RecordSources & { segmentMinutes?: number })` |
| `stop` | `Promise<RecordResult \| null>`, `RecordView.vue:93,99` deref `result.fileId` để chạy `sp.createJob` | N đoạn, upload bất đồng bộ, **không có `fileId`** lúc stop | Native trả `null`; `RecordView.toggle()` phải rẽ nhánh `isNative` cho đường split |

**`useSplitter` (128 LOC, 9 điểm dùng trong `RecordView.vue`: 7, 12, 89, 99, 105, 116–119, 211–216) không xuất hiện trong phase nào của bản 1.** Trên iOS đường split sau ghi là **thừa** — engine đã cắt sẵn theo phút. Khối tiến độ job (`RecordView.vue:210-217`) là code chết trên iOS.

## Related Code Files

- Create: `web/ios/App/App/Recorder/AudioRecorder.swift` — port có tái cấu trúc từ `ios-spike/ScribeSpike/Recorder.swift`
- Create: `web/ios/App/App/Recorder/SegmentStore.swift` — actor
- Create: `web/ios/App/App/Recorder/UploadQueue.swift` — gồm cả HTTP (không tách `ApiClient`)
- Create: `web/ios/App/App/Recorder/RecorderPlugin.swift` + `.m`
- Create: `web/src/composables/use-recorder-native.ts`
- Reference (không sửa): `ios-spike/`

## Implementation Steps

1. Viết **hợp đồng interface** ở trên vào `use-recorder-native.ts` dưới dạng type, trước khi viết Swift.
2. `SegmentStore` actor + atomic write + startup reconcile. Nền của mọi thứ còn lại.
3. Port `AudioRecorder` từ spike, thay log ad-hoc bằng closure `onEvent`, giữ 4 bài học + assert ftyp.
4. **Checkpoint engine** (không phải phase gate) — chạy bộ đo dưới đây trước khi nối upload.
5. `UploadQueue`: serial, ba nhóm lỗi, reconcile 404-finalize, `NWPathMonitor` đánh thức.
6. Khởi động app: quét mọi `session.json`, đưa `uploaded:false` vào queue, xử lý đoạn cụt.
7. `RecorderPlugin` + event lên WebView.

## Success Criteria

**Checkpoint engine (bước 4):**
- [ ] Ghi **60 phút** khoá màn hình liên tục, **không sạc** → mọi đoạn đọc lại đúng thời lượng; log ghi `nguồn=KHÔNG-sạc`
- [ ] Pin tụt **≤ 12%**, đo qua **Cài đặt → Pin → theo app** (không dùng `batteryLevel` — Phase 0 đã chứng minh nó dưới ngưỡng phân giải 1%)
- [ ] **Cuộc gọi đến** giữa phiên → engine dựng lại, đoạn trước và sau đều đọc được
- [ ] Rút/cắm AirPods → xoay đoạn theo format mới, không đoạn nào hỏng
- [ ] Xoay đoạn < 50ms; đoạn đầu có ftyp `M4A `

**Upload:**
- [ ] Ghi 10 phút / đoạn 1 phút → **10 đoạn trong Lịch sử ghi**, phát và tua từng đoạn
- [ ] Chế độ máy bay giữa lúc upload → tắt đi → tự tiếp tục, không trùng bản ghi
- [ ] Kill app khi còn đoạn chưa upload → mở lại tự upload nốt, **đúng một** `.part` phía server
- [ ] Kill app **giữa lúc đang ghi** → mở lại báo rõ mất bao nhiêu giây của đoạn đang mở
- [ ] Giả lập session hết hạn (hạ `session_ttl_secs` trên BE scratch xuống 3 phút) → ghi 15 phút → **mọi đoạn vẫn upload** nhờ auto-refresh
- [ ] Giả lập offline > 24h (chỉnh đồng hồ hoặc chờ) → session server bị sweep → đoạn vẫn upload được nhờ nhóm "làm lại được"
- [ ] Đoạn upload xong bị xoá khỏi đĩa máy (kiểm bằng bridge debug, **không** bật `UIFileSharingEnabled`)
- [ ] `git diff --stat server/` trống; `xcodebuild build` EXIT 0, 0 warning

## Risk Assessment

**Pin vượt ngưỡng (Trung bình).** Checkpoint bước 4 chặn trước khi đầu tư vào upload. Vượt → hạ sample rate xuống 24 kHz hoặc chuyển sang `AVAudioRecorder`.

**Cuộc gọi không cho resume (Trung bình).** Nếu `shouldResume=false` và engine không dựng lại được thì kết thúc phiên và lưu phần đã ghi, báo rõ — không giả vờ còn đang ghi.

**Trùng bản ghi do reconcile sai (Trung bình).** BE không có idempotency. Tên tất định `<sessionId>#<index>` + đối chiếu `GET /api/files` là hàng phòng thủ duy nhất. Phải có test riêng: giết mạng ngay sau khi gửi `finalize`.

**Rollback (đã sửa so với bản 1).** Bản 1 nói "gỡ subscriber là về Phase 2". Không đúng: `session.json` đã ghi xuống đĩa theo schema mới; revert code mà parser cũ không khoan dung thì mọi phiên tồn thành vô hình. Vì vậy `schemaVersion` + decode khoan dung là **bắt buộc**, không phải tuỳ chọn. Rollback cũng không khôi phục được file cục bộ đã xoá sau upload.
