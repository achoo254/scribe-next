---
phase: 2
title: "Auth Google (Firebase Authentication)"
status: done
priority: P1
effort: "1.5d"
dependencies: [1]
---

> **✅ DONE & VERIFIED LIVE 2026-07-11:** login Google thật thành công trên $DEPLOY_DOMAIN — user `quocdat254@gmail.com` upsert vào Mongo (`firebaseUid` + name + createdAt), `/api/me` trả email, audit-log ghi login. Toàn bộ Success Criteria pass.

# Phase 2: Auth Google qua Firebase Authentication

> **Progress 2026-07-11:** Code XONG + **DEPLOYED LIVE**. BE: `auth.rs` (POST `/api/auth/session` verify Firebase ID token → session cookie), `jwks.rs` (securetoken JWKS + cache + cooldown), `session.rs` (HS256 key runtime), `models/user.rs` (upsert + unique index `firebaseUid`), middleware guard `/api/me`. FE: `firebase.ts`, `use-auth.ts` (signInWithPopup→session), `router.ts` guard, LoginView/HomeView. Code-review 2 High đã fix.
>
> **Live verify trên .65 (`$DEPLOY_DOMAIN`):** Mongo user `$DB_NAME` connect OK (authSource=$DB_NAME), unique index `firebaseUid_1` tạo thật. `/api/me` chưa login → **401** (guard đúng). FE served. **CÒN LẠI = login Google thật (manual QA của user)** — cần verify authorized domain Firebase gồm `$DEPLOY_DOMAIN` (nếu popup báo `auth/unauthorized-domain` → thêm ở Console Authentication → Settings → Authorized domains).

## Overview

Xác thực **Google Sign-In qua Firebase Authentication**. FE dùng Firebase JS SDK (`signInWithPopup` + `GoogleAuthProvider`) → nhận **Firebase ID token**. FE gửi token lên BE → BE (Rust) **verify Firebase ID token** (RS256 qua Google `securetoken` JWKS), upsert user vào MongoDB, cấp **session cookie riêng của BE** (JWT httpOnly HS256, key runtime) — vì Firebase token nằm trong JS (không httpOnly), đổi ngay lấy cookie httpOnly để giữ chống-XSS. Middleware guard mọi `/api/*` (trừ auth). FE có trang login + guard route. Allowlist domain/email (optional) chống lạm dụng.

