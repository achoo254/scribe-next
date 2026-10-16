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

## Cách app hoạt động

| Phần | Ở đâu | Ghi chú |
|---|---|---|
| Giao diện | `web/dist` đóng gói trong app (`capacitor://localhost`) | Cùng mã với web; nhánh native qua `isNativeShell()` (`web/src/lib/api.ts`) |
| Điều hướng | `MainViewController.swift` | Mỗi trang có URL; app bật cử chỉ vuốt từ mép trái để lùi/tiến trang (`allowsBackForwardNavigationGestures`, Capacitor để tắt theo mặc định). Link IdP trong account menu và footer mở ở trình duyệt hệ thống |
| Gọi API | `CapacitorHttp` → `URLSession` | Không bị CORS/`SameSite`; cookie phiên nằm ở `HTTPCookieStorage` |
| Đăng nhập hoandat-auth | `HoandatAuthPlugin.swift` | Client public `scribe-mobile`: lấy tham số ở `GET /api/auth/native/config`, app tự sinh PKCE + `nonce`, mở `auth.hoandat.com` bằng `ASWebAuthenticationSession`, redirect `com.hoandat.scribe:/auth/callback` → gửi `code` + verifier + `nonce` tới `POST /api/auth/native/exchange`. Backend giữ refresh token IdP (mã hoá), trả cookie phiên + token thiết bị |
| Tải / lưu / phát file | `NativeFilesPlugin.swift` | Tải kèm cookie; share sheet (Lưu vào Tệp); bản cache để phát và tua |
| Ghi âm | `Recorder/AudioRecorder.swift` | AVAudioEngine → m4a AAC, đoạn 1/5/10 phút, chạy nền (`UIBackgroundModes: audio`) |
| Lưu bền | `Recorder/SegmentStore.swift` | `Documents/recordings/<userKey>/<sessionId>/session.json` + `seg-NNN.m4a`; đối soát khi mở app, báo đoạn cụt nếu app bị kill |
| Tải lên | `Recorder/UploadQueue.swift` | Mỗi đoạn một bản ghi tên `<sessionId>#<index>`; Lịch sử ghi gom thành một dòng "Bản ghi trên iPhone". Mất phản hồi finalize thì tra theo tên trước khi tạo lại (không sinh bản trùng) |
| Làm mới phiên | `SessionAuth.swift` | Token thiết bị của Scribe giữ trong Keychain của app (`AfterFirstUnlockThisDeviceOnly`). Phiên mobile 12 giờ; gặp 401 khi đang tải, hàng đợi đổi token thiết bị lấy cookie mới ở `POST /api/auth/native/refresh` (chỉ được khi hoandat-auth còn công nhận máy; thu hồi máy ở `/account` của IdP thì lần làm mới sau thất bại), rồi mới báo "cần đăng nhập lại". Đăng xuất thì xoá token và thiết bị trên server |
| Chép lời | (không có phần native) | Dùng key OpenAI của server, tính vào phút của tài khoản. `SecretsPlugin.swift` chỉ còn để xoá key người dùng tự nhập ở bản cũ khi đăng xuất |

> **Đổi bundle id và đăng nhập (2026-10-16):** app id là `com.hoandat.scribe`, đăng nhập qua hoandat-auth; Firebase đã bỏ hẳn. Bản cài mới là một app khác trên máy: dữ liệu cục bộ của app cũ (đoạn chưa tải, Keychain) không chuyển sang. Refresh token Firebase của bản cũ (nếu có) bị xoá ở lần đăng nhập hoặc đăng xuất kế tiếp.

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
| QA cuối giao diện mới (đăng xuất từ menu, theme sáng/tối, safe area) | **Chưa** |
| Ghi dài không sạc, đo pin (ngưỡng ≤ 12% / 60 phút) | Đạt (2026-10-16): 48:07 khoá màn hình, đã rút sạc, đoạn 1 phút; pin 76% → 76% (dưới ~2%); server nhận đủ 49 đoạn liền mạch (48 × ~60 s + 3,9 s), tải lên từng phút 08:43:38–09:30:42 |
