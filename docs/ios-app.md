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
- `web/.env` trong thư mục env trên iCloud (`scripts/env-dir.sh`) có `VITE_NATIVE_API_ORIGIN` (mẫu `web/.env.example`).
- Lần đầu cài: trên iPhone vào *Cài đặt → Cài đặt chung → Quản lý VPN & thiết bị* để tin cậy nhà phát triển.
- Sau mỗi lần Xcode cập nhật: nếu `xcodebuild -checkFirstLaunchStatus` trả khác 0 (17/10/2026, Xcode 27.1 trả 69), chạy `sudo xcodebuild -runFirstLaunch` một lần; nếu không, `devicectl`/`simctl` treo chờ bước này.

## Phát hành TestFlight / App Store

Chuẩn bị sẵn trong repo (19/10/2026, plan `plans/dattqh/261019-1105-app-store-readiness/`):

- `web/ios/App/App/PrivacyInfo.xcprivacy` — privacy manifest của app: dữ liệu thu thập (audio, ảnh/video tự chọn, nội dung người dùng, email, tên, mã tài khoản, nhật ký hành động), tất cả liên kết tài khoản, không tracking, mục đích App Functionality; API cần lý do: chỉ mốc thời gian tệp (`C617.1`, `SegmentStore.swift` đọc ngày tạo thư mục bản ghi trong container của app). Capacitor có manifest riêng cho API nó dùng (UserDefaults…).
- `Info.plist`: `ITSAppUsesNonExemptEncryption = false` (chỉ HTTPS chuẩn), mô tả quyền tiếng Việt cho micro, camera, lưu vào Ảnh (`NSPhotoLibraryAddUsageDescription`, bảng chia sẻ "Lưu video"); bản tiếng Trung giản thể ở `zh-Hans.lproj/InfoPlist.strings`, chữ màn hình khoá lúc ghi (`NowPlayingControls.swift`) dịch ở `zh-Hans.lproj/Localizable.strings` — máy đặt ngôn ngữ khác thấy tiếng Việt; `UIRequiredDeviceCapabilities` = `arm64`. Không dùng mạng cục bộ.
- `appId` (`com.hoandat.scribe`) và `appName` của `web/capacitor.config.ts` lấy từ `web/brand.config.json` qua `web/brand/generated/capacitor.json` (`pnpm run brand:gen`); test `web/scripts/brand.test.ts` khoá cấu hình Capacitor hiệu lực. Bundle id trong `project.pbxproj`, `CFBundleDisplayName`, tên service Keychain và AppIcon không sinh từ brand: đổi bundle id là app khác trên App Store. AppIcon (`AppIcon-512@2x.png`, 1024 px, không alpha) thay tay từ ảnh gốc 1024 px khi đổi icon (26/10/2026: icon vẽ bằng AI, cùng hình `web/brand/icon.svg`). Ảnh màn hình khởi động (`Splash.imageset`, 3 file giống nhau 2732 px: icon giữa nền `colors.background`) thì `pnpm run brand:gen` sinh từ `web/brand/icon.svg` (`web/scripts/brand-ios-splash.ts`).
- Chỉ iPhone (`TARGETED_DEVICE_FAMILY = 1`): không cần ảnh iPad khi nộp.
- Phiên bản: `MARKETING_VERSION` = `1.0` trong `project.pbxproj`; số build (`CFBundleVersion`) do script đặt lúc archive = giờ UTC `YYYYMMDD.HHMM` (luôn tăng), hoặc `IOS_BUILD_NUMBER`.
- Trang công khai `/privacy`, `/terms`; "Xoá tài khoản" trong menu tài khoản; hồ sơ App Store và ảnh chụp: [app-store/metadata.md](./app-store/metadata.md).

### Việc của chủ dự án khi Apple duyệt tài khoản

