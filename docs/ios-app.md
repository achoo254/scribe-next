# Scribe iOS app

App iPhone bọc giao diện web React (`web/`) bằng Capacitor 8, cộng phần native Swift để **ghi âm khi khoá màn hình** và tải từng đoạn lên backend prod `https://scribe.hoandat.com` (origin `VITE_NATIVE_API_ORIGIN` trong `web/.env`, nhúng vào Info.plist lúc build). Plan: `plans/260728-1609-scribe-ios-app-ghi-nen-khoa-man-hinh/`.

## Cài lên iPhone

Cắm iPhone vào Mac, mở khoá, rồi:

```bash
cd web
pnpm run ios:device
```

Script `web/scripts/ios-device.sh` build web → `cap sync ios` → `xcodebuild` (Release, Personal Team `AM5C55N339`, tự cấp provisioning) → cài → mở app. Chạy lại đúng lệnh này khi profile miễn phí hết hạn (7 ngày): app lại mở được, dữ liệu trên máy giữ nguyên.

Cần trên máy build:
- Xcode ở `/Applications/Xcode.app` (script tự đặt `DEVELOPER_DIR`, không cần `sudo xcode-select`), Apple ID của team đã đăng nhập trong Xcode.
- `web/.env` có `VITE_NATIVE_API_ORIGIN` (chép từ `web/.env.example`).
- Lần đầu cài: trên iPhone vào *Cài đặt → Cài đặt chung → Quản lý VPN & thiết bị* để tin cậy nhà phát triển.
- Sau mỗi lần Xcode cập nhật: nếu `xcodebuild -checkFirstLaunchStatus` trả khác 0 (17/10/2026, Xcode 27.1 trả 69), chạy `sudo xcodebuild -runFirstLaunch` một lần; nếu không, `devicectl`/`simctl` treo chờ bước này.

## Cách app hoạt động

| Phần | Ở đâu | Ghi chú |
|---|---|---|
| Giao diện | `web/dist` đóng gói trong app (`capacitor://localhost`) | Cùng mã với web; nhánh native qua `isNativeShell()` (`web/src/lib/api.ts`) |
| Điều hướng | `MainViewController.swift` | Mỗi trang có URL; app bật cử chỉ vuốt từ mép trái để lùi/tiến trang (`allowsBackForwardNavigationGestures`, Capacitor để tắt theo mặc định). Link IdP trong account menu và footer mở ở trình duyệt hệ thống |
| Gọi API | `CapacitorHttp` → `URLSession` | Không bị CORS/`SameSite`; cookie phiên nằm ở `HTTPCookieStorage`. Thân nhị phân chỉ đi đúng khi là `File` (Capacitor gửi base64); `Blob`/`ArrayBuffer` bị từ chối, `Uint8Array` bị hỏng — `apiFetch` tự bọc `Blob` (chunk của "Tải tệp lên", "Cắt file") thành `File`. `keepalive` bị bỏ qua |
| Chọn tệp | `<input type=file>` của WKWebView | Ảnh / Tệp / Quay video; quay video cần `NSCameraUsageDescription` (có từ 17/10/2026) |
| Vòng đời trang | WKWebView | Không có `pagehide`; JS không chạy khi app ở nền, `pause`/`visibilitychange` chỉ tới lúc mở lại. Xoá nhanh có hoàn tác vì vậy lưu id đang chờ ở `localStorage` và xoá nốt ở lần mở sau nếu app bị tắt trong lúc chờ |
| Đăng nhập hoandat-auth | `HoandatAuthPlugin.swift` | Client public `scribe-mobile`: lấy tham số ở `GET /api/auth/native/config`, app tự sinh PKCE + `nonce`, mở `auth.hoandat.com` bằng `ASWebAuthenticationSession`, redirect `com.hoandat.scribe:/auth/callback` → gửi `code` + verifier + `nonce` tới `POST /api/auth/native/exchange`. Backend giữ refresh token IdP (mã hoá), trả cookie phiên + token thiết bị |
| Tải / lưu / phát file | `NativeFilesPlugin.swift` | Tải kèm cookie; share sheet (Lưu vào Tệp); bản cache để phát và tua |
| Ghi âm | `Recorder/AudioRecorder.swift` | AVAudioEngine → m4a AAC, đoạn 1/5/10 phút, chạy nền (`UIBackgroundModes: audio`) |
| Lưu bền | `Recorder/SegmentStore.swift` | `Documents/recordings/<userKey>/<sessionId>/session.json` + `seg-NNN.m4a`; đối soát khi mở app, báo đoạn cụt nếu app bị kill |
| Đánh dấu từ màn hình khoá / AirPods | `Recorder/NowPlayingBookmarks.swift` | Xem mục "Đánh dấu khi khoá màn hình" bên dưới |
| Tải lên | `Recorder/UploadQueue.swift` | Mỗi đoạn một bản ghi tên `<sessionId>#<index>`; Lịch sử ghi gom thành một dòng "Bản ghi trên iPhone". Mất phản hồi finalize thì tra theo tên trước khi tạo lại (không sinh bản trùng) |
| Làm mới phiên | `SessionAuth.swift` | Token thiết bị của Scribe giữ trong Keychain của app (`AfterFirstUnlockThisDeviceOnly`). Phiên mobile 12 giờ; gặp 401 khi đang tải, hàng đợi đổi token thiết bị lấy cookie mới ở `POST /api/auth/native/refresh` (chỉ được khi hoandat-auth còn công nhận máy; thu hồi máy ở `/account` của IdP thì lần làm mới sau thất bại), rồi mới báo "cần đăng nhập lại". Đăng xuất thì xoá token và thiết bị trên server |
| Chép lời | (không có phần native) | Dùng key OpenAI của server, tính vào phút của tài khoản. `SecretsPlugin.swift` chỉ còn để xoá key người dùng tự nhập ở bản cũ khi đăng xuất |
| Chép trực tiếp | `Recorder/RealtimeTranscriber.swift`, `Recorder/TranscriptStore.swift` | Xem mục "Chép trực tiếp" bên dưới |

