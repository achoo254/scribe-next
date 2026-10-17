---
title: "Scribe iOS — app ghi âm nền khi khoá màn hình"
description: "App iOS hybrid (Capacitor + plugin Swift) tái dùng UI React (web/) và backend hiện tại; ghi âm sống sót khoá màn hình, cuộc gọi đến, chuyển app."
status: in-progress
priority: P1
branch: "main"
tags: [ios, capacitor, swift, avfoundation, mobile]
blockedBy: []  # web React rewrite đã xong phần code (2026-10-15); deploy web lên prod không chặn iOS
blocks: []
created: "2026-07-28"
createdBy: "ak:plan"
source: skill
---

# Scribe iOS — app ghi âm nền khi khoá màn hình

> **Bản 2 — viết lại sau red-team.** Bản 1 chứa một lỗ hổng kiến trúc Critical (tầng transport/origin) và cam kết "zero backend change" không đứng vững. Xem `## Red Team Review` cuối file.

## Overview

Đưa Scribe lên iPhone với **đầy đủ tính năng của web app hiện tại**, cộng thứ mà web vĩnh viễn không làm được: **ghi âm tiếp tục khi khoá màn hình, có cuộc gọi đến, hoặc chuyển sang app khác**.

Người dùng: **1 người duy nhất** (chủ sở hữu), cài bằng Apple ID cá nhân (Personal Team `AM5C55N339`, free provisioning), iPhone 15 Pro Max iOS 26.5.2, build trên Mac mini M4 (Xcode 26.6; từ 17/10/2026 là Xcode 27.1). Nếu chạy ổn thì **sau này có thể đưa lên App Store**.

Phase 0 (spike) **đã xong và gate xanh** — xem `ios-spike/` (commit `be7bf46`).

## Cập nhật 2026-10-15 — web đã viết lại bằng React, môi trường chỉ còn dev local + prod

Tên file Vue trong plan và các phase là **lịch sử**; khi làm, dùng bảng này. Đường dẫn trong `web/src/`:

| Plan nhắc (Vue) | Hiện tại (React) |
|---|---|
| 25 lời gọi `fetch` rải rác trong composable | Một hàm `lib/api.ts` → `apiFetch`/`apiJson`; `apiBase()` trả `https://$DEPLOY_DOMAIN` khi `Capacitor.isNativePlatform()` (đã có test). Gate 0 vẫn phải đo `CapacitorHttp` vì request đi qua đây |
| 5 chỗ `window.location.href`, `use-download.ts` mới | `lib/download.ts` → `download(path)`, `mediaUrl(fileId)`, `downloadPaths` — chỉ còn **một** chỗ để thêm nhánh native |
| `use-auth.ts`, `lib/firebase.ts` | `auth/auth-context.tsx` (`useAuth`), `auth/auth-api.ts`, `lib/firebase.ts` (khởi tạo lười) |
| `use-recorder.ts` / `use-recorder-native.ts` | `features/recorder/recorder.ts` (class thuần, `getState/subscribe`) + `use-recorder.ts`; bản native thay `Recorder` bằng cài đặt gọi plugin Swift cùng giao diện |
| `use-upload.ts`, `use-splitter.ts` | `features/upload/chunked-upload.ts`, `features/split/split-job.ts` |
| `use-recordings.ts`, `use-managed-files.ts`, `use-usage.ts` | `features/files/files-api.ts` + `file-queries.ts` (TanStack Query) |
| `RecordView.vue`, `RecordingHistory.vue`, `MediaPlayer.vue` | `pages/record/record-page.tsx`, `pages/record/recording-history.tsx` + `recording-row.tsx`, `components/media-player.tsx` |
| `SplitView.vue`, `AppTopBar.vue`, `LoginView.vue` | `pages/split/split-page.tsx`, `layout/app-top-bar.tsx`, `pages/login-page.tsx` |
| `vue-tsc` | `npm run typecheck` (`tsc --noEmit`) |
| "staging" `.65` | **prod** `.65` (`$DEPLOY_DOMAIN`); dev = máy local, xem `docs/deploy-web-production.md` |

Backend prod hiện chưa có access log/audit (commit `fa810f1` mới ở repo, chưa deploy); không chặn iOS.

## Nền tảng đã chứng minh (Phase 0)

Đo trên máy thật, 3 phiên, tổng **63 phút**:

| Kết quả | Số liệu |
|---|---|
| Ghi liên tục khi khoá màn hình | ✅ phiên dài nhất 33m48s, khoá liên tục 29m13s |
| Đoạn nguyên vẹn | ✅ **14/14**, thời lượng đọc lại lệch **0.0s** |
| Xoay đoạn theo phút | ✅ 11 lần, **9.6–11.0ms** |
| Lỗi ghi / engine chết / bị hệ thống kill | ✅ **0 / 0 / 0** |
| Chi phí pin | ⏸ dưới ngưỡng phân giải 1% của `batteryLevel` — đo lại ở Phase 2 với ngưỡng số |
| Cuộc gọi đến thật | ⏸ chưa đo — gate của Phase 2 |

Bốn bài học kỹ thuật **bắt buộc mang sang app thật** (đều đã trả giá bằng lỗi thật ở spike):

1. **Bitrate AAC phải tính theo sample rate.** 128 kbps bị từ chối ở 16 kHz mono (route HFP) → `kAudioCodecUnsupportedFormatError` (`'!dat'`).
2. **iOS chọn bất kỳ thiết bị HFP ghép đôi nào làm mic**, kể cả smartwatch → phải `setPreferredInput(builtInMic)`.
3. **Đổi route xảy ra bất đồng bộ ngay sau `setActive`** → cần retry một lần khi khởi động lỗi.
4. **Mức bảo vệ file phải là `completeUntilFirstUserAuthentication`**, nếu không sẽ không ghi được khi máy khoá.

