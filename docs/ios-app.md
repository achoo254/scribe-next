# Scribe iOS app

App iPhone bọc giao diện web React (`web/`) bằng Capacitor 8, cộng phần native Swift để **ghi âm khi khoá màn hình** và tải từng đoạn lên backend prod (origin `VITE_NATIVE_API_ORIGIN` trong `web/.env`, nhúng vào Info.plist lúc build). Plan: `plans/260728-1609-scribe-ios-app-ghi-nen-khoa-man-hinh/`.

## Cài lên iPhone

Cắm iPhone vào Mac, mở khoá, rồi:

```bash
cd web
npm run ios:device
```

Script `web/scripts/ios-device.sh` build web → `cap sync ios` → `xcodebuild` (Release, Personal Team `AM5C55N339`, tự cấp provisioning) → cài → mở app. Chạy lại đúng lệnh này khi profile miễn phí hết hạn (7 ngày): app lại mở được, dữ liệu trên máy giữ nguyên.

Cần trên máy build:
- Xcode ở `/Applications/Xcode.app` (script tự đặt `DEVELOPER_DIR`, không cần `sudo xcode-select`), Apple ID của team đã đăng nhập trong Xcode.
- `web/.env` (cấu hình Firebase web, xem `web/.env.example`).
- Lần đầu cài: trên iPhone vào *Cài đặt → Cài đặt chung → Quản lý VPN & thiết bị* để tin cậy nhà phát triển.

## Cách app hoạt động

| Phần | Ở đâu | Ghi chú |
|---|---|---|
| Giao diện | `web/dist` đóng gói trong app (`capacitor://localhost`) | Cùng mã với web; nhánh native qua `isNativeShell()` (`web/src/lib/api.ts`) |
| Gọi API | `CapacitorHttp` → `URLSession` | Không bị CORS/`SameSite`; cookie phiên nằm ở `HTTPCookieStorage` |
| Đăng nhập Google | `GoogleAuthPlugin.swift` | OAuth PKCE qua `ASWebAuthenticationSession` (client iOS của Firebase project) → `accounts:signInWithIdp` → Firebase ID token → `POST /api/auth/session`. Không dùng Firebase iOS SDK |
| Tải / lưu / phát file | `NativeFilesPlugin.swift` | Tải kèm cookie; share sheet (Lưu vào Tệp); bản cache để phát và tua |
| Ghi âm | `Recorder/AudioRecorder.swift` | AVAudioEngine → m4a AAC, đoạn 1/5/10 phút, chạy nền (`UIBackgroundModes: audio`) |
| Lưu bền | `Recorder/SegmentStore.swift` | `Documents/recordings/<userKey>/<sessionId>/session.json` + `seg-NNN.m4a`; đối soát khi mở app, báo đoạn cụt nếu app bị kill |
| Tải lên | `Recorder/UploadQueue.swift` | Mỗi đoạn một bản ghi tên `<sessionId>#<index>`; Lịch sử ghi gom thành một dòng "Bản ghi trên iPhone". Mất phản hồi finalize thì tra theo tên trước khi tạo lại (không sinh bản trùng) |
| Làm mới phiên | `SessionAuth.swift` | Refresh token Firebase giữ trong Keychain của app (`AfterFirstUnlockThisDeviceOnly`). Gặp 401 khi đang tải, hàng đợi tự đổi lấy ID token mới rồi gọi lại `POST /api/auth/session`; chỉ khi thất bại mới báo "cần đăng nhập lại". Đăng xuất thì xoá token |
| Key OpenAI (Chép lời) | `SecretsPlugin.swift` | Key người dùng tự nhập, lưu Keychain của app (chỉ nhận tên `openaiApiKey`). Gửi kèm mỗi lần bấm "Chép lời"; đăng xuất thì xoá |

`GoogleService-Info.plist` được commit (cấu hình công khai, API key giới hạn theo bundle id), giống cấu hình Firebase web.

> **Đổi bundle id (2026-10-16):** app id mới là `com.hoandat.scribe` (trước đây dùng bundle id khác). `GoogleService-Info.plist` (không có trong git, tải từ Firebase Console) vẫn là cấu hình Firebase của bundle id cũ và được giữ nguyên tới khi chuyển đăng nhập sang hoandat-auth. Đăng nhập Google vẫn chạy (kiểm trên iPhone 15 Pro Max 2026-10-16, server ghi "login thành công"): `GoogleAuthPlugin` gửi bundle id lấy từ plist trong header `X-Ios-Bundle-Identifier`, nên giới hạn API key của Firebase vẫn khớp. Phần này bỏ đi khi chuyển sang hoandat-auth. Bản cài mới là một app khác trên máy: dữ liệu cục bộ của app cũ (đoạn chưa tải, Keychain) không chuyển sang.

## Đã kiểm / còn phải kiểm trên máy thật

| Hạng mục | Trạng thái |
|---|---|
| Gate 0 — `CapacitorHttp` tới BE prod | Đạt trên iPhone 15 Pro Max (2026-10-15): JS nhận 401 của `/api/me`, nginx ghi nhận request |
| Self-test Simulator (`-OSSelfTest`, chỉ bản Debug) | Đạt: đối soát phát hiện đoạn cụt, tên tất định, hàng đợi dừng ở "cần đăng nhập" khi 401 |
| m4a qua BE | `ftyp` = `M4A `; `infer 0.16` (cùng bản BE dùng) nhận `audio/m4a`; BE trả `audio/mp4` khi phát |
| Gate 1 — đăng nhập Google trên iPhone | Đạt trên iPhone 15 Pro Max (2026-10-16): server log "login thành công", `POST /api/auth/session` 200 |
| Ghi khi khoá màn hình ~3 phút, đoạn 1 phút, tải lên | Đạt (2026-10-16): 3 đoạn tới server ngay khi xong (08:24:24, 08:25:24, 08:26:20), AAC 48 kHz mono, 60,05 + 60,05 + 56,06 s liền mạch; phát lại 200; `session.json` đánh dấu đã tải, không còn `.m4a` trên máy |
| Cuộc gọi đến, AirPods, chế độ máy bay khi đang tải | **Chưa** — cần máy thứ hai để gọi vào và AirPods |
| Kill app khi đang ghi | Đạt (2026-10-16): vuốt tắt sau ~30 s; mở lại thì đoạn dở 31,3 s được đối soát và tải lên (server 08:29:41, ffprobe 31,36 s) |
| Đăng xuất khi còn đoạn chưa tải | Đạt (2026-10-16): đoạn 90,8 s ở lại trên máy, `uploaded: false`; server có `recordings/init` 3 s trước `logout` nhưng không có `finalize`, không file nào vào tài khoản. Sau đăng xuất máy có mạng, 08:31:51–08:35:02 (gồm lúc mở app để đăng nhập lại) server không nhận request nào. Đăng nhập lại cùng tài khoản (08:35:02) thì đoạn tồn tải nốt ngay: `finalize` 08:35:03 nối tiếp phiên tải cũ (không `init` mới), ffprobe 90,86 s, đúng một file |
| Ghi dài không sạc, đo pin (ngưỡng ≤ 12% / 60 phút) | Đạt (2026-10-16): 48:07 khoá màn hình, đã rút sạc, đoạn 1 phút; pin 76% → 76% (dưới ~2%); server nhận đủ 49 đoạn liền mạch (48 × ~60 s + 3,9 s), tải lên từng phút 08:43:38–09:30:42 |
