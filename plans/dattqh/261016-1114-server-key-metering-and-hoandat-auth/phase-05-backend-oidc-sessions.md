---
phase: 5
title: "Backend: OIDC, phiên thu hồi được, chuyển user"
status: completed
priority: P1
effort: "L"
dependencies: [4]
---

# Phase 5: Backend — OIDC, phiên thu hồi được, chuyển user

## Goal

Backend đăng nhập người dùng qua `auth.hoandat.com` cho cả web (BFF) và iPhone (đổi code native), cấp phiên riêng lưu trong Mongo, thu hồi được; user cũ giữ dữ liệu.

## Files (`server/scribe-be/src/`)

- Create `oidc.rs` — discovery + JWKS (EC P-256 và RSA, chọn khoá theo `kid`, cache có cooldown như `jwks.rs`), xác minh ID token (`iss`, `aud`, `exp`, `nonce`, `alg ∈ {ES256, RS256}`), đổi code tại `token_endpoint` (web: `client_secret_basic`; native: public + `code_verifier`), refresh native.
- Create `models/session.rs` — `sessions {_id: sid nội bộ, userId, idpSid, client: web|mobile, createdAt, lastSeenAt, expiresAt}`, index TTL `expiresAt`, index `idpSid`.
- Rewrite `auth.rs`:
  - `GET /api/auth/login` → lưu `state`, `nonce`, `code_verifier` trong collection `auth_states` (TTL 10 phút, dùng một lần); cookie chỉ mang id ngẫu nhiên; 302 tới IdP.
  - `GET /api/auth/callback` → kiểm `state`, đổi code, xác minh ID token, ghép user, tạo session, cookie, 302 về `/`.
  - `POST /api/auth/native/exchange` `{code, codeVerifier, nonce}` → `redirect_uri` cố định từ config (không nhận từ client), kiểm `aud`/`azp` = `scribe-mobile` và `nonce`; tạo session mobile + **token thiết bị** của Scribe (ngẫu nhiên, lưu băm). Refresh token IdP **giữ phía server** trong session, mã hoá AES-GCM bằng khoá từ tệp `.session-key` (giữ tệp này); app không bao giờ thấy token IdP. `auth_limit` áp cho route.
  - `POST /api/auth/native/refresh` `{deviceToken}` → khoá theo session (cập nhật có điều kiện `refreshing`), làm mới tại IdP; `invalid_grant` (máy bị thu hồi ở `/account`) ⟹ xoá session, 401; mất mạng ⟹ giữ phiên cũ tới hết TTL.
  - `POST /api/auth/logout` → xoá session; web trả URL `end_session_endpoint` có `id_token_hint`.
  - `POST /api/auth/backchannel-logout` → xác minh logout token đủ: `alg=ES256`, `typ=logout+jwt`, `iss`, `aud=scribe-web`, `iat` ≤ 2 phút, có `events` đúng, không có `nonce`, `jti` chưa dùng (lưu TTL); chỉ xoá session `client=web` khớp `sid`.
  - `require_auth` → cookie `__Host-scribe` mang token phiên; `sessions._id = SHA-256(token)` (bản dump không lộ phiên); cache RAM 30 s, **xoá ngay** khi logout/back-channel trong cùng tiến trình.
  - TTL: phiên web 12 h trượt (gia hạn khi dùng, tối đa 7 ngày); phiên mobile 12 h, gia hạn chỉ qua `native/refresh` (có kiểm IdP).
  - **Bỏ hẳn Firebase** (người dùng chốt 2026-10-16, thay gợi ý giữ song song của Jev 0,62): xoá `POST /api/auth/session`, `jwks.rs`, `FIREBASE_PROJECT_ID`. Đổi đăng nhập là một chiều ⟹ phải thử đủ trên dev local trước khi đổi prod (Phase 6).
- Modify `models/user.rs` — `idpSub` (unique, sparse). `email_verified=false` ⟹ từ chối đăng nhập như hiện nay. `find_or_link`: có `idpSub` ⟹ dùng; không ⟹ `find_one_and_update({firebaseUid: {$exists: true}, idpSub: {$exists: false}, email: <khớp không phân biệt hoa thường>}, {$set: {idpSub}})` nguyên tử; không có ⟹ tạo mới. Ghi `audit_events` `auth.link`.
- Modify `config.rs` — cờ `TRANSCRIBE_OPEN` (mặc định tắt): bật thì mọi user đăng nhập được chép trong hạn mức gói; tắt thì chỉ `TRANSCRIBE_EMAILS`. Allowlist đăng nhập `ALLOWED_EMAILS/DOMAINS` giữ nguyên ý nghĩa (rỗng = mở).
- Modify `config.rs`, `build.rs` — `OIDC_ISSUER`, `OIDC_WEB_CLIENT_ID`, `OIDC_WEB_CLIENT_SECRET_PATH` (tệp 600, như key OpenAI), `OIDC_MOBILE_CLIENT_ID`, `PUBLIC_URL`; bỏ `FIREBASE_PROJECT_ID` và `jwks.rs` ở cuối phase 6 (khi client không còn gọi route cũ).
- Rate limit `auth_limit` áp cho các route auth mới.

## Dev local

Chỉ IdP prod có client. Phase 4 kiểm IdP có cho redirect `http://localhost:5173/api/auth/callback` (loopback) không: có ⟹ thêm client `scribe-web-dev` dùng cho dev local; không ⟹ dev local đăng nhập bằng script QA tạo session trực tiếp trong DB dev (như QA transcript GĐ1), không thêm đường đăng nhập giả vào binary.

## Verification

- Unit: xác minh ID token ES256/RS256 với khoá thử (sai `aud`/`iss`/`nonce`/hết hạn/alg `none` đều bị từ chối); `find_or_link` 3 nhánh; logout token sai `events` bị từ chối.
- Test Mongo thật: session thu hồi bằng back-channel hay logout ⟹ request kế tiếp 401 ngay; ghép user cũ theo email giữ nguyên `_id`; doc đã có `idpSub` không bị ghép lại; hai refresh đồng thời chỉ một lần gọi IdP.
- Rủi ro còn lại ghi nhận: scheme riêng `com.hoandat.scribe:` (iOS 15, chưa dùng được callback https của iOS 17.4) — một app khác trên cùng máy có thể đăng ký trùng scheme; `nonce` + `state` + PKCE do app sinh và kiểm, rate limit giảm nhưng không loại hết.
- HTTP dev local: toàn bộ route mới trả đúng mã; cookie `Secure`, `HttpOnly`, `SameSite=Lax`.