Điều spike **không** chứng minh, đừng nhầm lẫn: nó không có tầng lưu trữ bền vững (`grep JSONEncoder ios-spike/` → không có kết quả), không có upload, không có auth. "14/14 đoạn nguyên vẹn" nói về engine ghi, **không** nói gì về `SegmentStore` hay `UploadQueue` của Phase 2.

## Quyết định kiến trúc

### D1 — Hybrid Capacitor, KHÔNG phải SwiftUI thuần

| | Capacitor hybrid | SwiftUI thuần |
|---|---|---|
| Tái dùng UI | ~70% (giảm từ ước tính 85% của bản 1 — xem D3) | 0% |
| Codebase phải bảo trì | 2 (web + vỏ iOS) | 3 |

`SplitView` (354 LOC), `RecordingHistory`, `QuotaBar`, `LoginView`, `AppTopBar` chạy lại được. `<input type="file">` trong WKWebView mở picker Files gốc.

**Đính chính bản 1**: bản 1 tuyên bố tab Cắt file và Lịch sử ghi chạy "mà không viết một dòng Swift nào". **Sai** — tải file và phát media đều cần shim native (D3). Con số đúng là ~70%, không phải 85%.

**KHÔNG chọn Tauri iOS**: code Rust desktop 0% dùng lại được trên iOS, plugin ghi âm vẫn phải viết Swift.

### D2 — WebView là UI, Swift là nguồn chân lý

WKWebView **bị iOS đình chỉ** khi khoá màn hình. Mọi state của phiên ghi (thời lượng, chỉ số đoạn, trạng thái upload) **phải nằm trong Swift**; Vue **ghi đè** state của mình từ Swift khi foreground, không bao giờ merge.

### D3 — Tầng transport: mọi request đi qua native ⭐ QUYẾT ĐỊNH TRUNG TÂM CỦA BẢN 2

Vấn đề bản 1 bỏ sót hoàn toàn. Capacitor bundle `web/dist` → origin WebView là `capacitor://localhost`. Nhưng:

| Sự thật đã verify | Vị trí |
|---|---|
| Backend **không có CORS**; `tower-http` không bật feature `cors` | `server/scribe-be/Cargo.toml:19`, `src/routes.rs:11-53` |
| nginx **không set header CORS** nào | `deploy/nginx-scribe.conf` |
| Cookie session là `SameSite=Lax` → không gửi cross-site | `server/scribe-be/src/auth.rs:92` |
| FE có **25 đường dẫn `/api/` tương đối** | `web/src/composables/*`, `web/src/components/*` |
| FE có **5 chỗ `window.location.href`** để tải file | `use-recordings.ts:49,53,57` · `use-splitter.ts:113` · `use-managed-files.ts:56` |
| `<audio :src>` dùng path tương đối | `web/src/components/MediaPlayer.vue:72,80` |
| Đây là giả định thiết kế có chủ ý của web | `web/vite.config.ts:4-5` — *"FE + BE cùng origin (cookie same-site, không CORS)"* |

**Quyết định: định tuyến toàn bộ traffic API qua native, không qua WebKit.**

| Loại | Cách xử lý | Vì sao khả thi |
|---|---|---|
| 25 lời gọi `fetch` | Bật plugin **`CapacitorHttp`** — nó patch `fetch`/`XMLHttpRequest` để đi qua native | **CORS là chính sách của trình duyệt.** Request phát từ `URLSession` không bị CORS, không bị ràng buộc `SameSite`. Cookie nằm ở `HTTPCookieStorage` native |
| 5 chỗ tải file | Composable mới `use-download.ts`, nhánh `isNative` → plugin native tải rồi mở share sheet | `window.location.href` cross-origin bị Capacitor chặn hoặc đẩy ra Safari (nơi không có cookie) |
| Phát media | `MediaPlayer.vue` nhánh `isNative` → native tải về file tạm → `<audio>` trỏ vào file cục bộ | `<audio src>` là subresource của WebKit, JS **không** can thiệp được. Đổi lại: tua mượt hơn Range vì file đã ở local |

**Hệ quả đã chấp nhận**: phát lại phải tải xong file mới nghe được (đoạn ghi 3.73 MB — không đáng kể; file upload Mode B tới 2GB — hiển thị tiến độ, ghi nhận là hạn chế).

**Phương án đã loại**: `server.url` trỏ thẳng domain thật. Giữ same-origin, zero shim — nhưng **mất mạng là mất luôn UI**, không bấm được nút Ghi. Không chấp nhận được với app mà độ tin cậy là điểm bán duy nhất. Nó cũng làm yếu lập luận Guideline 4.2 khi nộp App Store.

**Fallback đã chốt nếu Gate 0 fail** (validate 2026-07-28): thêm `CorsLayer` vào BE + native đẩy session token qua bridge cho JS dùng `Authorization: Bearer`. **Giữ nguyên cookie `SameSite=Lax`** — không đổi sang `None`, vì BE không có CSRF token nên hạ SameSite là bỏ lá chắn CSRF duy nhất của web đang chạy. Đường này phá cam kết zero-code-change (sửa `Cargo.toml` bật feature `cors` + `routes.rs`), nhưng phạm vi hẹp và additive. **Không cần hỏi lại khi Gate 0 fail — cứ đi đường này.**

### D4 — Ranh giới thay đổi backend (thay cho "zero BE change" của bản 1)

