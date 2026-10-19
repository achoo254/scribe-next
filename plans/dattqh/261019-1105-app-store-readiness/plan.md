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

## Kiểm (19/10)
`cargo clippy -D warnings` sạch; `cargo test` 211 đạt (30 bỏ qua); test Mongo dev 22 đạt (tuần tự, bỏ `live_*` cần key
nhà cung cấp); `pnpm lint` 274 tệp sạch, `typecheck` sạch, `vitest` 381/381, `build` 0 cảnh báo, e2e 107 đạt / 7 bỏ qua;
Simulator `xcodebuild` BUILD SUCCEEDED, `PrivacyInfo.xcprivacy` có trong `App.app`.

## Còn lại (chủ dự án)
Khi Apple duyệt: Team ID, tạo app, key API, chạy `pnpm run ios:release`, TestFlight — `docs/ios-app.md`. Làm ngay
được: đặt `VITE_SUPPORT_EMAIL`, tạo tài khoản demo cho người duyệt, deploy backend + web.

## Câu hỏi mở
- Guideline 4.8: hoandat-auth chỉ có Google/GitHub — có thêm Sign in with Apple không nếu bị từ chối?
- Xoá tài khoản rồi đăng nhập lại tạo tài khoản mới với hạn mức free mới (sổ phút đã xoá).
- Thời gian giữ nhật ký truy cập (journald) trên `.65` chưa kiểm; chính sách không nêu số ngày.
