# Scribe Android app

App Android (`web/android/`) là vỏ Capacitor 8 quanh cùng bản build `web/dist` với app iPhone. Các plugin viết bằng
Kotlin và giữ **đúng contract JS** của plugin Swift (`HoandatAuth`, `NativeFiles`, `Recorder`, `Secrets` —
`web/src/lib/native.ts`, `web/src/features/recorder/native-recorder.ts`), nên mã web không đổi. Plan:
`hoandat-app/plans/dattqh/261020-1610-android-capacitor-apps/` (phase 01–05, 08).

> **Trạng thái (23/10/2026):** mã đã viết đủ; job `android` của CI build xanh lần đầu ở commit `c32dceb`
> (lint, 48 test JVM, APK debug và APK androidTest). Máy dev vẫn không có JDK/Android SDK, nên CI là nơi biên dịch.
> **Chưa chạy trên máy Android thật**: mọi hạng mục trên máy ở bảng cuối trang đều đang chờ.

## Số liệu nền

| Mục | Giá trị |
|---|---|
| applicationId | `com.hoandat.scribe` (`appIds.android` trong `web/brand.config.json`, bằng `appIds.ios`; test brand khoá `app/build.gradle`) |
| targetSdk / compileSdk | **36 (Android 16)** — đã kiểm [developer.android.com](https://developer.android.com/google/play/requirements/target-sdk) ngày **23/10/2026**: từ 31/08/2026 app mới và bản cập nhật phải nhắm API 36 (gia hạn tới 01/11/2026 nếu xin). Nâng ở `web/android/variables.gradle` |
| minSdk | 24 (Android 7.0, mặc định của Capacitor 8) |
| Gradle / AGP / Kotlin / JDK | Gradle 8.14.3, AGP 8.13.0, Kotlin 2.2.20, JDK 21 (temurin trên CI) |
| Thư viện thêm | `androidx.browser` 1.10.0 (Auth Tab), `kotlinx-coroutines-android` 1.10.2, OkHttp 4.12.0 (WebSocket chép trực tiếp) |
| Origin backend | `BuildConfig.SCRIBE_BACKEND_ORIGIN`, lấy từ biến môi trường `SCRIBE_BACKEND_ORIGIN` hoặc `VITE_NATIVE_API_ORIGIN`; thiếu thì mọi build dừng (như iOS dừng khi thiếu `ScribeBackendOrigin`) |
| versionName / versionCode | `1.0` (như `MARKETING_VERSION` iOS) / `ANDROID_VERSION_CODE`, CI đặt giờ UTC `yyMMddHH`; build cục bộ là 1 |

## Build và cài thử

Cần JDK 21, Android SDK (`ANDROID_HOME` hoặc `sdk.dir` trong `web/android/local.properties`), `adb`, `web/.env`.

```bash
cd web
pnpm run android:device   # build web → cap sync android → gradlew installDebug → mở app trên máy adb thấy
pnpm run android:sync     # chỉ build web + cap sync (mở web/android bằng Android Studio)
```

Gradle nặng: trên máy 16 GB dùng chung, không chạy cùng lúc với build/test khác. Muốn thử đường lùi Custom Tabs
khi trình duyệt có Auth Tab: `adb shell run-as com.hoandat.scribe touch files/force-custom-tabs` (chỉ bản debug).
Test JVM: `cd web/android && ./gradlew testDebugUnitTest`; test `SecureStore` với AndroidKeyStore thật:
`./gradlew connectedDebugAndroidTest` (cần emulator/máy).

Đổi icon/màu: sửa `web/brand.config.json` hoặc `web/brand/icon*.svg` rồi `pnpm run brand:gen`. Lệnh này sinh thêm
icon launcher adaptive dạng vector (nền, tiền cảnh, monochrome cho themed icon Android 13+) từ `icon-maskable.svg`,
icon PNG cho Android 7.x, màu splash sáng/tối, và ảnh Play ở `web/brand/generated/play/` (icon 512 × 512,
feature graphic 1024 × 500 JPEG) — `web/scripts/brand-android.ts`. Bộ chuyển SVG chỉ hiểu `<rect>`, `<g fill>`,
gradient: vẽ lại icon bằng phần tử khác thì `brand:gen` báo lỗi thay vì sinh sai.

## CI

| Workflow | Khi nào | Ra gì |
|---|---|---|
| `ci.yml` job `android` | mọi push lên `main` và PR | `vite build` → `cap sync android` → `./gradlew lintDebug testDebugUnitTest assembleDebug assembleDebugAndroidTest` → kiểm manifest (`web/scripts/android-check-manifest.py`: `targetSdkVersion` bằng `variables.gradle`, không quyền ngoài danh sách) → artifact `scribe-android-debug-apk` (14 ngày). Lỗi thì artifact `scribe-android-reports` (báo cáo lint, test) |
| `android-release.yml` | `workflow_dispatch` (input `version_code` tuỳ chọn) hoặc tag `android-v*` | AAB ký bằng upload key → artifact `scribe-release-aab-<versionCode>` (30 ngày), tóm tắt ghi `versionCode` và SHA-256 của upload cert. Thiếu secret thì không build, chỉ cảnh báo |

Action ghim theo SHA (cùng SHA các repo khác trong hệ sinh thái dùng), `actions/setup-java` v6.0.1, temurin 21.

## Phát hành

Mô hình **Play App Signing**: Google tạo và giữ khoá ký app; ta ký AAB bằng **upload key**. Repo không chứa
keystore hay mật khẩu (`.gitignore` chặn `*.jks`, `*.keystore`); Gradle chỉ đọc chúng từ biến môi trường, và
`bundleRelease` thiếu biến thì dừng với thông báo rõ thay vì ra bản không ký.

Việc của chủ dự án (một lần):

1. Tạo upload key, cất ngoài repo (quyền 600), bản sao + mật khẩu vào tệp access-tokens trên iCloud:
   `keytool -genkeypair -v -keystore scribe-upload.jks -alias upload -keyalg RSA -keysize 4096 -validity 10000`.
2. GitHub `achoo254/scribe-next` → Settings → Environments → `android-release` (nên bật "Required reviewers"),
   thêm 4 secret: `ANDROID_UPLOAD_KEYSTORE_BASE64` (`base64 -i scribe-upload.jks | pbcopy`),
   `ANDROID_UPLOAD_KEYSTORE_PASSWORD`, `ANDROID_UPLOAD_KEY_ALIAS` (`upload`), `ANDROID_UPLOAD_KEY_PASSWORD`.
3. Chạy workflow "Android release", tải AAB về, tải lên Play Console (track internal) bằng tay. Lần tải đầu của app
   mới phải qua Play Console; tự động hoá bằng service account để sau bản production đầu tiên (Jev 20/10/2026).

Mỗi lần phát hành cần `versionCode` lớn hơn bản trước: hai lần chạy trong cùng một giờ UTC thì nhập `version_code`.

## Cách app hoạt động

| Phần | Ở đâu (`web/android/app/src/main/java/com/hoandat/scribe/`) | Ghi chú |
|---|---|---|
| Giao diện | `web/dist` trong app (`https://localhost`) | Cùng mã với web; `isNativeShell()` đúng như trên iOS |
| Điều hướng | `MainActivity.kt` | Nút/cử chỉ Back (có predictive back) lùi lịch sử trang; hết lịch sử thì đưa app về nền chứ không đóng, nên bản ghi và trang còn nguyên. Link ra host khác (IdP, "Mở chat") được Capacitor mở ở trình duyệt hệ thống. Edge-to-edge (bắt buộc từ target 35) do `SystemBars` của Capacitor 8: WebView đủ mới nhận `env(safe-area-inset-*)`, WebView cũ thì Capacitor đệm lề |
| Gọi API, cookie phiên (Gate 0 G0.2) | `net/BackendHttp.kt`, `net/WebViewCookies.kt` | Quyết định từ mã nguồn Capacitor 8.5.3 (23/10/2026): plugin lõi `CapacitorCookies` luôn được nạp và đặt `java.net.CookieHandler` mặc định trên kho cookie của WebView (`android.webkit.CookieManager`); `CapacitorHttp` và mã Kotlin đều đi bằng `HttpURLConnection` nên **dùng chung một kho cookie**. Khi bridge chưa nạp (dịch vụ ghi chạy không có Activity), `WebViewCookies.install()` đặt một handler tương đương. Cookie phiên của backend có `Max-Age` nên không bị `removeSessionCookies()` của Capacitor xoá. Còn phải xác nhận trên máy (bảng cuối) |
| Đăng nhập auth-next | `auth/HoandatAuthPlugin.kt`, `auth/AuthRequest.kt`, `auth/AuthRedirectActivity.kt` | Như iOS: `GET /api/auth/native/config`, app tự sinh PKCE S256 + `state` + `nonce` (chỉ trong bộ nhớ), mở `auth.hoandat.com` bằng **Auth Tab**; trình duyệt mặc định không hỗ trợ thì **Custom Tabs**, redirect `com.hoandat.scribe:/auth/callback` về qua `AuthRedirectActivity` (activity duy nhất nhận scheme này). Không chế độ ẩn danh, dùng lại phiên IdP. Đóng tab = `CANCELLED`. App bị Android giết trong lúc ở trình duyệt thì lần đăng nhập đó mất (verifier không ghi đĩa), người dùng bấm lại |
| Vì sao không gọi `@shared-next/capacitor-system-auth` | — | Plugin dùng chung chỉ làm phần mở trình duyệt; PKCE, `state`/`nonce` và đổi mã nằm ở JS của app. Dùng nó cho Android thì verifier và nonce phải nằm trong WebView, còn iOS giữ ở Swift — hai nền tảng lệch nhau, và contract `HoandatAuth.signIn()` phải tách đôi. Plugin Capacitor không gọi được method của plugin khác từ Kotlin, nên `HoandatAuthPlugin.kt` **chép** phần Auth Tab/Custom Tabs của `SystemAuthPlugin.kt` (v0.3.0, đã biên dịch ở CI của Localman) kèm ghi chú nguồn. Đổi ở plugin dùng chung thì đối chiếu lại tệp này |
| Token thiết bị | `auth/SessionAuth.kt`, `auth/SecureStore.kt` | AES-256-GCM, khoá trong **AndroidKeyStore** (alias `scribe.session`, không cần xác thực người dùng để làm mới phiên lúc khoá máy), IV ngẫu nhiên mỗi lần ghi, tên mục làm associated data, lưu `base64(iv ‖ ciphertext)` ở `SharedPreferences` `scribe_secure`. Không dùng `EncryptedSharedPreferences`. Giải mã lỗi (mất khoá) = coi như chưa đăng nhập. Làm mới phiên một lần một lúc; 409 chờ 2 giây, 401 xoá token, 410 = tài khoản đã xoá |
| Tải / lưu / phát file | `files/NativeFilesPlugin.kt` | Chỉ nhận URL `https` cùng host backend, đường dẫn `/api/`. `cache` tải vào `cacheDir/recordings-cache/`, trả URL `https://localhost/_capacitor_file_/…` (máy chủ cục bộ của Capacitor trả range, tua được). `share` tải về `cacheDir/share/` rồi mở bảng chia sẻ qua `FileProvider` (chỉ thư mục này được chia sẻ). `clear` xoá cả hai |
| Secrets | `auth/SecretsPlugin.kt` | Như iOS: chỉ tên `openaiApiKey`, lưu bằng `SecureStore`. Android chưa từng có bản cũ lưu key, nên thực tế chỉ có lệnh `remove` lúc đăng xuất |
| Ghi âm | `recorder/AudioRecorder.kt`, `recorder/AacSegmentWriter.kt` | `AudioRecord` PCM16 mono 48 kHz (máy không nhận thì 44,1 kHz) → `MediaCodec` AAC-LC (96 kbps ở 48 kHz, bảng bitrate như iOS) → `MediaMuxer` MPEG-4. Cắt đoạn đúng mẫu cuối của đoạn (không mất mẫu giữa hai đoạn), đoạn 1/5/10 phút. Đổi **major brand `mp42` → `M4A `** ngay khi đóng đoạn (`recorder/M4aBrand.kt`): backend nhận dạng theo magic byte và `mp42` sẽ bị lưu thành video mp4 |
| Ghi nền | `recorder/RecordingService.kt` | Foreground Service loại `microphone`, chỉ khởi động khi bấm Ghi (app đang mở). Thông báo "Scribe đang ghi" có đồng hồ, nút **Tạm dừng**/**Ghi tiếp**, **Đánh dấu** và **Dừng** (mở app, không dừng ngầm), hiện cả trên màn hình khoá. Không `START_STICKY`: Android 14+ cấm mở lại micro từ nền; app bị giết thì lần mở sau đối soát |
| Trạng thái ghi | `recorder/RecorderEngine.kt`, `recorder/RecorderPlugin.kt` | Engine sống theo tiến trình (Activity có thể bị huỷ và tạo lại trong lúc ghi), plugin chỉ gắn vào làm listener; native là nguồn sự thật, trang đọc `getStatus()`. `start` xin quyền Micro (và Thông báo trên Android 13+) trước khi ghi; từ chối thì báo câu tiếng Việt chỉ đường vào Cài đặt |
| Cuộc gọi / app khác lấy micro | `AudioRecorder.kt` | Android không dừng `AudioRecord` mà đưa vào im lặng. Android 10+ báo qua `AudioRecordingConfiguration.isClientSilenced`: app hiện notice "Micro đang bị cuộc gọi hoặc ứng dụng khác dùng — đoạn này sẽ im lặng, bản ghi vẫn chạy" và "Ghi tiếp sau gián đoạn" khi hết. Khác iOS (tạm dừng rồi mở đoạn mới). Đầu vào chết (lỗi đọc, 3 giây không có mẫu) thì mở lại micro vào đoạn mới |
| Lưu bền, đối soát | `recorder/SegmentStore.kt` | Cùng bố cục và JSON với iOS dưới `filesDir/recordings/<userKey>/<sessionId>/` (`session.json` + `seg-NNN.m4a`). Đoạn không đọc được bằng `MediaExtractor` (đang ghi khi bị giết) bị xoá và báo `lostSegments` |
| Tải lên | `recorder/UploadQueue.kt` | Giống iOS từng nhánh: `init → chunk (1 MiB) → finalize`, tên `<sessionId>#<index>`, tra `/api/files` trước khi tạo lại sau finalize mất phản hồi, 401 làm mới phiên một lần, 410 xoá đoạn của tài khoản, 413 chờ. Có mạng lại (`ConnectivityManager`) thì thử ngay. Không WorkManager: như iOS, chỉ tải trong lúc ghi và khi mở app |
| Tạm dừng | `recorder/AudioRecorder.kt` (`pause`/`resume`), `recorder/RecordingService.kt`, `recorder/MediaButtonControls.kt` | Nút **Tạm dừng**/**Ghi tiếp** trên thông báo (kể cả màn hình khoá), nút trong app (`Recorder.pause`/`resume`, status có `paused`) và nút phát/tạm dừng của tai nghe. Tạm dừng: micro vẫn được đọc (dịch vụ nền và watchdog giữ nguyên) nhưng không ghi gì; đoạn đang mở đóng lại và tải lên, ghi tiếp mở đoạn mới. `positionS` và đồng hồ đứng yên nên dấu mốc, chép trực tiếp nối liền; thông báo đổi thành "Scribe đã tạm dừng" và ẩn đồng hồ, ghi tiếp thì đồng hồ chạy lại từ thời gian đã ghi |
| Đánh dấu | `recorder/BookmarkStore.kt`, `recorder/MediaButtonControls.kt` | Nút **Đánh dấu** trên thông báo (kể cả màn hình khoá) thêm mốc ở đồng hồ ghi, lưu ngay `filesDir/bookmarks/<userKey>.json`, thông báo đổi thành "Đã đánh dấu 12:34" 3 giây, chống bấm đúp 1 giây; JS lấy qua `takePendingBookmarks`, xoá khi `ackBookmarks`. Nút tai nghe: một `MediaSession` nhận phím; play/pause/hook (bấm 1 lần) là tạm dừng/ghi tiếp, next/previous (bấm 2–3 lần) là đánh dấu — **có thể không bao giờ tới** vì Android giao nút media cho app vừa phát âm thanh, và app không phát im lặng như iOS. Listing Android không hứa "tai nghe" tới khi kiểm trên máy |
| Chép trực tiếp | `recorder/RealtimeTranscriber.kt`, `recorder/RealtimeApi.kt`, `recorder/Resampler.kt`, `recorder/TranscriptStore.kt` | Cùng quy tắc với iOS (token, đổi socket khi còn 20 giây, backoff 1–60 giây, gửi bù 30 giây, `quota_exceeded`, báo usage 3 lần). WebSocket bằng OkHttp (ping 15 giây). Đổi 48/44,1 kHz → 24 kHz bằng nội suy tuyến tính (không lọc chống răng cưa; đủ cho bản xem trước, file m4a không bị đổi). Sự kiện `transcript` gửi cả khi app ở nền (WebView Android vẫn chạy), không chờ `visibilitychange` |

Khác biệt với iOS cần nhớ khi đọc mã web: WebView Android không bị treo khi app ở nền (JS vẫn chạy, hẹn giờ bị
giãn); plugin chỉ ngừng gửi `level`/`elapsed` lúc app không hiện và gửi lại đồng hồ ngay khi app hiện lại.
Đăng xuất: `Recorder.signOut` của Android **không** xoá token thiết bị (bản Swift có xoá); việc đó để
`HoandatAuth.signOut()` — trang gọi ngay sau — làm sau khi gửi token tới `POST /api/auth/logout`, nhờ vậy backend thu
hồi thiết bị ở IdP. Trên iOS thứ tự này làm logout không kèm token, thiết bị còn treo ở trang `/account` của IdP
(xem `auth-context.tsx` và `server/scribe-be/src/auth.rs` hàm `logout`); sửa bản Swift là việc riêng.
Sao lưu tắt hẳn (`allowBackup=false`, `data_extraction_rules.xml` loại mọi miền): bản ghi chưa tải, token và dấu mốc
không theo sang máy khác. Quyền khai báo: `INTERNET`, `ACCESS_NETWORK_STATE`, `RECORD_AUDIO`, `FOREGROUND_SERVICE`,
`FOREGROUND_SERVICE_MICROPHONE`, `POST_NOTIFICATIONS`; không quyền ảnh/video/bộ nhớ (chọn tệp đi qua trình chọn hệ
thống, quay video đi qua app camera mà không cần quyền `CAMERA`).

## Đã kiểm / còn phải kiểm

Đã kiểm trên máy dev (23/10/2026), không cần Android:

| Hạng mục | Kết quả |
|---|---|
| `pnpm run lint`, `pnpm run typecheck` của web | Xanh |
| `scripts/brand.test.ts` (vitest) | 17/17 đạt, gồm `appIds.android === appIds.ios`, `applicationId`/`namespace` trong `build.gradle`, tài nguyên launcher khớp brand, kích thước icon PNG, icon Play 512 và feature graphic 1024 × 500 JPEG 3 kênh |
| `brand:gen` | Sinh đủ vector adaptive + monochrome, PNG 5 mật độ, màu splash, ảnh Play; icon web giữ nguyên byte |
| `targetSdk` Play yêu cầu | API 36, kiểm trên developer.android.com ngày 23/10/2026 |

Chờ CI (lần đầu biên dịch — job `android` của `ci.yml`): Kotlin biên dịch, `lintDebug`, 48 test JVM
(`web/android/app/src/test/`: PKCE theo vector RFC 7636, dựng URL và đọc callback, nhánh làm mới phiên
401/409/410 và làm mới đồng thời bằng MockWebServer, chặn host lạ khi tải, `SegmentStore` đối soát, `UploadQueue`
trọn luồng init/chunk/finalize + 401 + 410 + tra tên sau finalize mất phản hồi, dấu mốc, `transcript.jsonl` có dòng
cụt, resampler, brand m4a, token chép trực tiếp), kiểm manifest, APK debug.

Chờ máy thật (chưa làm hạng mục nào; ghi kết quả vào bảng này như `ios-app.md`):

| Hạng mục | Đạt khi |
|---|---|
| G0.1 `CapacitorHttp` tới backend prod | JS nhận 401 của `/api/me`, nginx ghi request |
| G0.2 cookie phiên dùng chung | Sau đăng nhập, `fetch` của trang và request Kotlin (tải lên, tải tệp) đều kèm cookie; làm mới phiên từ Kotlin thì trang dùng được cookie mới |
| G0.3 thân nhị phân | Chunk 6 MB của "Tải tệp lên" khớp SHA-256 phía server (quirk `File`/`Blob` của iOS có thể không có trên Android) |
| G0.4 chọn tệp, quay video | Trình chọn hệ thống mở, tệp tới server; quay video mở app camera |
| G0.5 Back, link ngoài, safe area sáng/tối | Như mục "Điều hướng" |
| G0.6 vòng đời trang | `visibilitychange` khi về nền/mở lại; xoá có hoàn tác vẫn đúng |
| Đăng nhập Auth Tab / Custom Tabs, huỷ, đăng xuất, thu hồi máy ở `/account` | Đúng một thiết bị "Scribe" ở IdP; không màn IdP nào trong WebView (`chrome://inspect` chỉ thấy `https://localhost`); `run-as … cat shared_prefs/scribe_secure.xml` chỉ thấy chuỗi mã hoá |
| Ghi khoá màn hình 3 phút, đoạn 1 phút | 3 đoạn tới server, AAC mono, `ftyp` `M4A `, BE nhận `audio/m4a`, liền mạch (ffprobe) |
| Kill app khi đang ghi | Mở lại: đoạn dở được đối soát, tải lên đúng một file |
| Đăng xuất khi còn đoạn chưa tải | Đoạn ở lại; đăng nhập lại cùng tài khoản thì tải nốt, không `init` mới |
| 60 phút khoá màn hình, rút sạc | Pin giảm ≤ 12%, đủ đoạn |
| Cuộc gọi đến giữa lúc ghi | Ghi nhận hành vi thật (im lặng + notice); quyết định chữ listing ở phase 06 |
| Máy tối ưu pin mạnh (Samsung/Xiaomi), 30 phút khoá màn hình | Không bị giết; nếu bị, viết hướng dẫn tắt tối ưu pin |
| Chế độ máy bay khi đang tải | Đoạn chờ, có mạng lại tải tiếp, không trùng |
| Đánh dấu từ thông báo khi khoá màn hình | Mốc tới server đúng `positionS` (lệch ≤ 1 giây), tắt app trước khi mở lại không mất |
| Tạm dừng / ghi tiếp từ thông báo khi khoá màn hình, rồi trong app | Khoảng tạm dừng không có trong bản ghi (ffprobe tổng các đoạn = thời gian đã ghi), đồng hồ thông báo đứng yên rồi chạy tiếp đúng chỗ, nút trong app đổi theo |
| Nút tai nghe (Bluetooth, có dây) | Ghi nhận có tới hay không; bấm 1 lần tạm dừng/ghi tiếp, 2 lần đánh dấu |
| Chép trực tiếp 10 phút khoá màn hình, mất mạng 30 giây, tài khoản ngoài `REALTIME_EMAILS` | Như bảng phase 05 của plan |
| `SecureStore` với AndroidKeyStore thật | `./gradlew connectedDebugAndroidTest` xanh |