Bản 1 tuyên bố "zero thay đổi backend, đã kiểm chứng". **Tuyên bố đó sai** — chỉ tầng container được kiểm chứng, tầng transport chưa bao giờ được xét.

Ranh giới đúng, sau khi verify:

| Hạng mục | Trạng thái |
|---|---|
| **Code backend (`server/**/*.rs`)** | **Không đổi nếu Gate 0 pass.** Nếu Gate 0 fail → thêm `CorsLayer` (`Cargo.toml` + `routes.rs`), đã chốt ở D3, không hỏi lại |
| **Cấu hình deploy backend** | **Không đổi.** Allowlist **giữ mở** — quyết định của chủ sở hữu ở validate 2026-07-28, xem `## Validation Log` V4 |
| Container m4a | ✅ verify: `recordings.rs:172` `is_ebml = matches!(ext, "webm"\|"mkv")` → m4a bỏ qua remux; `infer 0.16.0` (pin ở **`Cargo.lock` gốc workspace**, không phải `server/scribe-be/Cargo.lock` như bản 1 ghi sai) có `is_m4a`. `ftypM4A ` của CoreAudio không nằm trong danh sách brand của `is_mp4`, nên không bị nhận nhầm — nhưng `infer` đăng ký Video **trước** Audio và trả match đầu tiên, nên nếu đổi writer sang `AVAssetWriter` brand có thể đổi → Phase 2 phải assert `ftypM4A ` |

### D5 — Auth: native sở hữu toàn bộ

Vì D3 đưa mọi request qua native, **cookie chỉ cần tồn tại ở một kho** (`HTTPCookieStorage`). Việc tiêm cookie sang WKWebView của bản 1 trở nên **không cần thiết** — đây là đơn giản hoá lớn, không phải thêm việc.

```
Google sign-in native → Firebase ID token
    → URLSession POST /api/auth/session → cookie vào HTTPCookieStorage
    → CapacitorHttp tự đính cookie cho mọi fetch của Vue
```

**Ba lỗ hổng của bản 1 phải xử lý:**

1. **Session hết hạn.** TTL là **12h** (`config.rs:54`) và **không có route refresh nào** (`routes.rs` — 0 kết quả cho `auth/refresh`). Bản 1 xếp 401 vào "lỗi vĩnh viễn" → hàng đợi upload tự chết sau một đêm. **Sửa**: 401 = *re-authenticate rồi thử lại*, không phải vĩnh viễn. `SessionAuth` giữ khả năng refresh Firebase ID token im lặng (`getIDToken(forcingRefresh:)`) → re-POST `/api/auth/session` → retry một lần. Thêm refresh chủ động ở mốc T-30 phút.
2. **Logout không sạch.** Bản 1 để Vue chủ động logout, chỉ xoá kho cookie của WebView. BE dùng JWT stateless **không có revocation** (`session.rs:60-70`), nên native vẫn upload tiếp tới 12h vào tài khoản cũ. **Sửa**: logout là thao tác native — drain/abort `UploadQueue` → POST logout → xoá cookie ở **cả hai** kho → native Firebase `signOut()`.
3. **Lối lui auth của bản 1 đều không chạy.** `ASWebAuthenticationSession` lấy Google ID token thẳng sẽ **bị BE từ chối** vì `jwks.rs:73-75` pin `aud`/`iss` theo securetoken Firebase. `signInWithRedirect` không hoạt động từ origin `capacitor://`. **Lối lui đúng duy nhất**: `ASWebAuthenticationSession` → Google credential → đổi lấy **Firebase** ID token qua REST `accounts:signInWithIdp` bằng web API key đã commit công khai ở `web/.env.staging`. Token này do securetoken phát hành nên `jwks.rs` chấp nhận, và không cần Firebase iOS SDK (né luôn rủi ro Keychain).

### D6 — Ghi m4a, xoay đoạn 1 phút, upload nguyên đoạn

Mỗi đoạn hoàn chỉnh là một file `kind:recording`, khớp hành vi web khi bật "Cắt thành đoạn theo phút". Kích thước đo thực tế: **3.73 MB / 5 phút ≈ 45 MB/giờ**.

**Đổi so với bản 1 — mặc định 1 phút, không phải 5 phút.** Lý do: `AVAudioFile` chỉ ghi moov atom khi đóng file. Nếu iOS kill app giữa chừng, **đoạn đang mở là file hỏng, không đọc được**. Bản 1 chỉ hứa "đoạn đã đóng còn nguyên" và im lặng về đoạn đang mở — tức Goal 3 không thật. Hạ xuống 1 phút thì bán kính thiệt hại là 1 phút, và không tốn gì (spike đo xoay đoạn 9.6–11.0ms).

**Bỏ resumable theo byte offset.** Bản 1 hứa "tiếp tục từ offset server báo" nhưng **`/api/recordings/status` không tồn tại** (chỉ `/api/upload/status` cho Mode B — `routes.rs:18` vs `:21-23`). Với đoạn 1 phút ≈ 750 KB, gửi lại cả đoạn tốn vài giây. **Upload nguyên đoạn, thất bại thì bỏ session server và làm lại từ đầu.** Xoá được toàn bộ sổ sách offset.

### D7 — Không phụ thuộc thứ mà free provisioning không có

Live Activity, App Groups, Push, TestFlight **không dùng ở v1**. `AudioRecorder` không import Capacitor (điều kiện để sau này gắn UI native), nhưng **không** dựng protocol `RecorderDelegate` cho subscriber giả định — free provisioning không ship được Widget Extension, nên subscriber thứ ba không tồn tại. Dùng một closure `onEvent`.

