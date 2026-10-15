---
phase: 4
title: "Deploy, vận hành, lộ trình App Store"
status: pending
priority: P2
effort: "S"
dependencies: [3]
---

# Phase 4: Deploy, vận hành, lộ trình App Store

## Overview

Biến build/cài thủ công thành **một lệnh**, xử lý thực tế phiền nhất của free provisioning (**profile hết hạn 7 ngày**), và ghi lại lộ trình App Store để các quyết định ở Phase 1–3 không vô tình chặn đường đó.

Phase 6 của bản 1 bị xoá: nó không tạo/sửa file nào, và bốn "bước kiểm" của nó chỉ là bốn lệnh grep đã được khẳng định ở phase khác. Nội dung có giá trị (danh sách chặn App Store) trở thành **sản phẩm tài liệu** của phase này.

## Requirements

- Một lệnh: build web → `cap sync` → `xcodebuild` → cài lên máy (USB hoặc Wi-Fi)
- Người vận hành biết ngày hết hạn profile **tại thời điểm deploy**
- Tài liệu đủ để dựng lại từ máy chưa cài gì
- Lộ trình App Store nằm trong `docs/project-roadmap.md`, không phải trong plan

## Architecture

Tham số đã biết chắc từ phiên setup thực tế:

| Thông số | Giá trị |
|---|---|
| Team ID | `AM5C55N339` (Personal Team, free provisioning) |
| Signing identity | `Apple Development: <Apple ID cá nhân> (F555GS5268)` — tra bằng `security find-identity -v -p codesigning` |
| Thiết bị | `00008130-000C5412012A001C` — iPhone 15 Pro Max, iOS 26.5.2 |
| Máy build | Mac mini M4, Xcode 26.6, iOS 26.5 SDK |
| Cài đặt | `devicectl` — chạy được cả USB lẫn `localNetwork` (Wi-Fi) |

**Cảnh báo hết hạn: in ở script, không phải banner trong app.** Bản 1 định parse `embedded.mobileprovision` trong Swift rồi hiện banner ở `AppTopBar.vue` — một component **dùng chung với web**, tức là thêm bề mặt hồi quy web để phục vụ iOS. Sai chỗ: người cần biết là người đang ngồi ở Mac mini chạy script, và script đã tồn tại. `security cms -D -i` một dòng, không Swift, không đụng file web.

## Related Code Files

- Create: `web/scripts/deploy-ios.sh`
- Modify: `docs/deployment-guide.md` — mục iOS bên cạnh phần deploy web
- Modify: `docs/codebase-summary.md` — thêm app thứ ba vào bảng tổng quan (hiện chỉ Desktop + Web)
- Modify: `docs/project-roadmap.md` — mục "OneScribe iOS" + lộ trình App Store (hiện `grep -i ios docs/project-roadmap.md` = 0 kết quả)

## Implementation Steps

1. `deploy-ios.sh`: `set -euo pipefail`, UDID + Team ID qua biến môi trường **có giá trị mặc định** (đổi máy hoặc trả $99 không phải sửa script), thông báo lỗi rõ ở từng bước, in ngày hết hạn profile ở cuối.
2. `docs/deployment-guide.md`. Ghi cả những thứ đã trả giá trong phiên setup thật:
   - cần **Xcode đầy đủ**, không phải Command Line Tools;
   - phải chạy `xcodebuild -downloadPlatform iOS` — **SDK có sẵn không đủ**, vẫn báo `iOS 26.5 must be installed`;
   - phải bật Developer Mode trên iPhone và tin cậy profile.
3. `docs/codebase-summary.md` phản ánh 3 app.
4. `docs/project-roadmap.md`: mục iOS + lộ trình App Store (mục kế tiếp).
5. Chạy thử từ máy sạch: `git clone` → `deploy-ios.sh` → app lên máy.

## Lộ trình App Store (tài liệu, không thực thi)

### $99/năm mở khoá gì

| Hạng mục | Hiện tại | Sau khi trả phí |
|---|---|---|
| Provisioning | Hết hạn 7 ngày | 1 năm |
| Phân phối | Chỉ máy tự cắm | TestFlight + App Store |
| App Groups | ❌ | ✅ → mở đường ReplayKit ghi màn hình |
| Widget Extension | Vướng quota App ID | ✅ → Live Activity: đồng hồ + nút Dừng trên màn hình khoá |

### Bắt buộc để nộp