1. **Team ID** `AM5C55N339` (developer.apple.com → Account → Membership details): gói trả phí giữ nguyên Team ID của Personal Team, nên `ios-device.sh` và `ios-release.sh` đều mặc định dùng nó, không cần đặt `IOS_TEAM`. Profile phát triển đang có (hạn 7 ngày) vẫn dùng tới khi hết hạn; lần cấp lại sau khi gói kích hoạt có hạn 1 năm.
2. **Tạo app** ở App Store Connect → Apps → "+" → New App: Platform iOS, Name `Scribe – Ghi âm & chép lời`, Primary language Vietnamese, Bundle ID `com.hoandat.scribe` (đăng ký trước ở developer.apple.com → Identifiers nếu danh sách chưa có; Xcode tự đăng ký ở lần archive đầu khi dùng ký tự động), SKU `scribe-ios`, User Access Full Access.
3. **Tạo key API**: App Store Connect → Users and Access → Integrations → App Store Connect API → Team Keys → "+". Role: **Admin** — ký phân phối tự động bằng key (cloud signing) cần quyền này theo báo cáo trên Apple Developer Forums; key **App Manager** tải lên được nhưng có thể lỗi "Cloud signing permission error" ở bước ký. Tải tệp `AuthKey_<KEY_ID>.p8` (chỉ tải được một lần), ghi Key ID và Issuer ID.
4. **Cất key**: tệp `.p8` để ngoài repo, quyền 600 (ví dụ `~/.appstoreconnect/private_keys/AuthKey_<KEY_ID>.p8`, thư mục mặc định Xcode cũng tìm); Key ID, Issuer ID và bản sao tệp `.p8` lưu vào tệp access-tokens trên iCloud với service `appstoreconnect.apple.com`. Không commit, không gửi qua chat.
5. **Chạy script** (từ `web/`, `web/.env` trên iCloud có `VITE_NATIVE_API_ORIGIN=https://scribe.hoandat.com` và nên có `VITE_SUPPORT_EMAIL`):

   ```bash
   cd web
   ASC_KEY_ID=<Key ID> ASC_ISSUER_ID=<Issuer ID>    ASC_KEY_PATH=~/.appstoreconnect/private_keys/AuthKey_<KEY_ID>.p8    pnpm run ios:release
   ```

   `web/scripts/ios-release.sh`: build web → `cap sync ios` → `xcodebuild archive` (Release, ký tự động, `-allowProvisioningUpdates` + key API) → `xcodebuild -exportArchive` với `method: app-store-connect`, `destination: upload` (ký phân phối và tải thẳng lên App Store Connect, không dùng `altool`) → in phiên bản và số build. Tuỳ chọn `IOS_BUILD_NUMBER`, `IOS_RELEASE_DIR` (giữ lại archive).
6. **TestFlight**: build hiện trong tab TestFlight sau khi Apple xử lý (vài phút tới vài chục phút). Lần đầu trả lời câu hỏi mã hoá đã được `ITSAppUsesNonExemptEncryption` trả lời sẵn. Tạo nhóm Internal Testing, thêm người thử (tài khoản trong Users and Access, tối đa 100), bật build cho nhóm.
7. **Nộp duyệt** (khi muốn lên App Store): điền hồ sơ từ [app-store/metadata.md](./app-store/metadata.md) (mô tả, từ khoá, ảnh `docs/app-store/screenshots/`, App Privacy, Age Rating, ghi chú cho người duyệt, tài khoản demo), chọn build, Submit for Review.

Backend có xoá tài khoản và trang `/privacy`, `/terms` đã chạy trên prod; trước khi nộp duyệt còn cần tài khoản demo cho người duyệt và `VITE_SUPPORT_EMAIL`.

## Cách app hoạt động