### D8 — iOS v1 chỉ có mic

iOS không cho app đọc audio của app khác. Toggle "Tiếng hệ thống" và "Ghi kèm màn hình" **không có** trên iOS. Ghi màn hình cần ReplayKit Broadcast Extension → cần App Groups → cần tài khoản trả phí.

## Sự thật về backend mà bản 1 bỏ sót

Bốn hành vi đã verify, phải hiện diện trong thiết kế client:

1. **`finalize` không idempotent.** Nó xoá session doc sau khi tạo file (`recordings.rs:215-219`). Mất response → retry → `load_session` → **404** (`:256`). Không có dedupe key; `file::create` là `insert_one` thuần. **Client phải**: đặt tên đoạn tất định `<sessionId>#<index>`, và khi gặp 404-on-finalize thì đối chiếu `GET /api/files` trước khi kết luận thất bại.
2. **Quota chặn ở CẢ `init` VÀ `chunk`, và `chunk` xoá luôn `.part`** (`recordings.rs:110-115`). Bản 1 chỉ nói "BE trả lỗi ở init". Sau khi bị xoá, mọi retry theo offset cũ sẽ dính `400 "offset không liền mạch"` (`:86-89`).
3. **Retention xoá `recording_sessions` sau 24h** (`retention.rs:14,15,129`). Offline quá một ngày → session server biến mất → chunk trả 404. Không phải giả định: profile 7 ngày của Phase 4 đảm bảo sẽ có những khoảng >24h.
4. **File bị xoá sau 14 ngày** (`RETENTION_DAYS=14`, `expiresAt` gắn cho mọi recording). **Server là cache 14 ngày, không phải kho lưu trữ.** Plan xoá bản cục bộ sau khi upload → tới ngày 15 bản ghi biến mất khỏi cả hai nơi. Phải hiển thị hạn trong Lịch sử ghi.

→ Phân loại lỗi phải có **ba** nhóm, không phải hai: **tạm thời** (retry) · **làm lại được** (400 offset, 404 session, 400 no-data → bỏ session server, `init` lại từ đầu) · **kết thúc** (403, audio hỏng).

## Goals

| # | Goal | Ưu tiên |
|---|------|---------|
| 1 | Ghi âm sống sót khoá màn hình / cuộc gọi / chuyển app | P1 |
| 2 | Đầy đủ tính năng web: Ghi âm, Cắt file, Lịch sử + phát lại, Quota, Xoá | P1 |
| 3 | Không mất dữ liệu khi mất sóng / app bị kill — **trừ đoạn đang mở, bán kính tối đa 1 phút** (D6). Bản ghi quan trọng export được ra Files trước hạn 14 ngày (V2) | P1 |
| 4 | Không sửa code backend, **trừ khi Gate 0 fail** (D3/D4). Phase 5–6 là ngoại lệ có chủ đích: cần endpoint token tạm thời, chặn chi phí và `TranscribeWorker` | P2 |
| 5 | Code tách lớp, sẵn sàng nâng cấp App Store | P2 |

## Phases

| # | Phase | Tier | Trạng thái |
|---|-------|------|-----------|
| 1 | [Vỏ Capacitor + transport + auth + parity không-ghi](./phase-01-vo-capacitor-va-auth.md) | **L** | Xong. Gate 0 đạt 15/10; đăng nhập hoandat-auth, ghi, tải lên, chép lời, phát lại đạt trên iPhone 16/10. "Tải tệp lên"/"Cắt file" trong app sửa 17/10 (xem dưới), chờ kiểm lại trên máy |
| 2 | [Recorder + upload](./phase-02-recorder-va-upload.md) | **L** | Xong, QA trên iPhone 16/10 (khoá màn hình, kill app, đăng xuất khi còn đoạn, 48 phút không sạc). Còn: cuộc gọi đến, AirPods, chế độ máy bay |
| 3 | [UI parity tab Ghi âm](./phase-03-ui-parity-tab-ghi-am.md) | **M** | Xong, QA trên iPhone 16/10. Giao diện mới (refactor 16/10 chiều) chưa cài lên máy — xem "Lần cắm iPhone tới" |
| 4 | [Deploy, vận hành, lộ trình App Store](./phase-04-deploy-va-van-hanh.md) | **S** | Xong: `pnpm run ios:device`, `docs/ios-app.md`, lộ trình App Store trong `docs/project-roadmap.md` |
| 5 | [Realtime transcript](./phase-05-realtime-transcript.md) | **L** | Đang làm — mã xong 17/10 (backend, Swift, giao diện; chỉ email trong `REALTIME_EMAILS`). Còn đo trên iPhone thật: Gate 0 bước 2 và Success Criteria |
| 6 | [Chép lời bản ghi bằng OpenAI (key của server)](./phase-06-ban-hoan-chinh-sau-buoi-ghi.md) | **L** | Xong 16/10: dùng key OpenAI của server, tính phút theo tài khoản (commit `a90a7e8`), mở cho mọi người 16/10 14:19; đã chép lời trên iPhone thật |

Thứ tự: **1 → 2 → 3 → 4**, rồi **6 trước 5** (2026-10-15: Phase 6 không cần iPhone để kiểm phần lớn). Ngày 16/10 Phase 6 đổi sang key OpenAI của server, tính vào phút của tài khoản — quyết định T1/T3/T4 (key người dùng tự nhập) không còn hiệu lực. Phase 5 và 6 thêm ngày 2026-10-15, sau khi đo OpenAI Realtime cho tiếng Việt ([báo cáo](../dattqh/reports/research-261015-1415-realtime-transcription-probe.md)).

