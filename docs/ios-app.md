# OneScribe iOS app

App iPhone bọc giao diện web React (`web/`) bằng Capacitor 8, cộng phần native Swift để **ghi âm khi khoá màn hình** và tải từng đoạn lên backend prod (`scribe.inetdev.io.vn`). Plan: `plans/260728-1609-onescribe-ios-app-ghi-nen-khoa-man-hinh/`.

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

`GoogleService-Info.plist` được commit (cấu hình công khai, API key giới hạn theo bundle id), giống cấu hình Firebase web.

## Đã kiểm / còn phải kiểm trên máy thật

| Hạng mục | Trạng thái |
|---|---|
| Gate 0 — `CapacitorHttp` tới BE prod | Đạt trên iPhone 15 Pro Max (2026-10-15): JS nhận 401 của `/api/me`, nginx ghi nhận request |
| Self-test Simulator (`-OSSelfTest`, chỉ bản Debug) | Đạt: đối soát phát hiện đoạn cụt, tên tất định, hàng đợi dừng ở "cần đăng nhập" khi 401 |
| m4a qua BE | `ftyp` = `M4A `; `infer 0.16` (cùng bản BE dùng) nhận `audio/m4a`; BE trả `audio/mp4` khi phát |
| Gate 1 — đăng nhập Google trên iPhone | **Chưa** — cần chủ sở hữu bấm đăng nhập trên máy |
| Ghi thật khi khoá màn hình, cuộc gọi, tải lên | **Chưa** — Simulator trên Mac mini không có micro (engine abort ở `AURemoteIO`); engine giống spike đã chạy 63 phút trên máy |