1. **Privacy Manifest** (`PrivacyInfo.xcprivacy`) — khai báo thu thập audio do người dùng tạo, không tracking.
2. **App Privacy** trên App Store Connect — "Audio Data" liên kết danh tính (có đăng nhập).
3. **Ghi chú cho App Review** giải thích `UIBackgroundModes: audio`. Thiếu ghi chú này là lý do reject phổ biến nhất cho background audio.
4. **Guideline 4.2** — app hybrid bị soi kỹ. Lập luận: có chức năng native thực chất (ghi nền, xử lý gián đoạn, hàng đợi offline). Quyết định D3 (bundle cục bộ, không `server.url` remote) **củng cố** lập luận này — một WebView trỏ URL remote đúng là hình dạng bị 4.2 từ chối.
5. **Guideline 5.1.1** — mô tả quyền micro phải cụ thể.
6. **Xoá tài khoản trong app** — bắt buộc với app có đăng nhập. Hiện chưa có; `DELETE /api/files/:id` chỉ xoá từng file.
7. **Hạ tầng** — đây là chặn thật, không phải giấy tờ.

### Chặn thật, và nó không phải việc của iOS

- BE chạy môi trường **prod** trên box `.65` dùng chung; nginx `listen 80` với Cloudflare Flexible → **chặng CF↔origin là HTTP cleartext**, và origin vẫn reachable trực tiếp (không firewall được vì box dùng chung — `docs/project-roadmap.md` P3 backlog).
- **Allowlist đang mở** (`config.rs:68` trả `true` khi cả hai danh sách rỗng) — chủ sở hữu chọn giữ nguyên cho giai đoạn dùng nội bộ (Validation Log V4). Đây là chặn cứng nếu nộp App Store: không thể công khai một API mà bất kỳ tài khoản Google nào cũng tạo được 5GB storage. Phải khoá trước khi nộp. <!-- Updated: Validation Session 1 - V4 -->
- Session là **JWT stateless không revocation** (`session.rs:60-70`). Mất máy = không de-authorize được trong 12 giờ. Với app giữ audio cuộc họp, đây là vấn đề ship-blocker, không phải mục giấy tờ.
- Chưa có chính sách riêng tư công khai, chưa có điều khoản dịch vụ.
- **Server là cache 14 ngày, không phải kho lưu trữ** (`RETENTION_DAYS=14`). Sản phẩm public cần nói rõ điều này hoặc đổi nó.

### Thứ tự đề xuất nếu quyết định lên Store

```
trả $99 → profile 1 năm → Live Activity → TestFlight vài người thử
        → hạ tầng đủ điều kiện công khai (TLS đầu-cuối, revocation, chính sách, xoá tài khoản)
        → ReplayKit ghi màn hình (nếu còn muốn) → nộp
```

## Success Criteria

- [ ] `bash web/scripts/deploy-ios.sh` từ repo sạch → app chạy trên iPhone, không thao tác Xcode GUI nào
- [ ] Script chạy được khi iPhone chỉ có Wi-Fi (không cáp)
- [ ] Script in ngày hết hạn profile ở cuối
- [ ] Thất bại có thông báo rõ khi: chưa mở khoá máy, chưa tin cậy profile, sai team
- [ ] `docs/deployment-guide.md` đủ để dựng lại từ máy chưa cài gì
- [ ] `docs/codebase-summary.md` phản ánh đúng 3 app
- [ ] `docs/project-roadmap.md` có mục iOS + lộ trình App Store
- [ ] Rà xong 4 mục (kết quả ghi vào roadmap): `AudioRecorder` không import Capacitor · không có UDID/Team ID trong mã nguồn app (chỉ trong script) · chuỗi mô tả quyền đủ cụ thể cho 5.1.1 · không dùng API không public

## Risk Assessment

**Script khoá cứng vào máy hiện tại (Thấp).** UDID và Team ID là biến môi trường có mặc định, không phải hằng số nhúng.

**Tài liệu lệch thực tế (Trung bình).** Tiêu chí "dựng lại từ máy chưa cài gì" chống việc này. Lưu ý phụ thuộc chéo: nếu Phase 1 gitignore `GoogleService-Info.plist` thì tiêu chí này **bất khả thi** — vì vậy Phase 1 chốt là commit file đó, nhất quán với cách repo đã xử lý config Firebase web (`web/.env.staging:1-3`: "PUBLIC — KHÔNG phải secret").

**Rollback**: chỉ thêm script + docs, không đụng code app và không đụng file web nào (đã bỏ banner ở `AppTopBar.vue`).