Đổi so với bản 1: Phase 2+3 cũ **gộp làm một** (tách chúng bắt phải sửa `RecordView.vue` ba lượt và dựng UI tạm rồi xoá); Phase 6 cũ **xoá**, nội dung lộ trình App Store thành sản phẩm tài liệu của Phase 4. Phase 1 lên **L** vì gánh thêm tầng transport.

## Cập nhật 2026-10-17 — kiểm tính năng web mới trong vỏ iOS bằng Simulator

Bản cài trên iPhone là bản 16/10 trưa (đăng nhập hoandat-auth). Từ đó web thêm: giao diện hệ sinh thái + URL cho từng trang, biên bản, dấu mốc theo đồng hồ native (`positionS`), tìm kiếm và hỏi đáp, "Tải tệp lên", xoá nhanh có hoàn tác, xoá tất cả, thẻ gói/dùng lượng, liên kết chat.

Không cắm iPhone, kiểm bằng Simulator iOS 27.0 (iPhone 17, bản Debug) trỏ vào **backend giả lập trên máy** (dữ liệu mẫu của e2e, không đăng nhập thật, không chạm prod). Simulator chạy không có cửa sổ (Xcode 27.1 chưa chạy first-launch nên không có Simulator.app), nên thao tác bằng script trong trang (click DOM) chứ không chạm màn hình.

| Hạng mục | Kết quả | Cách kiểm |
|---|---|---|
| **Tải tệp lên / Cắt file** | **Lỗi, đã sửa.** `CapacitorHttp` chỉ gửi đúng thân nhị phân khi là `File`; `file.slice()` là `Blob` nên bị từ chối ngay trong native (`CapacitorUrlRequestError 0`) → mọi chunk báo "Không kết nối được máy chủ". `ArrayBuffer` cũng bị từ chối, `Uint8Array` bị đổi thành UTF-8 (hỏng). Sửa: `apiFetch` bọc `Blob` thành `File` khi chạy native (`nativeSafeBody` trong `web/src/lib/api.ts`). Sau sửa: tệp 6 MB, 2 chunk, server nhận đủ 6 291 456 byte, SHA-256 khớp | Simulator |
| Xoá nhanh có hoàn tác khi app vào nền | **Lỗ hổng, đã sửa.** Trong app không có `pagehide`; `pause`/`visibilitychange` chỉ tới trang **khi app mở lại** (JS không chạy lúc ở nền). Tắt app trong 5 giây hoàn tác thì DELETE không bao giờ gửi, bản ghi quay lại. Sửa: id đang chờ xoá lưu ở `localStorage` (`scribe.pendingDeletes`), lần sau Lịch sử ghi mở thì xoá nốt. Đã kiểm: xoá → kill app → 0 DELETE → mở lại → DELETE gửi ngay | Simulator |
| `keepalive` qua `CapacitorHttp` | Bị bỏ qua nhưng không hại: request vẫn đi qua `URLSession` (đã thấy server nhận) | Simulator |
| Chọn tệp: `accept="audio/*,video/*"` | Đúng. **Thiếu `NSCameraUsageDescription`**: menu chọn tệp của iOS có "Quay video", bấm vào sẽ làm app bị hệ thống tắt. Đã thêm chuỗi vào `Info.plist`. Mở picker (Ảnh/Tệp) cần chạm thật | Đọc mã; cần máy thật |
| Xoá tất cả, thẻ gói + dùng lượng, tìm kiếm, hỏi đáp, trang Lời đã chép | Đạt | Simulator |
| Hộp liên kết chat | Mở được, tạo link một lần qua native. Bấm "Mở chat" bằng script không mở Safari (không phải cử chỉ thật, WKWebView chặn `window.open`); nút gọi `openExternal` ngay trong click, Capacitor chuyển sang `UIApplication.open` | Simulator + đọc mã; cần máy thật |
| Ghi native + dấu mốc | Simulator ghi được: `getStatus()` trả `positionS` 6,5 → 8,0 s cùng `elapsedS`, dấu mốc gửi `POST /api/transcript-docs/s:<session>/bookmarks`, giao diện "Đã đánh 1 mốc". Mã Swift `recordedSeconds` đọc dưới `fileLock` | Simulator + đọc mã |

## Lần cắm iPhone tới

Cắm iPhone, mở khoá, rồi:

```bash
cd web
pnpm run ios:device
```

Lưu ý: Xcode đã lên 27.1 và chưa chạy first-launch (`xcodebuild -checkFirstLaunchStatus` trả 69). Nếu script treo ở `devicectl`, chạy `sudo xcodebuild -runFirstLaunch` một lần (cần mật khẩu máy Mac) rồi chạy lại.

Bấm thử trên máy, theo thứ tự:

