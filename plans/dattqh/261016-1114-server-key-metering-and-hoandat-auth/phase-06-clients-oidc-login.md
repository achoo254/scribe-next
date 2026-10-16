---
phase: 6
title: "Web + iPhone: đăng nhập qua IdP, bỏ Firebase"
status: completed
priority: P1
effort: "M"
dependencies: [5]
---

# Phase 6: Web + iPhone — đăng nhập qua IdP, bỏ Firebase

## Files

- Web: `pages/login-page.tsx` — nút "Đăng nhập" điều hướng `/api/auth/login`; `auth-context.tsx` bỏ Firebase, logout đi theo URL backend trả; xoá `lib/firebase.ts`, gói `firebase`, `VITE_FIREBASE_*`.
- iOS: Create `web/ios/App/App/HoandatAuthPlugin.swift` — `ASWebAuthenticationSession` (ephemeral = false để dùng phiên IdP sẵn có), PKCE S256 sinh trong Swift, `state` kiểm trong app, callback scheme `com.hoandat.scribe`; gọi `/api/auth/native/exchange` (kèm `nonce`); lưu **token thiết bị của Scribe** trong Keychain (token IdP ở lại server).
- iOS: Rewrite `SessionAuth.swift` — `refreshSession` gọi `/api/auth/native/refresh` với token thiết bị (Keychain) thay securetoken Firebase; một `actor` đơn luồng cho refresh (`UploadQueue.swift:220-227` hiện refresh song song); giữ cookie trong `HTTPCookieStorage`.
- iOS: `Info.plist` URL scheme `com.hoandat.scribe`; xoá `GoogleAuthPlugin.swift`, `GoogleService-Info.plist` (gitignore), header bundle id cũ, gói `firebase` của web.
- Web `lib/native.ts`: thay `GoogleAuth` bằng `HoandatAuth`.
- Docs: `docs/ios-app.md`.

## Môi trường thử iPhone

Chỉ có dev local và prod (không dựng preview). Trước cutover: web dev local + **iOS simulator** trỏ backend dev local (`VITE_NATIVE_API_ORIGIN=http://localhost:8787`, ATS cho phép localhost), đăng nhập thật qua `auth.hoandat.com` bằng client dev (Phase 4 task 0) hoặc script QA. iPhone thật thử ngay sau cutover 7b; đổi đăng nhập là một chiều nên rollback 7b = sửa tiến, không quay về Firebase.

## Verification

- `npm run lint/typecheck/check:tokens/test`; test mới cho login page và logout.
- Build iOS simulator; trên iPhone thật: đăng nhập, ghi khi khoá màn hình 15 phút với phiên bị ép hết hạn (backend TTL thử ngắn) để kiểm hàng đợi tự làm mới, ngắt mạng ngay sau khi refresh rồi bật lại, đăng xuất.
