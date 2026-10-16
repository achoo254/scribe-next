# Review: ghi âm native iOS + hàng đợi tải lên (Scribe)

Ngày: 2026-10-15 · Người review: code-reviewer · Chế độ: chỉ đọc, chưa chạy trên máy thật.

## Phạm vi

- Swift: `web/ios/App/App/Recorder/{AudioRecorder,SegmentStore,UploadQueue,RecorderPlugin}.swift`, `GoogleAuthPlugin.swift`, `NativeFilesPlugin.swift`, `MainViewController.swift`, `SceneDelegate.swift` (kèm thay đổi chưa commit ở `AudioRecorder.swift:272-279`).
- TS: `features/recorder/native-recorder.ts`, `use-native-recorder.ts`, `pages/record/native-record-card.tsx`, `lib/download.ts`, `lib/firebase.ts`, `auth/auth-context.tsx`.
- Đối chiếu với: `plans/260728-1609-.../phase-02-recorder-va-upload.md`, `server/scribe-be/src/recordings.rs`, `files.rs`, `error.rs`, `retention.rs`, `rate_limit.rs`, `ios-spike/ScribeSpike/Recorder.swift`.
- Kiểm tra đã chạy: `tsc --noEmit` exit 0; `biome check` trên 15 file TS liên quan: sạch. Swift **chưa** build (`xcodebuild` không chạy trong phiên này).

## Tổng quan

Phần lưu trữ đã đúng thiết kế: `SegmentStore` actor, ghi atomic, decode khoan dung, reconcile. Engine port gần như nguyên bản từ spike. Lỗi tập trung ở **UploadQueue** (idempotency của finalize, thứ tự sign-out, đánh thức bị mất) và ở **những đường dừng âm thầm của engine** mà spike chưa đo (engine tự dừng do đổi cấu hình, lỗi ghi đĩa). Không thấy lỗi crash chắc chắn; có hai data race giữa luồng audio và main.

---

## Cao (High)

### H1. Mất phản hồi `finalize` thì vẫn tạo bản ghi trùng
`UploadQueue.swift:191-203`, `:106-121`, `:143-146`

- **Tình huống:** `finalize` đã commit ở server (`recordings.rs:212` `file::create`), nhưng phản hồi bị mất (mạng rớt, chế độ máy bay, timeout). `json()` ném `.transient` → nhánh `catch Outcome.transient` gọi `try? findRecording(...)`. Lúc này mạng vẫn đang hỏng nên `GET /api/files` cũng lỗi, `try?` trả `nil` → ném `.transient` → `scheduleRetry`. Lần thử sau `upload()` chạy lại từ `init` và **không hề tra tên trước** → init/chunk/finalize mới → **hai bản ghi `<sessionId>#<index>`, tốn quota gấp đôi**.
- Cùng kết quả khi: (a) iOS kill app sau khi `finalize` thành công nhưng trước `markUploaded`; (b) `markUploaded` ném lỗi (ví dụ đầy đĩa) → rơi vào `catch` chung ở `:143` → retry → upload lại.
- Đây đúng là bài test mà plan bắt buộc ("giết mạng ngay sau khi gửi finalize"); code hiện tại sẽ không qua.
- **Cách sửa:** lưu một dấu vào manifest trước khi gửi finalize, ví dụ `finalizeSentAt` (thêm `SegmentStore.markFinalizing`). Ở đầu `upload()`, nếu segment có dấu này thì gọi `findRecording(named:)` trước, và **bắt buộc lời gọi đó phải thành công**: lỗi thì ném `.transient`, không được `init` lại. Chỉ khi biết chắc chưa có bản ghi mới `init` lại. Gom chung với H6 (lưu `serverSession`).

### H2. Engine có thể dừng âm thầm, UI vẫn báo "Đang ghi"
`AudioRecorder.swift:352-398`, `:402-406`