1. Mở app → vẫn đăng nhập (hoặc đăng nhập lại hoandat-auth); top bar, menu tài khoản, theme sáng/tối, không phần tử nào bị Dynamic Island / home indicator che.
2. Ghi âm → "Tải tệp lên" → picker mở; chọn **một video trong Ảnh** và **một tệp âm thanh trong Tệp** → thanh tiến độ chạy tới 100%, tệp hiện trong Lịch sử ghi, phát được, bấm "Chép lời" được.
3. "Tải tệp lên" → chọn **Quay video** → app xin quyền camera (không bị tắt), quay vài giây → tải lên được.
4. Cắt file: chọn tệp → tải lên 100% → cắt → tải kết quả về máy (share sheet → Lưu vào Tệp).
5. Lịch sử ghi: xoá một bản ghi → "Hoàn tác" → bản ghi quay lại. Xoá lại, **vuốt tắt app trong 5 giây** → mở lại → bản ghi không còn.
6. "Xoá tất cả" (dùng tài khoản/dữ liệu thử) → hỏi xác nhận đúng số tệp → danh sách trống, thẻ dùng lượng giảm.
7. Menu tài khoản → "Liên kết chat" → "Mở chat" → **Safari** mở chat.hoandat.com; xác nhận liên kết; quay lại app thấy "Đã liên kết". Gửi một biên bản sang chat.
8. Ghi 2–3 phút, khoá màn hình giữa chừng, bấm "Đánh dấu" trước và sau khi khoá → mở bản chép lời, mốc nằm đúng giây.
9. Tìm kiếm một từ trong bản đã chép → bấm kết quả tua đúng chỗ; hỏi một câu → có trích mốc giây.
10. Đánh dấu từ màn hình khoá: bắt đầu ghi, khoá máy → màn hình khoá (và Control Center) có mục **"Scribe đang ghi"** với thời gian chạy. Bấm nút tạm dừng ở đó → tiêu đề đổi thành "Đã đánh dấu mm:ss" vài giây, bản ghi **vẫn chạy** (thời gian không dừng). Bấm hai lần thật nhanh → chỉ thêm một mốc. Không thấy mục này → thử đặt `playsSilenceWhileRecording` ngược lại để so; ghi lâu 10 phút xem pin và âm thu không có tạp.
11. ~~Bỏ qua theo chủ dự án 17/10~~ Đeo AirPods (micro vẫn là micro iPhone) khi đang ghi và khoá máy: bấm 1 lần, rồi bấm 2 lần → mỗi lần thêm một mốc, không dừng ghi. Thử thêm khi AirPods là micro nếu chọn được.
12. Mở khoá, mở app → nút "Đánh dấu" vẫn chạy như cũ; dừng ghi → mở bản chép lời: các mốc bấm từ màn hình khoá/AirPods nằm đúng giây. Thử thêm: bấm mốc khi khoá → vuốt tắt app → mở lại → mốc vẫn có.
13. Còn nợ từ trước: cuộc gọi đến khi đang ghi, AirPods, chế độ máy bay khi đang tải (cần máy thứ hai + AirPods).
14. **Chép trực tiếp** (cần bản backend có `REALTIME_EMAILS` chứa email chủ sản phẩm): bật công tắc "Chép trực tiếp" → ghi → panel "Đang chép" hiện câu sau mỗi lần ngừng nói, mốc đúng đồng hồ ghi. Tài khoản khác (ngoài danh sách) → panel báo chưa được dùng, ghi vẫn chạy. Tắt công tắc → ghi bình thường, log backend không có `realtime.token`.
15. Gate 0 bước 2 của Phase 5: bật chép trực tiếp, ghi **30 phút khoá máy liên tục** → mở lại thấy đủ câu nói trong lúc khoá; đếm số dòng "mất đoạn này" (số lần rớt kết nối); ghi % pin ở *Cài đặt → Pin*. Sau đó ghi 60 phút để đo Success Criteria (≥ 95% câu, pin ≤ 15%).
16. Giữa phiên bật **chế độ máy bay 2 phút** → panel "Mất kết nối từ mm:ss", tắt máy bay → tự nối lại, có một dòng "mất đoạn này"; file m4a nguyên vẹn, Lịch sử ghi đủ đoạn.
17. Ghi quá 10 phút có chép trực tiếp → qua mốc đổi token (mỗi ~10 phút) không mất câu; `admin usage` thấy phút tăng xấp xỉ thời lượng stream (không gấp đôi).

## Success Criteria (toàn plan)

- [ ] Ghi 60 phút khoá màn hình liên tục, **không sạc** → mọi đoạn nguyên vẹn; pin tụt **≤ 12%**, đo qua *Cài đặt → Pin → theo app* (không dùng `batteryLevel`, Phase 0 đã chứng minh nó dưới ngưỡng phân giải)
- [ ] Cuộc gọi đến giữa phiên → tự phục hồi, đoạn trước và sau đều đọc được
- [ ] Chế độ máy bay giữa phiên → đoạn giữ trên đĩa, tự upload lại khi có mạng
- [ ] Kill app giữa phiên → mở lại upload nốt; đoạn đang mở được **báo cáo là mất**, không im lặng bỏ qua
- [ ] Offline **> 24 giờ** rồi mới có mạng → đoạn tồn vẫn upload được (session server đã bị sweep)
- [ ] Tab **Cắt file** đầy đủ trên iPhone (chọn file → upload → cắt → tải kết quả **về máy**)
- [ ] **Lịch sử ghi** phát lại + tua + xoá; hiển thị **ngày hết hạn 14 ngày**; **export ra Files** được (V2)
- [ ] Đoạn cùng một phiên **gom thành một dòng gấp/mở** trong Lịch sử ghi — họp 2 tiếng không đẻ ra 120 dòng rời (V3)
- [ ] Đăng xuất → hàng đợi upload dừng, cookie sạch ở cả hai kho, đoạn tồn không upload vào tài khoản cũ
- [ ] `git diff --stat server/` **trống** — hoặc, nếu Gate 0 fail, diff chỉ gồm `Cargo.toml` (feature `cors`) + `routes.rs` (`CorsLayer`), và điều đó được nêu rõ trong báo cáo Phase 1 (V1)
- [ ] `npm run lint && npm run typecheck && npm test && npm run build` trong `web/` EXIT 0; `xcodebuild build` EXIT 0, 0 warning
- [ ] Web **không hồi quy**: dev local (`npm run dev` + BE local, Mongo dev 176) ghi thử trên Chrome desktop vẫn chạy
- [ ] Một lệnh deploy lại được sau khi profile hết hạn 7 ngày