> **Đổi bundle id và đăng nhập (2026-10-16):** app id là `com.hoandat.scribe`, đăng nhập qua hoandat-auth; Firebase đã bỏ hẳn. Bản cài mới là một app khác trên máy: dữ liệu cục bộ của app cũ (đoạn chưa tải, Keychain) không chuyển sang. Refresh token Firebase của bản cũ (nếu có) bị xoá ở lần đăng nhập hoặc đăng xuất kế tiếp.

## Đánh dấu khi khoá màn hình

Khi khoá màn hình, JS của WKWebView bị treo nên nút "Đánh dấu" trên trang không bấm được. Vì vậy trong lúc ghi, Swift tự đăng ký mục **Now Playing** (màn hình khoá, Control Center):

- Tiêu đề "Scribe đang ghi", thời gian trôi theo đồng hồ ghi. Mọi lệnh phát/tạm dừng/bài kế/bài trước — nút trên màn hình khoá, AirPods bấm 1, 2 hay 3 lần — đều **thêm một dấu mốc**; từ đó không dừng hay tạm dừng được bản ghi. Tua, nhảy 15 giây, dừng bị tắt. Hai lần bấm cách nhau dưới 1 giây chỉ tính một.
- Bấm xong, tiêu đề đổi thành "Đã đánh dấu 12:34" khoảng 3 giây. Không phát âm báo (âm sẽ lọt vào bản ghi).
- Mốc lưu ngay xuống `Documents/bookmarks/<userKey>.json` (id 24 hex, `sessionId`, `positionS` lấy từ `recordedSeconds` như nút trên trang). Trang kéo về qua `takePendingBookmarks()` khi mở lại app (`visibilitychange`), khi Swift báo `bookmarkAdded` lúc đang mở, khi dừng ghi và khi khởi động, rồi gửi qua cùng hàng đợi `BookmarkQueue` tới `s:<sessionId>` (`web/src/features/recorder/native-bookmarks.ts`). Swift chỉ xoá mốc khi trang báo `ackBookmarks` — sau khi server đã lưu, từ chối hẳn, hoặc bản ghi không còn — nên tắt app giữa chừng không mất mốc; gửi lại cùng id thì server chỉ lưu một lần.
- iOS chỉ giao mục Now Playing và lệnh AirPods cho app đang phát âm thanh, nên trong lúc ghi `AudioRecorder` phát lặp một giây im lặng vào bộ trộn của engine (`playsSilenceWhileRecording`, đặt `false` để tắt nếu gây lỗi trên máy). Âm này không vào nhánh thu.
- Dừng ghi thì gỡ lệnh, dừng phát im lặng và xoá mục Now Playing. Cấu hình phiên âm thanh (`.playAndRecord`, ưu tiên micro trong máy, cho phép HFP/A2DP) giữ nguyên.