| Phần | Ở đâu | Ghi chú |
|---|---|---|
| Giao diện | `web/dist` đóng gói trong app (`capacitor://localhost`) | Cùng mã với web; nhánh native qua `isNativeShell()` (`web/src/lib/api.ts`) |
| Điều hướng | `MainViewController.swift` | Mỗi trang có URL; app bật cử chỉ vuốt từ mép trái để lùi/tiến trang (`allowsBackForwardNavigationGestures`, Capacitor để tắt theo mặc định). Link IdP trong account menu và footer mở ở trình duyệt hệ thống |
| Gọi API | `CapacitorHttp` → `URLSession` | Không bị CORS/`SameSite`; cookie phiên nằm ở `HTTPCookieStorage`. Thân nhị phân chỉ đi đúng khi là `File` (Capacitor gửi base64); `Blob`/`ArrayBuffer` bị từ chối, `Uint8Array` bị hỏng — `apiFetch` tự bọc `Blob` (chunk của "Tải tệp lên", "Cắt file") thành `File`. `keepalive` bị bỏ qua |
| Chọn tệp | `<input type=file>` của WKWebView | Ảnh / Tệp / Quay video; quay video cần `NSCameraUsageDescription` (có từ 17/10/2026) |
| Vòng đời trang | WKWebView | Không có `pagehide`; JS không chạy khi app ở nền, `pause`/`visibilitychange` chỉ tới lúc mở lại. Xoá nhanh có hoàn tác vì vậy lưu id đang chờ ở `localStorage` và xoá nốt ở lần mở sau nếu app bị tắt trong lúc chờ |
| Đăng nhập auth-next | `HoandatAuthPlugin.swift` | Client public `scribe-mobile`: lấy tham số ở `GET /api/auth/native/config`, app tự sinh PKCE + `nonce`, mở `auth.hoandat.com` bằng `ASWebAuthenticationSession`, redirect `com.hoandat.scribe:/auth/callback` → gửi `code` + verifier + `nonce` tới `POST /api/auth/native/exchange`. Backend giữ refresh token IdP (mã hoá), trả cookie phiên + token thiết bị |
| Tải / lưu / phát file | `NativeFilesPlugin.swift` | Tải kèm cookie; share sheet (Lưu vào Tệp); bản cache để phát và tua |
| Ghi âm | `Recorder/AudioRecorder.swift` | AVAudioEngine → m4a AAC, đoạn 1/5/10 phút, chạy nền (`UIBackgroundModes: audio`) |
| Lưu bền | `Recorder/SegmentStore.swift` | `Documents/recordings/<userKey>/<sessionId>/session.json` + `seg-NNN.m4a`; đối soát khi mở app, báo đoạn cụt nếu app bị kill |
| Tạm dừng / đánh dấu từ màn hình khoá, AirPods | `Recorder/NowPlayingControls.swift` | Xem mục "Tạm dừng và đánh dấu khi khoá màn hình" bên dưới |
| Tải lên | `Recorder/UploadQueue.swift` | Mỗi đoạn một bản ghi tên `<sessionId>#<index>`; Lịch sử ghi gom thành một dòng "Bản ghi trên iPhone". Mất phản hồi finalize thì tra theo tên trước khi tạo lại (không sinh bản trùng) |
| Xoá tài khoản | `RecorderPlugin.signOut({discardUserKey})`, `SessionAuth.swift` | Menu tài khoản → "Xoá tài khoản" gọi `POST /api/me/delete`; xong thì app dừng ghi, xoá cache audio và token thiết bị như đăng xuất, và xoá đoạn chưa tải, chữ chép trực tiếp, dấu mốc của tài khoản đó trên máy (`forgetDeletedAccount` ở `auth-context.tsx`); không kết thúc phiên ở auth-next. Tài khoản bị xoá từ máy khác: lần làm mới phiên sau nhận 410 `ACCOUNT_DELETED`, hàng đợi xoá đoạn của tài khoản đó thay vì chờ đăng nhập lại (thu hồi máy thường vẫn giữ đoạn để tải nốt khi đăng nhập lại) |
| Làm mới phiên | `SessionAuth.swift` | Token thiết bị của Scribe giữ trong Keychain của app (`AfterFirstUnlockThisDeviceOnly`). Phiên mobile 12 giờ; gặp 401 khi đang tải, hàng đợi đổi token thiết bị lấy cookie mới ở `POST /api/auth/native/refresh` (chỉ được khi auth-next còn công nhận máy; thu hồi máy ở `/account` của IdP thì lần làm mới sau thất bại), rồi mới báo "cần đăng nhập lại". Đăng xuất thì xoá token và thiết bị trên server |
| Chép lời | (không có phần native) | Dùng key OpenAI của server, tính vào phút của tài khoản. `SecretsPlugin.swift` chỉ còn để xoá key người dùng tự nhập ở bản cũ khi đăng xuất |
| Chép trực tiếp | `Recorder/RealtimeTranscriber.swift`, `Recorder/TranscriptStore.swift` | Xem mục "Chép trực tiếp" bên dưới |

> **Đổi bundle id và đăng nhập (2026-10-16):** app id là `com.hoandat.scribe`, đăng nhập qua auth-next; Firebase đã bỏ hẳn. Bản cài mới là một app khác trên máy: dữ liệu cục bộ của app cũ (đoạn chưa tải, Keychain) không chuyển sang. Refresh token Firebase của bản cũ (nếu có) bị xoá ở lần đăng nhập hoặc đăng xuất kế tiếp.

## Tạm dừng và đánh dấu khi khoá màn hình

