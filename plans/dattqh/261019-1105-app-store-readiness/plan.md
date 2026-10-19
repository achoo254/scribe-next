---
title: Sẵn sàng App Store — chính sách, xoá tài khoản, manifest, phát hành
status: completed
created: 2026-10-19
tier: L
---

# Sẵn sàng App Store

Chủ dự án đã đăng ký Apple Developer Program trả phí (chờ duyệt, chưa có Team ID). Mục tiêu: ngày Apple duyệt là
bắt đầu được TestFlight và nộp App Store.

## Kết quả mong muốn, ràng buộc, ngoài phạm vi
- Có trang `/privacy`, `/terms` công khai; xoá tài khoản trong app; manifest và Info.plist đạt yêu cầu; script tải
  build lên App Store Connect; hồ sơ App Store và ảnh chụp.
- Repo công khai: không key, không email/số điện thoại thật; không deploy, không đụng prod.
- Ngoài phạm vi: Sign in with Apple ở hoandat-auth, Live Activity, chạy script phát hành (cần Team ID).

## Hạng mục (đều xong 19/10)
| # | Hạng mục | Tier | Tệp chính |
|---|---|---|---|
| 1 | Trang `/privacy`, `/terms` (VI + tóm tắt EN), link ở trang đăng nhập, footer, menu tài khoản; khách xem trong `PublicLayout`, người đăng nhập xem trong shell | M | `web/src/pages/legal-page.tsx`, `i18n/messages/vi/legal.ts`, `layout/public-layout.tsx`, `lib/legal.ts` |
| 2 | Xoá tài khoản: `DELETE /api/me` + hộp "Xoá tài khoản" gõ XOÁ | L | `server/scribe-be/src/account.rs`, `web/src/components/delete-account-dialog.tsx` |
| 3 | `PrivacyInfo.xcprivacy`, `ITSAppUsesNonExemptEncryption`, mô tả quyền, chỉ iPhone, `arm64` | S | `web/ios/App/App/` |
| 4 | `web/scripts/ios-release.sh` (archive + `exportArchive` tải lên bằng key API) và hướng dẫn | S | `docs/ios-app.md` |
| 5 | Hồ sơ App Store VI/EN, độ tuổi, App Privacy, ghi chú duyệt | S | `docs/app-store/metadata.md` |
| 6 | Ảnh chụp 6.9"/6.5"/6.3" (5 màn, PNG RGB, 3,3 MB) | S | `web/e2e/app-store/`, `docs/app-store/screenshots/` |

## Quyết định
1. **Xoá gì / giữ gì** (chi tiết ở đầu `account.rs`): xoá mọi tệp và doc theo `userId`, sổ riêng, doc `users`. Sổ chi
   tiết `*_runs` giữ số tiền với `userId: "deleted"` (trần tổng và lệnh đối soát vẫn đúng). `audit_events` giữ tới
   TTL 180 ngày, bỏ `name`/`email`/`title`.
2. **Việc đang chạy**: từ chối khi đang ghi trên trình duyệt (409 `RECORDING_OPEN`); phiên bị thu hồi trước khi xoá;
   việc nền ghi muộn được dọn bởi vòng retention qua `deleted_users` (TTL 7 ngày).
3. **Xác nhận**: gõ "XOÁ" (so sau chuẩn hoá, cả server và web).
4. **Sau khi xoá**: không kết thúc phiên hoandat-auth (tài khoản hoandat còn); trang đăng nhập báo đã xoá.
5. **Chỉ iPhone** để không phải nộp ảnh iPad.
6. **Key API**: Admin (cloud signing bằng key cần quyền này theo Apple Developer Forums), lệch đề nghị App Manager.

## Sửa sau review (`plans/dattqh/reports/code-reviewer-261019-1150-account-deletion-review.md`)
- M1: `users.deletingAt` + `deleted_users` trước khi xoá; đăng nhập web/iPhone và làm mới phiên bị chặn (mã `deleting`)
  và làm tiếp việc xoá; phần còn lại chạy trong `tokio::spawn`, lỗi sau dấu xoá trả 200 `complete: false`.