- **Tình huống:** AVAudioEngine **tự dừng** và phát `AVAudioEngineConfigurationChange` khi số kênh hoặc sample rate của phần cứng đổi. Việc này có thể xảy ra ngay sau `setActive`/`setPreferredInput` (bài học 3 của spike: route đổi bất đồng bộ), khi đổi tai nghe Bluetooth, hoặc với lý do route mà code không xử lý (`.categoryChange`, `.override`, `.routeConfigurationChange`). Code chỉ khởi động lại với `.oldDeviceUnavailable/.newDeviceAvailable` và không nghe notification này. Khi engine dừng: tap không còn buffer, `state` vẫn `.recording`, còn `tickElapsed` tính bằng đồng hồ tường nên **đồng hồ vẫn chạy**. Người dùng thấy đang ghi trong khi không có frame nào được ghi → **mất audio âm thầm** cho tới khi bấm Dừng.
- Spike đo 63 phút chỉ với mic tích hợp, chưa đi qua đường này. Mục "Rút/cắm AirPods" trong plan vẫn chưa tick.
- **Cách sửa:** quan sát `.AVAudioEngineConfigurationChange` (object: `engine`) → `restartEngineChain(reason:)`. Thêm watchdog trong `tickElapsed`: nếu `state == .recording && pausedAt == nil && !engine.isRunning` (hoặc `currentFrames` không tăng trong khoảng 3 giây) → restart, và phát `notice` nếu không dựng lại được.

---

## Trung bình (Medium)

### M1. Sign-out có thể khởi động lại hàng đợi cho tài khoản cũ; upload đang chạy không bị huỷ (ranh giới tin cậy)
`RecorderPlugin.swift:175-184`, `:208-214`; `UploadQueue.swift:59-75`, `:105`

- **Tình huống:** đăng xuất khi đang ghi. `signOut` gọi `recorder.stop()`, hàm này phát `segmentClosed` đồng bộ → `handle` tạo `Task { appendSegment; queue.start(userKey: A) }`. Ngay sau đó lại tạo `Task { queue.stop() }`. Hai Task không cấu trúc **không có thứ tự**. Nếu `stop()` chạy trước thì `start(userKey: A)` sẽ **bật lại hàng đợi cho A** sau khi đã đăng xuất. Nếu B đăng nhập xong trước khi một request của A nhận 401 (ví dụ A đang chờ backoff `retryTask` rồi `kick()`), các đoạn của A được `init` bằng cookie của B → **audio của A vào tài khoản B**.
- Ngoài ra `stop()` không huỷ lượt `upload()` đang chạy. Các chunk còn lại đi với cookie mới → 403 → đoạn của A bị đánh dấu `failure` vĩnh viễn (xem M3). Hoặc gặp 401 → phát `needsSignIn` → JS gọi `logout()` lần nữa (`use-native-recorder.ts:44`), có thể đăng xuất luôn B nếu B vừa đăng nhập.
- Khả năng xảy ra thấp nhưng đây là rò dữ liệu giữa hai tài khoản.
- **Cách sửa:** `segmentClosed` không được gọi `start(userKey:)`. Dùng `kick(ifUser: userKey)`, hàm này bỏ qua khi `self.userKey != userKey`; chỉ `syncUploads` (lúc đăng nhập) mới được đặt userKey. Giữ handle của Task drain, `stop()` thì `cancel()` nó. `URLSession.data(for:)` tôn trọng cancellation, và trong `upload()` cần kiểm tra `Task.isCancelled` / `self.userKey == pending.userKey` trước mỗi request.

### M2. Đánh thức bị mất: đoạn cuối có thể nằm chờ tới lần mở app sau
`UploadQueue.swift:77-81`, `:103-108`

- **Tình huống:** do actor cho phép reentrancy, trong lúc `drain` đang `await store.pending(...)` hoặc `await publishCounts(...)`, một đoạn mới được `appendSegment` rồi `queue.start` → `kick()` thấy `running == true` nên bỏ qua. Nếu `pending` đã được đọc trước khi append thì `drain` thấy rỗng, `return`, và `running = false`. Đoạn mới **không được tải** cho tới trigger kế tiếp (đoạn sau, đổi mạng, mount/đăng nhập). Với đoạn **cuối cùng** sau khi Dừng thì có thể phải chờ tới lần mở app sau. Dễ xảy ra khi đang có tồn đọng (mạng chậm).
- **Cách sửa:** thêm cờ `rerun`. `kick()` khi `running` thì đặt `rerun = true`. Cuối `drain`, nếu `rerun` thì xoá cờ và chạy thêm một vòng trước khi hạ `running`.