**Đổi hướng — 2026-07-11:** thay OAuth2 auth-code flow tự làm trong Rust (`oauth2` crate) bằng **Firebase Auth** (user chốt). Hệ quả tích cực: **bỏ `oauth2` crate** (red-team #11 → moot), bỏ `/api/auth/login`+`/callback` redirect + state/CSRF, **bỏ luôn 2 secret compile-time `GOOGLE_CLIENT_ID/SECRET`** (Firebase tự quản OAuth client). Giữ nguyên: MongoDB users, JWT session HS256 key runtime (red-team #4), check `email_verified` (red-team #9), httpOnly cookie, middleware guard, allowlist option.

## Firebase project (ĐÃ provision — 2026-07-11)

- Project ID: `$FIREBASE_PROJECT_ID` · Project number: `600080952715` · Owner: quocdat254@gmail.com
- Web App: "Scribe Web" — App ID `1:600080952715:web:41d846fba6732899d3538e`
- Web SDK config (public, KHÔNG phải secret): `authDomain=$FIREBASE_PROJECT_ID.firebaseapp.com`, `projectId=$FIREBASE_PROJECT_ID`, `messagingSenderId=600080952715`, `storageBucket=$FIREBASE_PROJECT_ID.firebasestorage.app`. **`apiKey` không lưu ở đây** — lấy qua MCP `firebase_get_sdk_config` hoặc đặt trong `web/.env` (`VITE_FIREBASE_API_KEY`).
- Google sign-in provider: **ĐÃ BẬT** (`firebase deploy --only auth`); authorized domains: `$DEPLOY_DOMAIN` + `localhost` (default). Support email consent: quocdat254@gmail.com.

## Requirements

- **Functional:** FE nút "Đăng nhập Google" → Firebase popup → nhận ID token → `POST /api/auth/session {idToken}` → BE verify + set cookie → redirect FE; `/api/me` trả user; logout xoá cookie BE + `signOut()` FE. Chưa login → trang login.
- **Non-functional:** verify Firebase ID token đủ `aud`==`$FIREBASE_PROJECT_ID` + `iss`==`https://securetoken.google.com/$FIREBASE_PROJECT_ID` + `exp` + RS256 + `kid` match (securetoken JWKS) + **`email_verified==true`**; session key tự sinh runtime (không bake) + token TTL ngắn; cookie `HttpOnly`+`Secure`+`SameSite=Lax`; JWKS cache (TTL theo Cache-Control); allowlist enforce nếu cấu hình (chỉ sau `email_verified`).

## Architecture

**FE (`web/`):** `firebase` JS SDK (`firebase/app`, `firebase/auth`). `initializeApp(config)` từ `VITE_FIREBASE_*` env (public). `signInWithPopup(auth, new GoogleAuthProvider())` → `user.getIdToken()` → POST BE. Router guard gọi `/api/me` (401 → login). Cùng origin (nginx) → cookie same-site, không CORS.

**BE crates:** `jsonwebtoken` (RS256 verify), `reqwest` (rustls, fetch JWKS `securetoken`), `serde`. **KHÔNG cần `oauth2` crate. KHÔNG bake `GOOGLE_CLIENT_ID/SECRET`** (Firebase quản OAuth client).

**Verify Firebase ID token (khác Google id_token OAuth thường ở JWKS + aud + iss):**
- JWKS/certs `securetoken`: x509 (canonical theo Firebase docs) `https://www.googleapis.com/robot/v1/metadata/x509/securetoken@system.gserviceaccount.com`; nếu muốn JWK `{kid,n,e}` cho `DecodingKey::from_rsa_components` → endpoint JWK `securetoken` — **verify endpoint chính xác + format ở cook**. Cache theo `Cache-Control`/`max-age`.
- `Validation` RS256 + `set_audience(["$FIREBASE_PROJECT_ID"])` + `set_issuer(["https://securetoken.google.com/$FIREBASE_PROJECT_ID"])` + leeway 60s. Claims: `sub` (Firebase UID), `email`, `email_verified`, `name`, `firebase.sign_in_provider`.
- **[Red-team #9] BẮT BUỘC check `claims.email_verified == true`** trước khi dùng email cho allowlist/phân quyền — reject nếu false.

**[Red-team #4] JWT session signing key — KHÔNG bake compile-time (GIỮ NGUYÊN):** BE tự sinh key ngẫu nhiên first-run ghi file runtime `chmod 600` (VD `$DEPLOY_APP_DIR/.session-key`, ngoài web root). Session token HS256 TTL ngắn + refresh. Không cần secret ngoài cho session → vẫn đúng "server sạch, không env".

**Flow:**
1. FE `signInWithPopup(GoogleAuthProvider)` → Firebase trả user + ID token (Firebase quản popup + OAuth client).
2. FE `POST /api/auth/session` body `{idToken}`.
3. BE verify Firebase ID token (securetoken JWKS cache, aud/iss/exp/kid, RS256) + `email_verified`.
4. **Allowlist** (nếu cấu hình `ALLOWED_DOMAINS`/`ALLOWED_EMAILS`): reject nếu không khớp → 403. Chỉ so khớp SAU `email_verified`.
5. Upsert `users{firebaseUid, email, name, createdAt}` (Mongo) — key theo `firebaseUid` (claim `sub`).
6. Cấp JWT session (sub=userId, HS256 key runtime) → set httpOnly cookie → 200.
7. Middleware `auth`: đọc cookie JWT → verify HS256 → gắn `userId` vào request extension. Thiếu/sai → 401. Áp mọi `/api/*` trừ `/api/auth/*` + `/api/health`.
8. Logout: BE xoá cookie + FE `signOut()`.

## Related Code Files

- Create: `server/scribe-be/src/auth.rs` (verify firebase token + session issue + middleware), `server/scribe-be/src/models/user.rs`, `web/src/lib/firebase.ts` (init SDK), `web/src/views/LoginView.vue`, `web/src/router.ts` (guard), `web/src/composables/use-auth.ts`, `web/.env` (VITE_FIREBASE_* public config)
- Modify: `server/scribe-be/src/routes.rs` (mount auth routes + guard), `server/scribe-be/src/config.rs` (bỏ GOOGLE_CLIENT_ID/SECRET; thêm `FIREBASE_PROJECT_ID` const + allowlist option), `server/scribe-be/build.rs` (bỏ bake OAuth secret), `web/src/App.vue`, `web/package.json` (thêm `firebase`)

## Implementation Steps

1. FE: `npm i firebase`; `web/src/lib/firebase.ts` = `initializeApp(config)` (config từ `VITE_FIREBASE_*`) + `getAuth`. `web/.env`: `VITE_FIREBASE_API_KEY`/`AUTH_DOMAIN`/`PROJECT_ID` (public — commit được, không phải secret).
2. FE: `LoginView` nút Google → `signInWithPopup` → `getIdToken()` → `POST /api/auth/session`; router guard gọi `/api/me` (401 → login); `use-auth` store user.
3. BE `config.rs`: `FIREBASE_PROJECT_ID = "$FIREBASE_PROJECT_ID"` (const/compile-time, KHÔNG secret). `option_env!` cho `ALLOWED_DOMAINS/EMAILS`. `JWT_SECRET` KHÔNG bake — sinh runtime (#4). **Bỏ `GOOGLE_CLIENT_ID/SECRET` + `OAUTH_REDIRECT_URL`.**
   **[Validate #2] Allowlist MẶC ĐỊNH TẮT** (user chốt mở mọi tài khoản Google) → để trống = cho phép mọi email đã `email_verified`; **audit-log mọi email đăng nhập**. Giữ code allowlist để bật nhanh nếu bị abuse (đổi = rebuild).
4. BE `auth.rs`: `POST /api/auth/session` (verify Firebase ID token + securetoken JWKS cache + email_verified + allowlist + upsert user + set cookie). Middleware guard + `/api/me` + `/api/auth/logout` (clear cookie).
5. JWKS: struct keys `{kid,n,e}` (hoặc parse x509); fetch reqwest rustls; cache in-process (TTL theo header); verify RS256 đủ aud/iss/exp/kid.
6. E2E: login thật 1 tài khoản Google (popup) → cookie httpOnly set → `/api/me` trả email; xoá cookie → 401; token sai/hết hạn → reject; `email_verified==false` → reject; allowlist chặn email ngoài (nếu bật).

## Success Criteria

- [ ] FE `signInWithPopup` Google → BE verify Firebase ID token → cookie httpOnly set → `/api/me` trả `{email}`.
- [ ] Middleware chặn `/api/*` khi chưa auth (401); logout → 401 + Firebase `signOut()`.
- [ ] Firebase ID token verify đủ `aud`($FIREBASE_PROJECT_ID)/`iss`(securetoken)/`exp`/`kid` + RS256; sai/hết hạn → reject.
- [ ] `email_verified==false` → reject.
- [ ] Allowlist (nếu bật) chặn email/domain ngoài danh sách.
- [ ] User upsert đúng Mongo `users` (không trùng theo `firebaseUid`).

## Risk Assessment

- **Firebase ID token trong JS (không httpOnly):** đổi ngay lấy session cookie httpOnly của BE sau sign-in; Firebase token TTL ~1h, chỉ dùng 1 lần để lập session BE → giảm bề mặt token lộ qua XSS.
- **JWKS `securetoken` rotation → kid mismatch:** luôn cho refetch JWKS khi kid không thấy trong cache (không hard-fail).
- **aud/iss sai project:** hardcode đúng `$FIREBASE_PROJECT_ID` + issuer securetoken; test reject token của project khác.
- **Phụ thuộc hosted Firebase/GCP:** auth phụ thuộc Firebase uptime (đánh đổi đã chấp nhận khi user chọn Firebase). Allowlist compile-time giữ như "công tắc khẩn".
- **"Mọi tài khoản Google" → lạm dụng (user chốt MỞ):** allowlist mặc định TẮT → phòng thủ abuse dồn sang quota 5GB + size cap 2GB + quota tăng dần + rate-limit (firewall CF #2) + ffmpeg timeout/version-pin + audit-log (Phase 5).
- **Cookie qua CF proxied:** `Secure` cookie cần HTTPS (CF cấp) — verify set-cookie tới browser qua CF không bị strip.
- **[Red-team #11 → MOOT]** oauth2 crate v5 typestate: không còn liên quan (đã bỏ `oauth2` crate).
