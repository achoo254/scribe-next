---
phase: 4
title: "hoandat-auth: client scribe-web + scribe-mobile"
status: completed
priority: P1
effort: "M"
dependencies: [3]
---

# Phase 4: hoandat-auth — client `scribe-web` + `scribe-mobile`

Repo `~/inet-gitlab/hoandat-auth`, làm theo mẫu client chat (`src/oidc/chat-clients.ts`, commit `0c0020d`, `4a9ca7d`). Vận hành IdP theo skill `hoandat-auth-ops-prod` của repo đó.

## Files (repo hoandat-auth)

- Create `src/oidc/scribe-clients.ts` — `scribe-web`: confidential `client_secret_basic`, Code + PKCE, scope `openid profile email`, redirect `${SCRIBE_WEB_URL}/api/auth/callback`, post-logout `${SCRIBE_WEB_URL}/`, back-channel logout `${SCRIBE_WEB_URL}/api/auth/backchannel-logout`. `scribe-mobile`: public native (`none`), redirect `SCRIBE_MOBILE_REDIRECT_URI` (mặc định `com.hoandat.scribe:/auth/callback`), refresh theo thiết bị như `chat-mobile`.
- Modify `src/oidc/clients.ts`, `src/config/env.ts` (`SCRIBE_WEB_URL`, `SCRIBE_WEB_CLIENT_SECRET`, `SCRIBE_MOBILE_REDIRECT_URI`), `src/tenants/env-tenant.ts`, `src/oidc/client-seed.ts`. Chỉ đăng ký Scribe ở **tenant chính**; không đụng `app-client-secrets.ts` (secret dẫn xuất cho mọi tenant).
- Modify `src/app-directory/apps-catalog.ts` — Scribe trên `/apps` của tenant chính.
- Tests: bản sao các test của chat (`chat-clients-flow`, `client-contract`, `env`, `apps-page`) cho scribe.
- Docs: `docs/integration-guide.md` §2 bảng client, `docs/deployment-guide.md` (biến mới, kiểm từ ngoài), `.env.example`.

## Tasks

0. Kiểm IdP có nhận redirect loopback `http://127.0.0.1:<port>/api/auth/callback` cho client confidential không (oidc-provider bỏ qua cổng loopback cho native). Có ⟹ thêm client `scribe-web-dev` cho dev local; không ⟹ dev local dùng script QA tạo session trong DB dev.
1. Thêm client + env + test; chạy toàn bộ gate của repo IdP (`pnpm lint && pnpm typecheck && pnpm test`, e2e nếu repo yêu cầu).
2. Sinh `SCRIBE_WEB_CLIENT_SECRET` trên server (không in), đặt vào `.env` của IdP; `SCRIBE_WEB_URL=https://scribe.hoandat.com`.
3. Deploy IdP theo `infra/deploy.sh` (người dùng đồng ý riêng cho lần deploy IdP này — IdP dùng chung cho Localman, Tunelo, chat).
4. Kiểm: `curl …/auth?client_id=scribe-web&response_type=code&scope=openid` chuyển lỗi "PKCE required" về đúng redirect; client lạ không chuyển; `seed-clients.mjs` sạch; restart pm2 không tăng sau 15 phút; Localman/chat vẫn đăng nhập được.

## Risks

- IdP dùng chung: lỗi cấu hình làm hỏng đăng nhập app khác ⟹ deploy giờ thấp điểm, có rollback release của IdP.
- Domain `scribe.hoandat.com` chưa có ở bước này: redirect trỏ trước domain mới, chỉ dùng được sau Phase 7 (dev local dùng client riêng — xem Phase 5).

## Kết quả (2026-10-16 12:05)

- hoandat-auth commit `e1a7c96` (+ docs): `src/oidc/scribe-clients.ts`, env `SCRIBE_WEB_URL`, `SCRIBE_WEB_CLIENT_SECRET` (tuỳ chọn), `SCRIBE_MOBILE_REDIRECT_URI`, cặp tuỳ chọn `SCRIBE_WEB_DEV_URL` + `SCRIBE_WEB_DEV_CLIENT_SECRET`; chỉ tenant chính (test tenant khác không có client); `/apps` có thẻ Scribe khi có client web. `scribe-mobile` bật cả khi chỉ có client dev.
- Task 0: IdP nhận redirect loopback cho client web confidential, **cổng phải khớp chính xác** (client web không được nới cổng như native) — có test.
- Gate: lint, typecheck, 518 test, 10 e2e, check:tokens sạch. Deploy `infra/deploy.sh`: `/healthz` OK, Localman 200, chat-web/chat-mobile chuyển đúng redirect.
- `.env` IdP (sao lưu `.env.bak-<giờ>`): bật `scribe-web-dev` (`http://localhost:5174`); secret dev ở máy dev `~/.onescribe-dev/.oidc-web-dev-secret` (600). `scribe-web` chưa bật: bật ở 7b cùng site `scribe.hoandat.com` (DNS proxied đã có — người dùng cấu hình 16/10; hiện trả 421 vì chưa có vhost).