**Chưa kiểm trên máy thật** (Simulator không có micro, không có AirPods): mục Now Playing có hiện khi app thu âm kèm phát im lặng hay không (không hiện thì đường lùi là Live Activity, cần tài khoản Apple trả phí); phát im lặng có làm đổi đường âm AirPods hay tốn pin đáng kể không; AirPods bấm 1/2 lần có tới app không, nhất là khi AirPods đang là micro (HFP); lệnh có tới khi đang có cuộc gọi. Danh sách bấm thử ở mục "Lần cắm iPhone tới" của plan.

## Chép trực tiếp

Công tắc **"Chép trực tiếp"** trên thẻ ghi (mặc định tắt, nhớ lựa chọn trên từng máy ở `localStorage` `scribe.native.realtime`). Bật thì trong lúc ghi, chữ của từng câu hiện ở panel "Đang chép" khoảng 1 giây sau khi người nói ngừng, kèm mốc theo đồng hồ phiên ghi. Đây chỉ là bản xem trước: sau buổi ghi, chép lời cả file chạy như cũ; chữ trực tiếp không lưu thành bản chép lời.

- **Đường đi.** `AudioRecorder` đưa mỗi buffer đã ghi vào file kèm vị trí phiên (`setPCMSink`) cho `RealtimeTranscriber`: đổi sang PCM16 24 kHz mono, cắt khung 100 ms, gửi `input_audio_buffer.append` qua `URLSessionWebSocketTask` tới `wss://api.openai.com/v1/realtime`. Cấu hình phiên (`gpt-4o-mini-transcribe`, `vi`, `server_vad` 500 ms) gắn vào token lúc backend cấp, app không gửi `session.update`. Mốc của câu = vị trí phiên của khung đầu tiên trên socket + `audio_start_ms`.
- **Token.** Swift xin `POST /api/realtime/token` bằng cookie phiên native (401 thì làm mới bằng token thiết bị như hàng đợi tải lên), vì JS bị treo khi khoá máy. Mỗi token được stream tối đa `reservedSeconds` (backend giữ chỗ trên phút chép lời); còn 20 giây thì xin token mới, mở socket mới, chuyển audio sang, socket cũ gửi `commit` rồi đóng sau 4 giây để câu cuối vẫn về. Token cũ báo số giây đã stream qua `POST /api/realtime/usage` (thử 3 lần; không báo được thì backend tính trọn phần giữ chỗ).
- **Mất mạng.** Socket đứt thì nối lại với backoff 1–60 giây (token còn hạn thì dùng lại). Trong lúc nối, giữ tối đa 30 giây audio gần nhất để gửi bù; hụt hơn 1 giây thì ghi một dòng "mất đoạn này, chờ bản hoàn chỉnh". Trang hiện "Mất kết nối từ mm:ss".
- **Lỗi không dừng ghi.** Tài khoản ngoài `REALTIME_EMAILS` (403), hết phút (403 cùng câu như chép lời), chép lời tạm ngưng (503): panel hiện nguyên câu của máy chủ, bản ghi m4a chạy tiếp. Hết phút giữa chừng thì token đang dùng chạy hết phần đã giữ rồi dừng.
- **Lưu và đọc lại.** Mỗi câu ghi một dòng vào `transcript.jsonl` cạnh `session.json` của phiên (actor `TranscriptStore`, dòng bị cắt dở khi app bị tắt thì bỏ qua). Plugin phát sự kiện `transcript` (`type` sentence | partial | status) khi trang đang hiện; mở lại app, trang thay toàn bộ state bằng `getTranscript()` (`web/src/features/recorder/live-transcript.ts`).