Khi khoá màn hình, JS của WKWebView bị treo nên các nút trên trang không bấm được. Vì vậy trong lúc ghi, Swift tự đăng ký mục **Now Playing** (màn hình khoá, Control Center):

- **Nút giữa (⏸/▶) và AirPods bấm 1 lần: tạm dừng hoặc ghi tiếp** (`AudioRecorder.pause()`/`resume()`). Cả ba lệnh phát, tạm dừng, phát/tạm dừng đều đảo trạng thái, vì iOS tự chọn lệnh theo âm im lặng vẫn đang phát. Hai lần bấm cách nhau dưới 0,4 giây chỉ tính một. Khi tạm dừng: đoạn đang mở đóng lại và tải lên; không ghi gì cho tới lúc ghi tiếp (ghi tiếp mở đoạn mới); đồng hồ và `positionS` đứng yên nên dấu mốc và chép trực tiếp nối liền; tiêu đề "Scribe đã tạm dừng", nút đổi thành ▶ (`PlaybackRate` 0). Engine và âm im lặng vẫn chạy để app không bị iOS treo và mục Now Playing còn đó, nên chấm cam micro vẫn sáng khi tạm dừng. Trong app có nút "Tạm dừng"/"Ghi tiếp" cạnh "Dừng ghi"; nút chỉ hiện khi `getStatus()` có trường `paused` (bản app cũ và Android không có).
- **⏩/⏪ và AirPods bấm 2, 3 lần: thêm một dấu mốc.** Tua, nhảy 15 giây, dừng bị tắt. Hai lần bấm cách nhau dưới 1 giây chỉ tính một.
- Đánh dấu xong, tiêu đề đổi thành "Đã đánh dấu 12:34" khoảng 3 giây. Không phát âm báo (âm sẽ lọt vào bản ghi).
- Mốc lưu ngay xuống `Documents/bookmarks/<userKey>.json` (id 24 hex, `sessionId`, `positionS` lấy từ `recordedSeconds` như nút trên trang). Trang kéo về qua `takePendingBookmarks()` khi mở lại app (`visibilitychange`), khi Swift báo `bookmarkAdded` lúc đang mở, khi dừng ghi và khi khởi động, rồi gửi qua cùng hàng đợi `BookmarkQueue` tới `s:<sessionId>` (`web/src/features/recorder/native-bookmarks.ts`). Swift chỉ xoá mốc khi trang báo `ackBookmarks` — sau khi server đã lưu, từ chối hẳn, hoặc bản ghi không còn — nên tắt app giữa chừng không mất mốc; gửi lại cùng id thì server chỉ lưu một lần.
- iOS chỉ giao mục Now Playing và lệnh AirPods cho app đang phát âm thanh, nên trong lúc ghi `AudioRecorder` phát lặp một giây im lặng vào bộ trộn của engine (`playsSilenceWhileRecording`, đặt `false` để tắt nếu gây lỗi trên máy). Âm này không vào nhánh thu.
- Dừng ghi thì gỡ lệnh, dừng phát im lặng và xoá mục Now Playing. Cấu hình phiên âm thanh (`.playAndRecord`, ưu tiên micro trong máy, cho phép HFP/A2DP) giữ nguyên.

**Chưa kiểm trên máy thật** (Simulator không có micro, không có AirPods): mục Now Playing có hiện khi app thu âm kèm phát im lặng hay không (không hiện thì đường lùi là Live Activity, cần tài khoản Apple trả phí); phát im lặng có làm đổi đường âm AirPods hay tốn pin đáng kể không; AirPods bấm 1/2 lần có tới app không, nhất là khi AirPods đang là micro (HFP); lệnh có tới khi đang có cuộc gọi. Danh sách bấm thử ở mục "Lần cắm iPhone tới" của plan.

## Chép trực tiếp

Công tắc **"Chép trực tiếp"** trên thẻ ghi (mặc định tắt, nhớ lựa chọn trên từng máy ở `localStorage` `scribe.native.realtime`). Bật thì trong lúc ghi, chữ của từng câu hiện ở panel "Đang chép" khoảng 1 giây sau khi người nói ngừng, kèm mốc theo đồng hồ phiên ghi. Đây chỉ là bản xem trước: sau buổi ghi, chép lời cả file chạy như cũ; chữ trực tiếp không lưu thành bản chép lời.