### M3. 403/404/400 không phải JSON (Cloudflare, nginx) bị coi là lỗi vĩnh viễn; đoạn lỗi không bao giờ được thử lại
`UploadQueue.swift:235-245`, `:131-133`; `SegmentStore.swift:162`

- **Tình huống:** Cloudflare đứng trước backend. Một challenge hoặc WAF trả 403 HTML, khi đó `reply["error"]` rỗng → `.terminal("HTTP 403")`. Một 404/400 HTML của proxy (message `"HTTP 404"`) không khớp `isRedo` → cũng `.terminal`. Đoạn đó bị gắn `failure` và **bị lọc khỏi `pending` vĩnh viễn**: không retry, không có nút thử lại, file nằm mãi trên máy. Chỉ một sự cố biên ngắn cũng có thể khoá cả loạt đoạn.
- **Cách sửa:** chỉ coi là terminal/redo khi body là JSON của backend có khoá `error`. Không có thì xếp vào `.transient`. Nên có thêm cách xoá `failure` (ví dụ `syncUploads` reset các failure cũ hơn N giờ, hoặc nút "Thử lại" trên UI).

### M4. Thử lại luôn tạo `.part` mồ côi trên server, trái tiêu chí "đúng một `.part`"
`UploadQueue.swift:177-190`

- **Tình huống:** mỗi lỗi transient hoặc mỗi lần app bị kill giữa chunk → lần sau `init` mới và để lại `.part` cũ. Phần này vẫn được `user_inflight_bytes` tính vào quota (`recordings.rs:240-252`) cho tới khi sweep chạy (sau 24h, `retention.rs:14`). Với đoạn 10 phút (~7 MB) và mạng chập chờn, quota "đang dở" tăng nhanh và có thể dẫn tới **413 giả**, làm hàng đợi dừng (`quotaFull`).
- **Cách sửa:** lưu `serverSession` vào manifest ngay sau `init`. Lần thử sau dùng lại session đó và gửi từ `offset=0`: backend cho phép `offset ≤ current_len` (`recordings.rs:90`), ghi đè cùng dữ liệu nên an toàn. Nếu nhận 404 "session ghi không tồn tại" thì mới `init` lại.

### M5. Lỗi ghi file bị nuốt: đầy đĩa nghĩa là mất audio âm thầm
`AudioRecorder.swift:327-331`

- **Tình huống:** `target.write(from:)` ném lỗi (đầy đĩa, lỗi encoder) → chỉ `log.error` rồi `return`. `currentFrames` không tăng nên đoạn không bao giờ xoay. UI vẫn "Đang ghi". Khi Dừng, `seconds < 0.5` → **file bị xoá** (`:260-263`). Toàn bộ phần ghi sau khi đầy đĩa mất mà không có thông báo nào.
- **Cách sửa:** đếm số lần lỗi ghi liên tiếp. Quá ngưỡng (ví dụ 10 buffer) thì `DispatchQueue.main.async { fail("Không ghi được vào bộ nhớ máy: …") }` để đoạn hiện tại đóng lại và người dùng biết. Đồng thời đừng xoá đoạn có lỗi ghi mà vẫn còn frame hợp lệ.

### M6. Route change hoặc media reset trong lúc đang bị gián đoạn sẽ kết thúc cả phiên; không có đường resume nếu thiếu `.ended`
`AudioRecorder.swift:368-398`, `:186-197`

- **Tình huống 1:** đang có cuộc gọi (`.began`, `state` vẫn `.recording`, engine đã dừng) mà người dùng nối tai nghe hoặc loa xe (`.newDeviceAvailable`) → `restartEngineChain` → `setActive(true)` thất bại vì cuộc gọi đang giữ phiên audio → `fail()` → **cả phiên ghi kết thúc**, dù cuộc gọi xong lẽ ra phải ghi tiếp được.
- **Tình huống 2:** Apple không đảm bảo `.began` luôn có `.ended` đi kèm (Siri, app khác chiếm mic mà không trả lại). Khi đó recorder kẹt ở "recording" với engine đã dừng và không bao giờ tự thử lại.
- **Cách sửa:** trong `handleRouteChange` và `mediaServicesWereReset`, nếu `pausedAt != nil` (đang gián đoạn) thì chỉ ghi nhận, chờ `.ended`. Thêm observer `UIApplication.didBecomeActiveNotification`: nếu `state == .recording && pausedAt != nil` thì thử `restartEngineChain`, và phát `notice` nếu không được.