## Rủi ro chính

| Rủi ro | Mức | Giảm thiểu |
|---|---|---|
| `CapacitorHttp` không phủ hết (redirect, streaming, header lạ) | **Cao** | Gate bước 0 Phase 1 — đo trước mọi thứ khác. Fallback đã chốt sẵn (D3), không stall chờ quyết định |
| **Allowlist mở** — mọi tài khoản Google verified vẫn dùng được API, trong khi iOS thêm một thiết bị giữ credential 12h không revoke được, chạy nền | **Đã chấp nhận** | Quyết định của chủ sở hữu (V4). Phòng thủ dựa vào quota 5GB + rate-limit real-IP + audit log, đúng như backlog P3 của `docs/project-roadmap.md`. Ghi lại để lần rà App Store (Phase 4) không bỏ sót |
| Firebase iOS SDK vướng Keychain (free team thiếu Keychain Sharing) | Trung bình | Gate bước 2 Phase 1; lối lui `signInWithIdp` đã xác định cụ thể (D5.3), không còn mơ hồ như bản 1 |
| Phát media phải tải trọn file → file Mode B 2GB | Trung bình | Hiển thị tiến độ; ghi nhận hạn chế; ưu tiên phát đoạn ghi (3.73 MB) |
| Đoạn đang mở mất khi OS kill | Chấp nhận | Đoạn 1 phút → bán kính 1 phút; báo cáo rõ thay vì im lặng |
| Profile hết hạn 7 ngày giữa lúc đang dùng | Trung bình | Phase 4: script deploy 1 lệnh in ngày hết hạn |
| **Không có TestFlight** — TestFlight bắt buộc tài khoản Apple Developer trả phí ($99/năm); chủ sở hữu không đăng ký (2026-10-15) | Đã chấp nhận | Cài qua Xcode bằng Personal Team như Phase 0; cài lại mỗi 7 ngày bằng script Phase 4 |
| Realtime gửi audio ra OpenAI, tốn tiền theo phút | Trung bình | Phase 5: công tắc bật/tắt, allowlist email, trần phút ở backend |
| `RecordView.vue` là file dùng chung → hồi quy web | Trung bình | Mọi nhánh sau `isNative`; tiêu chí hồi quy chạy được (dev server, không phải `file://`) |

## Dependencies

- Không có plan nào chặn plan này.
- `plans/260714-1453-media-toolkit-wave1` (pending, web) chạm cùng backend nhưng khác file. Không đặt `blockedBy`/`blocks`. Ghi chú: plan đó tồn tại một phần vì *"bản ghi ra `.webm` khó dùng ngoài app"* — bản ghi từ iOS là m4a, dùng được ngay.

## Open questions

Không còn. Ba giả định kỹ thuật còn lại là **gate đo được ở bước 0–2 của Phase 1** (CapacitorHttp gọi được BE không · Firebase có vướng Keychain không · BE có nhận m4a không), và mỗi gate đều có đường đi tiếp đã chốt sẵn nên không phase nào phải dừng chờ quyết định.

---

## Validation Log

### Session 1 — 2026-07-28
**Câu hỏi:** 4 · **Quyết định chốt:** 4 · **Verification pass:** bỏ qua theo guard của workflow (mục `## Red Team Review` đã có bằng chứng `file:line`); không còn tag `[UNVERIFIED]`.

| # | Quyết định | Chọn | Lý do & hệ quả |
|---|---|---|---|
| **V1** | Đường đi nếu Gate 0 (CapacitorHttp) fail | **CORS + Bearer, giữ `SameSite=Lax`** | Chốt trước để Phase 1 không dừng giữa chừng chờ quyết định. Không hạ SameSite vì BE không có CSRF token — hạ là bỏ lá chắn duy nhất của web đang chạy. Phá cam kết zero-code-change nhưng phạm vi hẹp, additive. → D3, D4, Phase 1 |
| **V2** | Bản ghi mất sau 14 ngày (server) khi bản cục bộ đã xoá | **Thêm nút "Lưu vào Files"** | Giữ luồng xoá-sau-upload (máy không đầy), hiện ngày hết hạn, người dùng tự cứu bản đáng giữ. Zero BE change — không đụng `RETENTION_DAYS`. → Phase 1 |
| **V3** | Độ dài đoạn mặc định | **1 phút + gom nhóm theo phiên trong UI** | Giữ bán kính mất 1 phút khi OS kill, đồng thời không đẻ ra 120 dòng cho họp 2 tiếng. Khả thi rẻ vì tên đoạn đã tất định `<sessionId>#<index>` (Phase 2). → Phase 2, Phase 3 |
| **V4** | Allowlist backend đang mở | **Giữ mở** — *ngược khuyến nghị của plan* | Quyết định của chủ sở hữu. Ghi nhận rủi ro đã chấp nhận: mọi tài khoản Google verified vẫn tạo được session 5GB, trong khi iOS thêm một thiết bị giữ credential 12h không revoke được chạy nền. Phòng thủ giữ nguyên: quota + rate-limit real-IP + audit log. Bỏ khỏi điều kiện tiên quyết Phase 1. → D4, bảng rủi ro, Phase 4 (mục rà App Store) |