- **Nhà cung cấp.** Backend chọn theo biến build `REALTIME_PROVIDER` và báo trong mỗi token (`provider`): **ElevenLabs Scribe v2 Realtime** mặc định từ 19/10/2026; `openai` (OpenAI Realtime, `gpt-4o-mini-transcribe`) giữ làm dự phòng một bản phát hành. Thiếu `provider` (backend cũ) thì app hiểu là OpenAI. App bản 17/10 không biết ElevenLabs: cài app mới trước hoặc cùng lúc deploy backend mới.
- **Đường đi.** `AudioRecorder` đưa mỗi buffer đã ghi vào file kèm vị trí phiên (`setPCMSink`) cho `RealtimeTranscriber`: đổi sang PCM16 24 kHz mono, cắt khung 100 ms, gửi qua `URLSessionWebSocketTask` tới `url` backend trả. ElevenLabs: `input_audio_chunk` (`sample_rate` 24000), cấu hình phiên nằm trong query của `url` (`scribe_v2_realtime`, `pcm_24000`, `vi`, `commit_strategy=vad`, im lặng 1,0 giây, `include_timestamps=true`), app chỉ thêm `token`; câu = `committed_transcript_with_timestamps`, mốc = vị trí phiên của khung đầu tiên trên socket + `start` của từ đầu (ElevenLabs tính mốc từ khung đầu của kết nối); chữ tạm = `partial_transcript`. OpenAI: `input_audio_buffer.append` tới `wss://api.openai.com/v1/realtime`, cấu hình gắn vào token, mốc = khung đầu + `audio_start_ms`.
- **Token.** Swift xin `POST /api/realtime/token` bằng cookie phiên native (401 thì làm mới bằng token thiết bị như hàng đợi tải lên), vì JS bị treo khi khoá máy. Mỗi token được stream tối đa `reservedSeconds` (backend giữ chỗ trên phút chép lời); còn 20 giây thì xin token mới, mở socket mới, chuyển audio sang, socket cũ gửi lệnh chốt rồi đóng sau 4 giây để câu cuối vẫn về. Token ElevenLabs dùng một lần (hết hạn 15 phút, một token mở đúng một socket). Token cũ báo số giây đã stream qua `POST /api/realtime/usage` (thử 3 lần; không báo được thì backend tính trọn phần giữ chỗ).
- **Mất mạng.** Socket đứt (hoặc ElevenLabs gửi lỗi như `auth_error`, `rate_limited`, `session_time_limit_exceeded`) thì nối lại với backoff 1–60 giây (OpenAI: token còn hạn thì dùng lại; ElevenLabs: luôn xin token mới). Trong lúc nối, giữ tối đa 30 giây audio gần nhất để gửi bù; hụt hơn 1 giây thì ghi một dòng "mất đoạn này, chờ bản hoàn chỉnh". Trang hiện "Mất kết nối từ mm:ss".
- **Lỗi không dừng ghi.** Tài khoản ngoài `REALTIME_EMAILS` (403), hết phút (403 cùng câu như chép lời), chép lời tạm ngưng (503): panel hiện nguyên câu của máy chủ, bản ghi m4a chạy tiếp. Hết phút giữa chừng thì token đang dùng chạy hết phần đã giữ rồi dừng. Sổ giờ ElevenLabs của tháng đầy (403 `PROVIDER_HOURS`) hoặc WebSocket báo `quota_exceeded` thì dừng chép trực tiếp; app báo `quotaExceeded` cho backend để chặn chép lời bằng ElevenLabs tới tháng sau.
- **Lưu và đọc lại.** Mỗi câu ghi một dòng vào `transcript.jsonl` cạnh `session.json` của phiên (actor `TranscriptStore`, dòng bị cắt dở khi app bị tắt thì bỏ qua). Plugin phát sự kiện `transcript` (`type` sentence | partial | status) khi trang đang hiện; mở lại app, trang thay toàn bộ state bằng `getTranscript()` (`web/src/features/recorder/live-transcript.ts`).

**Chưa đo trên máy thật:** WebSocket có sống khi khoá máy lâu không (Gate 0 bước 2 của Phase 5), số câu nhận được so với số câu VAD chốt, pin khi ghi kèm stream, và toàn bộ đường ElevenLabs trên iPhone (đã thử từ Mac 19/10: cấp token qua đường mã backend, stream 10 giây ra câu tiếng Việt). Danh sách ở mục "Lần cắm iPhone tới" của plan.