### M7. Upload dừng khi app xuống nền sau lúc Dừng ghi, và không được đánh thức khi quay lại
`UploadQueue.swift:151-159`; không có `beginBackgroundTask` hay observer foreground ở đâu cả

- **Tình huống:** sau khi Dừng thì không còn audio nền, iOS treo app sau vài giây. Request đang chạy thất bại → `transient` → backoff tăng dần tới 300 giây. Khi mở app lại không có gì gọi `kick()` (`visibilitychange` chỉ gọi `refresh()`, còn `syncUploads` chỉ chạy khi email đổi). Hàng đợi tồn đọng sau một buổi ghi offline có thể phải chờ tới 5 phút mỗi lần, dù app đang mở.
- **Cách sửa:** bọc mỗi lượt `upload()` trong `UIApplication.beginBackgroundTask`. Ở `willEnterForeground`, gọi `networkBack()` (reset backoff + `kick`), hoặc cho JS gọi `syncUploads` khi trang hiện lại.

---

## Thấp (Low)

| # | File:dòng | Tình huống → kết quả sai | Sửa |
|---|---|---|---|
| L1 | `AudioRecorder.swift:202`, `:257`, `:265`, `:296-305` | `closeSegment`/`openSegment` chạy trên luồng tap nhưng đọc `sessionDir`, `sessionId`, `checkedBrand` không qua khoá, trong khi `stop()`/`fail()` gán nil trên main. Nếu xoay đoạn trùng đúng lúc bấm Dừng: (a) đoạn N vừa đóng không phát `segmentClosed` (`guard let dir…` trả về sớm). Không mất audio vì reconcile sẽ thêm lại, nhưng phải chờ tới `syncUploads` sau. (b) `openSegment` ném lỗi → `fail()` trên recorder đã idle → hiện lỗi "Không mở được đoạn ghi mới" sau một lần Dừng bình thường. Data race trên `String?`/`URL?` cũng có thể gây crash hiếm. Bản sửa chưa commit (dispatch sang main) đúng hướng nhưng chưa xử lý phần này. | Chụp `sessionId`/`dir` vào trạng thái được bảo vệ bởi `fileLock` khi mở đoạn; `closeSegment` chỉ dùng bản chụp đó. `rotate` bỏ qua khi `state != .recording` (đọc dưới khoá). |
| L2 | `RecorderPlugin.swift:121-131`, `:160-165` | Capacitor gọi method plugin trên queue nền, còn `self.userKey` được gán và `recorder.state`/`recorder.sessionId` được đọc ngoài main, đua với `handle()` và `stop()` trên main. | Bọc thân `start`/`syncUploads` trong `DispatchQueue.main.async` hoặc `Task { @MainActor in }` trước khi chạm state. |
| L3 | `SegmentStore.swift:140-141` | `markUploaded` xoá file **trước** khi ghi manifest. Nếu ghi manifest lỗi thì `uploaded` vẫn false, file đã mất → lần sau `upload()` báo terminal "Tệp đoạn ghi không đọc được" cho một đoạn thực ra đã lên server (UI báo sai "bị từ chối"). | Ghi manifest trước, xoá file sau. |
| L4 | `SegmentStore.swift:201`, `:209` | `Int(name.dropFirst(4).prefix(3))` cắt `seg-1000.m4a` thành 100. Từ index 1000 trở đi (khoảng 16,7 giờ với đoạn 1 phút, sớm hơn nếu bị gián đoạn nhiều) reconcile gán trùng index → trùng tên `<sid>#100` → `findRecording` có thể khớp nhầm, đánh dấu đã tải và **xoá một đoạn chưa tải**. | Parse phần giữa `seg-` và `.m4a` (`name.dropFirst(4).dropLast(4)`). |
| L5 | `AudioRecorder.swift:36`, `:362-365` | Sau `mediaServicesWereReset`, Apple yêu cầu tạo lại mọi đối tượng audio. `engine` là `let` nên bị dùng lại → `start` dễ thất bại → `fail()`. | Đổi `engine` thành `var`, tạo `AVAudioEngine()` mới trong nhánh reset. |
| L6 | `UploadQueue.swift:134-137`; `use-native-recorder.ts:36-45` | Đang ghi mà cookie hết hạn: mỗi đoạn đóng lại gọi `start` → 401 → `needsSignIn` mỗi phút → loạt toast dồn lại khi mở khoá máy. Đăng xuất thủ công lúc đang upload cũng sinh `needsSignIn` giả và gọi `logout()` lần hai. | Chỉ phát `needsSignIn` một lần cho tới lần `syncUploads` kế tiếp; bỏ qua 401 khi `userKey == nil` (đã sign-out). |
| L7 | `RecorderPlugin.swift:203-207` | `level` (~2 lần/giây) và `elapsed` (1 lần/giây) vẫn `notifyListeners` khi WebView bị treo lúc khoá máy. Trong 1 giờ có khoảng 10 nghìn lần gọi evaluateJavaScript bị dồn lại. Chưa kiểm chứng trên máy, có thể gây giật hoặc tốn bộ nhớ khi mở khoá. | Không phát `level`/`elapsed` khi `applicationState != .active`; JS đã `refresh()` lúc trang hiện lại. |
| L8 | `AudioRecorder.swift:313`, `:269-270` | Biến cục bộ `target` trong `process()` vẫn giữ `AVAudioFile` khi `closeSegment` đọc `fileSize`/brand. Nếu ARC không nhả sớm (bản debug) thì moov chưa được ghi, nên `bytes` trong manifest thấp hơn thực tế. Không ảnh hưởng upload vì upload đọc file mới. | Truyền kết quả đóng ra ngoài sau khi `process` trả về, hoặc tính `bytes` lúc upload. |
| L9 | `SegmentStore.swift:234-238` | `sessionDirs` sắp theo UUID, không theo thời gian. Comment ghi "oldest first" là sai: thứ tự tải giữa các phiên là ngẫu nhiên. | Sắp theo `startedAt` trong manifest hoặc theo `creationDateKey` (đã yêu cầu nhưng không dùng). |
| L10 | `native-recorder.ts:130-175`; `use-native-recorder.ts:56-66` | `attach()` đăng ký listener tuần tự bằng `await`. Nếu `dispose()` chạy trước khi xong (StrictMode ở dev, unmount nhanh) thì listener đăng ký sau sẽ không bao giờ bị gỡ → sự kiện bị xử lý hai lần (toast, `logout`). | Dùng cờ `disposed`: listener nào đăng ký xong sau khi dispose thì gỡ ngay. |
| L11 | `use-native-recorder.ts:72-85` | `syncUploads` reject (ví dụ lỗi bridge) → unhandled rejection, không có thông báo. | `.catch` → toast hoặc log. |
| L12 | `AudioRecorder.swift:85-101`, `:75-82` | Đăng xuất trong lúc hộp thoại xin quyền micro đang mở hoặc đang chờ retry 0,5 giây: `stop()` không làm gì (`state == .idle`), sau đó việc ghi vẫn bắt đầu cho một user đã đăng xuất. Các đoạn bị `guard let userKey` bỏ qua (`RecorderPlugin.swift:209`), chỉ được reconcile nhận lại khi user cũ đăng nhập. | Thêm state `.starting` và cho `stop()` huỷ khi đang ở đó. |
| L13 | `GoogleAuthPlugin.swift:184`, `UploadQueue.swift:34` | Còn `!` force-unwrap, trái quy ước "không `!`" của plan. Hiện an toàn vì là hằng số và buffer khác rỗng. | Dùng `guard let`. |

