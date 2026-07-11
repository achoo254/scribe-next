---
phase: 2
title: "Auth Google OAuth"
status: pending
priority: P1
effort: "2d"
dependencies: [1]
---

# Phase 2: Auth Google OAuth

## Overview

Xác thực **Google Sign-In (OAuth2 auth-code flow)** ở BE: verify `id_token` qua Google JWKS, upsert user vào MongoDB, cấp session cookie (JWT httpOnly SameSite). Middleware guard mọi `/api/*` (trừ auth). FE có trang login + guard route. Allowlist domain/email (compile-time, optional) chống lạm dụng.

## Requirements

- **Functional:** `/api/auth/login` → redirect Google; `/api/auth/callback` → verify → set cookie → redirect FE; `/api/me` trả user; logout xoá cookie. FE chưa login → về trang login.
- **Non-functional:** verify đủ `aud`/`iss`/`exp` + RS256 + `kid` match; cookie `HttpOnly`+`Secure`+`SameSite=Lax`; JWKS cache (TTL theo Cache-Control Google); allowlist enforce nếu cấu hình.

## Architecture

**BE crates (research):** `oauth2` (auth-code flow), `jsonwebtoken` (RS256 verify), `reqwest` (rustls, fetch JWKS). Secret (`GOOGLE_CLIENT_ID/SECRET`) nhúng compile-time (`env!`).

**Flow:**
1. FE bấm "Đăng nhập Google" → `GET /api/auth/login` → BE tạo authorize_url (state chống CSRF, lưu state ngắn hạn) → 302 tới Google.
2. Google → `GET /api/auth/callback?code&state` → verify state → exchange code → nhận `id_token`.
3. Verify `id_token`: `decode_header` lấy `kid` → fetch JWKS `https://www.googleapis.com/oauth2/v3/certs` (cache) → `DecodingKey::from_rsa_components(n,e)` → `Validation` RS256 + set_audience(client_id) + set_issuer(accounts.google.com) + leeway 60s.
4. **Allowlist** (nếu cấu hình `ALLOWED_DOMAINS`/`ALLOWED_EMAILS` compile-time): reject nếu email/domain không khớp → trang "không có quyền".
5. Upsert `users{googleSub, email, createdAt}` (Mongo). Cấp JWT session (sub=userId) → set cookie → 302 về FE.
6. Middleware `auth`: đọc cookie JWT → verify (HS256, JWT_SECRET compile-time) → gắn `userId` vào request extension. Thiếu/sai → 401. Áp cho mọi `/api/*` trừ `/api/auth/*` + `/api/health`.

**FE (`web/`):** trang login (ViUI, nút Google) + router guard (gọi `/api/me`, 401 → login). Store user. Cùng origin (nginx) → cookie same-site, không CORS.

## Related Code Files

- Create: `server/onescribe-be/src/auth.rs` (oauth + jwks verify + middleware), `server/onescribe-be/src/models/user.rs`, `web/src/views/LoginView.vue`, `web/src/router.ts` (guard), `web/src/composables/use-auth.ts`
- Modify: `server/onescribe-be/src/routes.rs` (mount auth routes + guard), `server/onescribe-be/src/config.rs` (thêm OAuth + JWT + allowlist), `server/onescribe-be/build.rs` (bake vars), `web/src/App.vue`

## Implementation Steps

1. `config.rs`: `env!` cho `GOOGLE_CLIENT_ID/SECRET`, `JWT_SECRET`, `OAUTH_REDIRECT_URL`, `option_env!` cho `ALLOWED_DOMAINS/EMAILS`.
2. `auth.rs`: oauth2 client + `/api/auth/login` (authorize_url + state store) + `/api/auth/callback` (exchange + verify id_token + JWKS cache + allowlist + upsert user + set cookie).
3. JWKS: struct JwksResponse{keys[{kid,n,e,alg}]}; fetch reqwest rustls; cache in-process (TTL theo header); verify RS256 đủ aud/iss/exp/kid.
4. Middleware guard + `/api/me` + `/api/auth/logout` (clear cookie).
5. FE: LoginView + router guard + use-auth (fetch /api/me).
6. E2E: login thật bằng 1 tài khoản Google → cookie set → /api/me trả email; xoá cookie → 401; allowlist chặn email ngoài (nếu bật).

## Success Criteria

- [ ] Login Google thật → cookie httpOnly set → `/api/me` trả `{email}`.
- [ ] Middleware chặn `/api/*` khi chưa auth (401); logout → 401.
- [ ] id_token verify đủ aud/iss/exp/kid + RS256; sai/hết hạn → reject.
- [ ] Allowlist (nếu bật) chặn email/domain ngoài danh sách.
- [ ] User upsert đúng vào Mongo `users` (không tạo trùng theo googleSub).

## Risk Assessment

- **JWKS key rotation → kid mismatch:** luôn cho phép refetch JWKS khi kid không thấy trong cache (không hard-fail).
- **state CSRF/replay:** state ngẫu nhiên + TTL ngắn + one-time.
- **Cookie qua CF proxied:** `Secure` cookie cần HTTPS (CF cấp) — verify set-cookie tới browser qua CF không bị strip.
- **"Mọi tài khoản Google" → lạm dụng:** allowlist compile-time là hàng rào chính; ràng buộc quota ở Phase 5. Đổi allowlist = rebuild (hệ quả env compile-time).
- **oauth2 crate API đổi theo version:** chốt version + verify API thật lúc cook (research snippet có thể lệch minor).