## Đã kiểm / còn phải kiểm trên máy thật

| Hạng mục | Trạng thái |
|---|---|
| Gate 0 — `CapacitorHttp` tới BE prod | Đạt trên iPhone 15 Pro Max (2026-10-15): JS nhận 401 của `/api/me`, nginx ghi nhận request |
| Self-test Simulator (`-OSSelfTest`, chỉ bản Debug) | Đạt: đối soát phát hiện đoạn cụt, tên tất định, hàng đợi dừng ở "cần đăng nhập" khi 401 |
| m4a qua BE | `ftyp` = `M4A `; `infer 0.16` (cùng bản BE dùng) nhận `audio/m4a`; BE trả `audio/mp4` khi phát |
| Gate 1 — đăng nhập trên iPhone | Firebase: đạt (2026-10-16 sáng, bản cũ). auth-next trên prod: đạt (2026-10-16 chiều) — `native/exchange` 200, một thiết bị + một phiên mobile, ghi, tải lên, chép lời bằng key server, phát lại |
| Ghi khi khoá màn hình ~3 phút, đoạn 1 phút, tải lên | Đạt (2026-10-16): 3 đoạn tới server ngay khi xong (08:24:24, 08:25:24, 08:26:20), AAC 48 kHz mono, 60,05 + 60,05 + 56,06 s liền mạch; phát lại 200; `session.json` đánh dấu đã tải, không còn `.m4a` trên máy |
| Cuộc gọi đến, AirPods, chế độ máy bay khi đang tải | **Chưa** — cần máy thứ hai để gọi vào và AirPods |
| Kill app khi đang ghi | Đạt (2026-10-16): vuốt tắt sau ~30 s; mở lại thì đoạn dở 31,3 s được đối soát và tải lên (server 08:29:41, ffprobe 31,36 s) |
| Đăng xuất khi còn đoạn chưa tải | Đạt (2026-10-16): đoạn 90,8 s ở lại trên máy, `uploaded: false`; server có `recordings/init` 3 s trước `logout` nhưng không có `finalize`, không file nào vào tài khoản. Sau đăng xuất máy có mạng, 08:31:51–08:35:02 (gồm lúc mở app để đăng nhập lại) server không nhận request nào. Đăng nhập lại cùng tài khoản (08:35:02) thì đoạn tồn tải nốt ngay: `finalize` 08:35:03 nối tiếp phiên tải cũ (không `init` mới), ffprobe 90,86 s, đúng một file |
| Ghi khi đổi trang, vuốt lùi và khoá màn hình (giao diện có URL) | Đạt (2026-10-16): đoạn 1 phút, một phiên, 3 đoạn 60 + 60 + 48 s, `init`/`finalize` 200, không lỗi, không phiên treo |
| QA cuối giao diện mới (đăng xuất từ menu, theme sáng/tối, safe area) | **Chưa** — hoãn 16/10/2026 theo yêu cầu (không cắm iPhone); web đã kiểm bằng Playwright e2e |
| Tính năng web 16–17/10 trong vỏ iOS (Simulator iOS 27.0, backend giả lập) | 17/10/2026: tải tệp lên (sau sửa, 6 MB khớp SHA-256), xoá có hoàn tác kể cả khi tắt app, xoá tất cả, thẻ gói, tìm kiếm, hỏi đáp, ghi native + dấu mốc `positionS` đạt. **Chưa** trên máy thật: picker Ảnh/Tệp/Quay video, "Mở chat" sang Safari — danh sách bấm thử ở mục "Lần cắm iPhone tới" của plan |
| Tạm dừng / ghi tiếp từ màn hình khoá | Đạt (2026-10-23, TestFlight build 20261023.253): bấm ⏸/▶ trên màn hình khoá tạm dừng và ghi tiếp bản ghi. **Chưa** kiểm riêng: AirPods bấm 1/2 lần, đánh dấu bằng ⏩ trên bản mới |
| Ghi dài không sạc, đo pin (ngưỡng ≤ 12% / 60 phút) | Đạt (2026-10-16): 48:07 khoá màn hình, đã rút sạc, đoạn 1 phút; pin 76% → 76% (dưới ~2%); server nhận đủ 49 đoạn liền mạch (48 × ~60 s + 3,9 s), tải lên từng phút 08:43:38–09:30:42 |