---

## Những phần kiểm tra thấy đúng

- **SegmentStore là chủ duy nhất của `session.json`**: mọi đọc-sửa-ghi đều đi qua actor; ghi `.atomic` + `completeFileProtectionUntilFirstUserAuthentication`; có `schemaVersion` và decode khoan dung (thiếu khoá vẫn đọc được). Đúng plan.
- **Reconcile**: bỏ qua phiên đang ghi; thêm lại file có trên đĩa mà thiếu trong manifest; xoá và báo đoạn không đọc được; xoá phiên đã tải hết. Phiên tạo xong nhưng chưa có đoạn nào cũng được dọn đúng. Đoạn chưa kịp `appendSegment` khi app bị kill vẫn được cứu lại.
- **Khớp với backend**: `written` là tổng độ dài `.part` (`recordings.rs:108`), khớp với guard `written > offset`. `GET /api/files` trả `items[].id/name` (`files.rs:29-44`), khớp `findRecording`. `isRedo` khớp đúng các message `offset không liền mạch`, `session ghi không tồn tại`, `session id không hợp lệ`, `chưa có dữ liệu ghi`. 403 `session không thuộc bạn` được xử lý trước. 413 ở cả init và chunk → tạm dừng, lần sau `init` lại (đúng vì chunk đã xoá `.part`). 429 rơi vào nhóm transient. Chunk 1 MB nằm dưới giới hạn body 8 MB của BE và 500 MB của nginx.
- **Tên tất định `<sessionId>#<index>`** dùng UUID nên không trùng giữa các phiên.
- **4 bài học spike + kiểm tra ftyp `M4A `** đều có mặt và có comment.
- **Bản sửa chưa commit** (`AudioRecorder.swift:272-279`, chuyển `segmentClosed` sang main) đúng và cần thiết: trước đó `RecorderPlugin.handle` chạy trên luồng tap. Xem thêm L1 cho phần còn thiếu.
- **GoogleAuthPlugin**: PKCE S256, kiểm tra `state`, continuation chỉ resume một lần, xử lý huỷ đúng.
- **NativeFilesPlugin**: chỉ chấp nhận `https` + đúng host + `/api/`, nên cookie không ra ngoài. Thư mục share được dọn sau khi dùng.
- **`NativeRecordCard` luôn được mount** (`app-shell.tsx`: `forceMount`), nên `syncUploads` chạy ngay sau khi đăng nhập hoặc mở app, bất kể đang ở tab nào.