- M2: `realtime_tokens` ẩn danh như sổ chi tiết (token chưa quyết toán vẫn vào đối soát).
- M3: xoá trong app gửi `discardUserKey` → Swift xoá đoạn, chữ chép trực tiếp, dấu mốc của tài khoản; xoá từ máy khác
  → `native/refresh` trả 410 `ACCOUNT_DELETED` (id thiết bị giữ trong `deleted_users`) → hàng đợi xoá đoạn. Thu hồi máy
  thường (401) vẫn giữ đoạn để tải nốt khi đăng nhập lại cùng tài khoản (hành vi đã kiểm trên máy thật 16/10). Khoá
  thư mục vẫn theo băm email (đổi sang id user cần đổi tên thư mục cũ; không cần khi đã xoá lúc xoá tài khoản).
- M4: app gọi `POST /api/me/delete`; `DELETE /api/me` giữ.
- L1 audit bỏ thêm `chatId`, `error`, `reason` của `admin.block`; L2 thu hồi IdP kiểm mã trạng thái, lỗi thì thử lại
  từ vòng retention; L3 bỏ phiên đã xoá khỏi cache; L4 index `userId` và tiền tố `_id` riêng từng sổ; L7 chính sách:
  nhà cung cấp chữ theo cấu hình, bản sao lưu, số liệu ẩn danh, quyền lưu vào Ảnh; L8 chỉ xoá tệp trong thư mục user.

## Kiểm (19/10)
`cargo clippy -D warnings` sạch; `cargo test` 211 đạt (30 bỏ qua); test Mongo dev 22 đạt (tuần tự, bỏ `live_*` cần key
nhà cung cấp); `pnpm lint` 274 tệp sạch, `typecheck` sạch, `vitest` 381/381, `build` 0 cảnh báo, e2e 107 đạt / 7 bỏ qua;
Simulator `xcodebuild` BUILD SUCCEEDED, `PrivacyInfo.xcprivacy` có trong `App.app`.

Sau sửa review (19/10 12:10): clippy sạch; `cargo test` 212 đạt / 30 bỏ qua; Mongo dev 22 đạt (gồm
`purge_user_on_real_mongo`: dấu xoá, token chép trực tiếp ẩn danh, audit bỏ `chatId`/`error`); lint, typecheck sạch;
vitest 381/381; build 0 cảnh báo; e2e 107 đạt / 7 bỏ qua (iPhone: `POST /api/me/delete`, `Recorder.signOut` có
`discardUserKey`); Simulator BUILD SUCCEEDED; self-test `-OSSelfTest` trên Simulator: `SELFTEST discard pending=0`. Chưa
chạy được trên iPhone/Simulator với backend thật: thân JSON của `POST /api/me/delete` qua CapacitorHttp (cùng đường POST
JSON các tính năng khác đang dùng trên prod).

## Còn lại (chủ dự án)
Khi Apple duyệt: Team ID, tạo app, key API, chạy `pnpm run ios:release`, TestFlight — `docs/ios-app.md`. Làm ngay
được: đặt `VITE_SUPPORT_EMAIL`, tạo tài khoản demo cho người duyệt, deploy backend + web.

## Câu hỏi mở
- Guideline 4.8: hoandat-auth chỉ có Google/GitHub — có thêm Sign in with Apple không nếu bị từ chối?
- Xoá tài khoản rồi đăng nhập lại tạo tài khoản mới với hạn mức free mới (sổ phút đã xoá).
- Thời gian giữ nhật ký truy cập (journald) trên `.65` chưa kiểm; chính sách không nêu số ngày.
- Chính sách hứa xoá bản sao lưu khi bản cập nhật ổn định: trên `.65` đang còn nhiều thư mục `/root/scribe-backup-*`
  từ 16–19/10 cần dọn.

## Deploy 19/10 12:11
Chủ dự án cho phép ("sửa xong hết thì deploy luôn"). Build sạch từ `47e74fe`, có `VITE_SUPPORT_EMAIL` (web/.env, không commit). Hàng đợi rỗng.
Sao lưu `/root/scribe-backup-20261019-121148`, binary sha256 `ec57158b8cfb168d…`, `index-BYJSOVBQ.js`, health ok, `NRestarts=0`.
`/privacy` 200, `/terms` 200, `POST /api/me/delete` chưa đăng nhập 401. Chưa thử xoá tài khoản thật trên prod và chưa thử từ iPhone thật.