**Chưa đo trên máy thật:** WebSocket có sống khi khoá máy lâu không (Gate 0 bước 2 của Phase 5), số câu nhận được so với số câu `server_vad` chốt, pin khi ghi kèm stream. Danh sách ở mục "Lần cắm iPhone tới" của plan.

## Đã kiểm / còn phải kiểm trên máy thật

| Hạng mục | Trạng thái |
|---|---|
| Gate 0 — `CapacitorHttp` tới BE prod | Đạt trên iPhone 15 Pro Max (2026-10-15): JS nhận 401 của `/api/me`, nginx ghi nhận request |
| Self-test Simulator (`-OSSelfTest`, chỉ bản Debug) | Đạt: đối soát phát hiện đoạn cụt, tên tất định, hàng đợi dừng ở "cần đăng nhập" khi 401 |
| m4a qua BE | `ftyp` = `M4A `; `infer 0.16` (cùng bản BE dùng) nhận `audio/m4a`; BE trả `audio/mp4` khi phát |
| Gate 1 — đăng nhập trên iPhone | Firebase: đạt (2026-10-16 sáng, bản cũ). hoandat-auth trên prod: đạt (2026-10-16 chiều) — `native/exchange` 200, một thiết bị + một phiên mobile, ghi, tải lên, chép lời bằng key server, phát lại |
| Ghi khi khoá màn hình ~3 phút, đoạn 1 phút, tải lên | Đạt (2026-10-16): 3 đoạn tới server ngay khi xong (08:24:24, 08:25:24, 08:26:20), AAC 48 kHz mono, 60,05 + 60,05 + 56,06 s liền mạch; phát lại 200; `session.json` đánh dấu đã tải, không còn `.m4a` trên máy |
| Cuộc gọi đến, AirPods, chế độ máy bay khi đang tải | **Chưa** — cần máy thứ hai để gọi vào và AirPods |
| Kill app khi đang ghi | Đạt (2026-10-16): vuốt tắt sau ~30 s; mở lại thì đoạn dở 31,3 s được đối soát và tải lên (server 08:29:41, ffprobe 31,36 s) |
| Đăng xuất khi còn đoạn chưa tải | Đạt (2026-10-16): đoạn 90,8 s ở lại trên máy, `uploaded: false`; server có `recordings/init` 3 s trước `logout` nhưng không có `finalize`, không file nào vào tài khoản. Sau đăng xuất máy có mạng, 08:31:51–08:35:02 (gồm lúc mở app để đăng nhập lại) server không nhận request nào. Đăng nhập lại cùng tài khoản (08:35:02) thì đoạn tồn tải nốt ngay: `finalize` 08:35:03 nối tiếp phiên tải cũ (không `init` mới), ffprobe 90,86 s, đúng một file |
| Ghi khi đổi trang, vuốt lùi và khoá màn hình (giao diện có URL) | Đạt (2026-10-16): đoạn 1 phút, một phiên, 3 đoạn 60 + 60 + 48 s, `init`/`finalize` 200, không lỗi, không phiên treo |
| QA cuối giao diện mới (đăng xuất từ menu, theme sáng/tối, safe area) | **Chưa** — hoãn 16/10/2026 theo yêu cầu (không cắm iPhone); web đã kiểm bằng Playwright e2e |
| Tính năng web 16–17/10 trong vỏ iOS (Simulator iOS 27.0, backend giả lập) | 17/10/2026: tải tệp lên (sau sửa, 6 MB khớp SHA-256), xoá có hoàn tác kể cả khi tắt app, xoá tất cả, thẻ gói, tìm kiếm, hỏi đáp, ghi native + dấu mốc `positionS` đạt. **Chưa** trên máy thật: picker Ảnh/Tệp/Quay video, "Mở chat" sang Safari — danh sách bấm thử ở mục "Lần cắm iPhone tới" của plan |
| Ghi dài không sạc, đo pin (ngưỡng ≤ 12% / 60 phút) | Đạt (2026-10-16): 48:07 khoá màn hình, đã rút sạc, đoạn 1 phút; pin 76% → 76% (dưới ~2%); server nhận đủ 49 đoạn liền mạch (48 × ~60 s + 3,9 s), tải lên từng phút 08:43:38–09:30:42 |