### Whole-Plan Consistency Sweep
- Files reread: `plan.md`, `phase-01-vo-capacitor-va-auth.md`, `phase-02-recorder-va-upload.md`, `phase-03-ui-parity-tab-ghi-am.md`, `phase-04-deploy-va-van-hanh.md`
- Decision deltas checked: 4 (V1 fallback chốt sẵn · V2 export Files · V3 gom nhóm UI · V4 allowlist giữ mở)
- Reconciled stale references: 8 (D3 "hai phương án đã loại"→một loại một chốt · D4 hai dòng bảng · Goal 3 và 4 · Success Criteria +2 dòng · bảng rủi ro +1 dòng · Phase 1 bỏ điều kiện tiên quyết allowlist + thêm export · Phase 3 thêm gom nhóm · Success Criteria bỏ nhắc ALLOWED_EMAILS)
- Unresolved contradictions: **0**

---

## Red Team Review

### Session — 2026-07-28
**Findings:** 15 sau khi gộp trùng (26 thô từ 4 reviewer) — **15 accepted, 0 rejected**
**Severity:** 6 Critical, 7 High, 2 Medium
**Cấu hình:** 4 reviewer (Security Adversary/Fact Checker · Failure Mode Analyst/Flow Tracer · Assumption Destroyer/Scope Auditor · Scope & Complexity Critic/Contract Verifier), verification tier Full.

Ba trên bốn reviewer độc lập tìm ra cùng lỗ hổng transport/origin. Mọi finding Critical đã được verify lại bằng grep trực tiếp trước khi adjudicate.

| # | Finding | Severity | Áp vào |
|---|---|---|---|
| 1 | Origin `capacitor://localhost` + không CORS + `SameSite=Lax` + 25 URL tương đối → app không gọi được BE; phá cam kết zero-BE | Critical | D3, D4, Phase 1 bước 0 |
| 2 | `finalize` xoá session doc → mất response → 404 → trùng bản ghi hoặc mất | Critical | "Sự thật backend" #1, Phase 2 |
| 3 | `session.json` bị 2 luồng read-modify-write không khoá, không atomic | Critical | Phase 2 (SegmentStore = actor) |
| 4 | Session 12h + 0 route refresh + 401 xếp "vĩnh viễn" → hàng đợi chết sau một đêm | Critical | D5.1, Phase 2 |
| 5 | Không có `/api/recordings/status` → resume theo offset không implement được | Critical | D6 (bỏ resumable) |
| 6 | Logout chỉ xoá kho WebView; native upload tiếp 12h, BE không revoke | Critical | D5.2, Phase 3 |
| 7 | `use-recorder-native` không drop-in: `start()`/`stop()` lệch chữ ký, `useSplitter` (9 chỗ) bị bỏ quên | High | Phase 2 (hợp đồng interface) |
| 8 | Đoạn đang mở mất trắng khi OS kill — mâu thuẫn Goal 3 | High | D6, Goal 3, Phase 2 |
| 9 | Cả 2 lối lui auth bản 1 đều không chạy (sai `aud`; `capacitor://` không redirect được) | High | D5.3 |
| 10 | Quota chặn ở cả `chunk` và xoá `.part` → offset lệch → chết vĩnh viễn | High | "Sự thật backend" #2 |
| 11 | Retention 24h (session) + 14 ngày (file) không được nhắc | High | "Sự thật backend" #3–4 |
| 12 | Gate xác minh m4a nằm ở bước cuối Phase 3 cũ, sau toàn bộ chi phí chìm | High | Phase 1 bước 3 |
| 13 | Gate không đo được: pin không ngưỡng; test hồi quy web qua `file://` luôn fail | High | Success Criteria |
| 14 | YAGNI: `RecorderDelegate`, `ApiClient`, `ProvisioningInfo` banner, `UploadQueueStatus.vue`, resumable offset | Medium | D7, Phase 2/3/4 |
| 15 | Nhãn `"Sắp có (Phase 2)"` nhét số phase vào UI; rollback claim sai 3 chỗ; citation `Cargo.lock` sai đường dẫn | Medium | Phase 1/2/3, D4 |

### Whole-Plan Consistency Sweep
- Files reread: `plan.md`, `phase-01-vo-capacitor-va-auth.md`, `phase-02-recorder-va-upload.md`, `phase-03-ui-parity-tab-ghi-am.md`, `phase-04-deploy-va-van-hanh.md`
- Decision deltas checked: 9 (origin/transport · ranh giới BE change · bỏ resumable · đoạn 1 phút · SegmentStore actor · 401 re-auth · logout native · gộp phase 2+3 · xoá phase 6)
- Reconciled stale references: 14 (tái dùng UI 85%→70% · "zero BE change"→ranh giới D4 · 5 phút→1 phút · Cargo.lock path · bỏ nhãn "Sắp có" · bỏ `RecorderDelegate`/`ApiClient`/`ProvisioningInfo`/`UploadQueueStatus` · rollback claims · gate pin có ngưỡng · gate hồi quy web · đánh số phase 6→4)
- Unresolved contradictions: **0**

## Kết quả thử trên iPhone 17/10 11:43
- Mục 10: **đạt** — màn hình khoá hiện "Scribe đang ghi" (nhờ phát im lặng), bấm ở đó thêm mốc, mở app thấy "Đã đánh 1 mốc", bản ghi vẫn chạy.
- Sau đó sửa (chưa cài lên máy): khung hiện dạng trực tiếp thay cho thanh `--:--` trống, rút gọn dòng hướng dẫn bị cắt. Cài ở lần cắm sau.
- Mục 11 (AirPods): chủ dự án bỏ qua 17/10. Lệnh AirPods dùng cùng đường với nút trên màn hình khoá (đã đạt) nên không kiểm riêng.