## Việc nên làm (theo thứ tự ưu tiên)

1. H1: lưu dấu finalize; tra theo tên **trước** khi `init` lại, và lỗi tra cứu thì không được init.
2. H2: nghe `AVAudioEngineConfigurationChange` và thêm watchdog engine trong `tickElapsed`.
3. M1: tách `kick(ifUser:)` khỏi `start(userKey:)`; huỷ Task drain khi `stop()`.
4. M2: thêm cờ `rerun` trong `drain`.
5. M3 + M4: chỉ phân loại terminal khi có JSON lỗi của BE; lưu `serverSession` để thử lại mà không tạo `.part` mới; có đường xoá `failure`.
6. M5, M6, M7: báo lỗi ghi đĩa; không restart engine trong lúc đang gián đoạn; dùng background task và đánh thức hàng đợi khi app lên foreground.
7. Các mục Low khi tiện tay sửa chung.

## Chỉ số

- TS: `tsc --noEmit` exit 0; biome 15 file, 0 lỗi.
- Swift: chưa build, chưa có test tự động. Self-test `-OSSelfTest` chỉ phủ reconcile và nhánh 401.
- Plan Phase 2: các tiêu chí "giết mạng sau finalize", "đúng một `.part`" và "AirPods" **sẽ không qua** nếu chưa sửa H1/M4/H2.

## Câu hỏi còn mở

- Plan yêu cầu 401 → `refreshSession()` rồi thử lại một lần. Bản native thì dừng hàng đợi và bắt đăng nhập lại Google (TTL cookie 12 giờ). Đây là quyết định có chủ đích hay còn nợ? Với buổi ghi qua đêm, các đoạn sẽ chờ tới lần đăng nhập lại.
- Cloudflare có bật Bot Fight hay WAF challenge cho `/api/recordings/*` không? Nếu có thì M3 xảy ra thường xuyên, không còn là trường hợp biên.
