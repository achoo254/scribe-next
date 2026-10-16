---
title: "Bước 2: key server + đo phút, rồi đăng nhập hoandat-auth"
description: "Bỏ key người dùng tự nhập: key OpenAI của chủ sản phẩm trên server, đo phút theo user, quota tháng, trần chi phí; sau đó chuyển đăng nhập sang hoandat-auth, đổi domain và database, mở chép lời cho mọi người."
status: pending
priority: P1
effort: "XL (A: M+S+S, B: M+L+M+M+S)"
branch: main
tags: [backend, frontend, ios, auth, billing, infra, database]
blockedBy: []
blocks: []
created: 2026-10-16
---

# Bước 2: key server + đo phút, rồi đăng nhập hoandat-auth

## Overview

Hiện người dùng tự dán key OpenAI, đăng nhập bằng Firebase Google, chép lời chỉ cho danh sách email nhúng lúc build. Đích (đã duyệt, `../reports/brainstorm-261016-0809-scribe-target-architecture-refactor.md` §6): key server + đo phút + trần chi phí (D2=B), quota phút, chưa bán (D3=A), đăng nhập hoandat-auth (D1=B), domain `scribe.hoandat.com` và đổi tên database cùng đợt auth.

**Thứ tự (Jev 2026-10-16, `jev-1.13.0`, 0,74):** phần A trước, giữ allowlist `TRANSCRIBE_EMAILS`; phần B sau và chỉ mở chép lời cho mọi người khi B xong.

## Phases

| # | Phase | Tier | Status | Phụ thuộc |
|---|-------|------|--------|-----------|
| **A** | **Key server + đo phút** | | | |
| 1 | [Backend: key server, sổ phút, quota, trần, CLI admin](./phase-01-backend-server-key-metering.md) | M | Pending | — |
| 2 | [Web/iPhone: bỏ nhập key, hiện quota](./phase-02-ui-quota-no-key.md) | S | Pending | 1 |
| 3 | [Deploy phần A](./phase-03-deploy-part-a.md) | S | Pending | 1, 2 |
| **B** | **Đăng nhập hoandat-auth + hạ tầng mới** | | | |
| 4 | [hoandat-auth: client `scribe-web` + `scribe-mobile`](./phase-04-idp-scribe-clients.md) | M | Pending | 3 |
| 5 | [Backend: OIDC, phiên thu hồi được, chuyển user](./phase-05-backend-oidc-sessions.md) | L | Pending | 4 |
| 6 | [Web + iPhone: đăng nhập qua IdP, bỏ Firebase](./phase-06-clients-oidc-login.md) | M | Pending | 5 |
| 7 | [Hạ tầng: 7a DB + service, 7b đăng nhập + domain, 7c mở chép lời](./phase-07-infra-cutover.md) | M | Pending | 6 (7a chỉ cần 3) |
| 8 | [QA iPhone thật + docs](./phase-08-qa-docs.md) | S | Pending | 7 |
| 9 | [Chống lạm dụng gói free (bắt buộc trước 7c)](./phase-09-free-abuse-guards.md) | M | Pending | 5 |

## Thiết kế chính

**Phần A — tiền là của chủ sản phẩm, nên mỗi giây phải được giữ chỗ trước khi chi.**

```
POST /api/transcripts ─► allowlist? ─► cần = Σ durationS các file
        │                                    │
        │            sổ phút tháng (Asia/Ho_Chi_Minh, "YYYY-MM")
        │            user:  $inc có điều kiện  used + cần ≤ hạn mức gói
        │            global: $inc có điều kiện used + cần ≤ trần
        │            (một trong hai fail ⟹ hoàn phần đã giữ, 403 kèm số)
        ▼
 transcripts.chargedS = giây đã giữ ──► worker dùng key server
        lỗi / huỷ ⟹ hoàn đúng chargedS ; xong ⟹ giữ nguyên
```

- Key OpenAI ở tệp `OPENAI_KEY_PATH` (600, chủ là user chạy BE), đọc lúc khởi động; không nằm trong repo, binary hay argv. Bỏ `PendingKeys`: restart không còn làm hỏng lượt chép (lượt dở quay lại `queued`).
- Gói: `free` 120 phút (mặc định), `plus` 3.000 phút, cấp bằng `scribe-be admin set-plan` chạy trên server, có hạn, ghi `audit_events`. Trần: **quỹ free $10/tháng** (chỉ chặn user free) + **trần tổng $30/tháng** (chặn mọi người) — người dùng chốt 2026-10-16. Đều là biến build.
- Mongo prod standalone (không transaction): giữ chỗ/hoàn dựa trên thao tác nguyên tử một doc, hoàn gắn `runId`, có lệnh đối soát.

**Phần B — làm theo mẫu chat-web đã chạy thật; luồng native tự làm (chat-next mới có thiết kế).**

- Web: backend là BFF, client confidential `scribe-web`, Code + PKCE phía server, cookie phiên HttpOnly; không token IdP nào tới JS.
- iPhone: client public `scribe-mobile`, `ASWebAuthenticationSession`, redirect `com.hoandat.scribe:/auth/callback`; app gửi `code` + `code_verifier` cho backend đổi lấy phiên. Refresh token IdP (theo thiết bị) giữ trong Keychain để hàng đợi tải lên tự làm mới phiên như hiện nay.
- Backend tin IdP bằng cách xác minh ID token ES256 qua JWKS một lần lúc đăng nhập rồi cấp phiên riêng (không cần resource audience).
- Phiên lưu trong Mongo (`sessions`, `_id` = băm của token) để thu hồi được; web nhận back-channel logout; mobile mất phiên khi máy bị thu hồi ở `/account` (refresh IdP giữ phía server, `invalid_grant`).
- Bỏ hẳn Firebase, không chạy song song (người dùng chốt) ⟹ đổi đăng nhập một chiều, thử đủ trên dev local + iOS simulator trước.
- Mở chép lời cho mọi người bằng cờ `TRANSCRIBE_OPEN`, bật vài ngày sau khi đổi đăng nhập ổn (Jev 0,98) **và chỉ sau Phase 9** (chống lạm dụng free: giới hạn ngày, theo IP, tài khoản mới chờ 24 giờ, lệnh admin chặn, báo Telegram — người dùng chốt).
- User cũ ghép theo email khi IdP báo `email_verified` (đã duyệt D1: "migrate user theo email"); sau đó khoá theo `idpSub`.

## Non-goals

Bán gói / thanh toán; trang admin web (CLI là đủ); Sign in with Apple; tóm tắt AI; realtime; đổi model chép lời; cảnh báo Telegram (để sau, chỉ log + audit khi chạm 80%/100% trần).

## Success Criteria

- [ ] Không còn ô nhập key ở web và iPhone; chép lời dùng key server.
- [ ] Hai yêu cầu chép đồng thời không bao giờ vượt hạn mức gói hay trần toàn cục (test đua trên Mongo thật).
- [ ] Lượt lỗi hoặc bị huỷ được hoàn đúng số giây đã giữ; chép lại tính phí lần nữa.
- [ ] Restart backend khi đang chép: lượt chạy tiếp, không bắt người dùng bấm lại, không giữ phí hai lần.
- [ ] `/api/me` trả phút đã dùng / hạn mức / ngày reset; UI hiện "Còn N phút tháng này" và báo rõ khi hết.
- [ ] `scribe-be admin set-plan <email> plus --until YYYY-MM-DD` chạy trên server, có audit.
- [ ] Đăng nhập web và iPhone qua `auth.hoandat.com`; user cũ giữ nguyên bản ghi và transcript sau lần đăng nhập đầu.
- [ ] Đăng xuất ở IdP (back-channel) làm phiên web mất hiệu lực ngay; thu hồi máy ở `/account` làm app iPhone mất phiên ở lần làm mới kế tiếp.
- [ ] Prod chạy ở `scribe.hoandat.com`, database đổi tên có sao lưu, mọi người đăng nhập được chép lời trong hạn mức.
- [ ] `cargo clippy -D warnings`, `cargo test --workspace`, `npm run lint/typecheck/check:tokens/test` sạch ở cả hai repo; CI xanh.

## Rủi ro chính

| Rủi ro | Giảm thiểu |
|---|---|
| Vượt chi phí do đua hoặc lỗi giữ chỗ | `$inc` có điều kiện trên doc tháng; trần toàn cục là chốt cuối; test đua trên Mongo thật |
| Lộ key OpenAI | Tệp 600 ngoài web root, không argv/log/audit; lỗi OpenAI 401 báo "key máy chủ hỏng" cho admin chứ không hiện key |
| Luồng native chưa có mẫu chạy thật | Phase 6 làm plugin Swift nhỏ + test trên iPhone thật trước khi đổi prod |
| User cũ mất dữ liệu khi đổi đăng nhập | Ghép theo email đã xác minh; giữ `firebaseUid` tới khi xong; test với bản sao DB prod |
| Đổi domain + DB làm gián đoạn | Một cửa sổ deploy, sao lưu trước, giữ domain cũ chuyển hướng 301 một thời gian, rollback có lệnh sẵn |
| App iPhone đang cài trỏ domain cũ | Cài bản mới trước khi tắt domain cũ |

## Red Team Review

### Session — 2026-10-16
15 phát hiện (1 Critical, 7 High, 5 Medium, 2 Low), đều chấp nhận, đã áp: hoàn phút gắn `runId` + `find_one_and_update` (Critical, Phase 1); sửa `files.path` khi dời thư mục (Phase 7a); giữ thêm khi probe vượt, trần lần thử sau restart, `sentS` cộng trước khi gửi (Phase 1); tách quỹ free/tổng + cờ mở chép lời (Phase 1, 7c); từ chối email chưa xác minh, ghép nguyên tử (Phase 5); tiêu chí đăng xuất mobile (plan); refresh IdP giữ phía server (Phase 5–6); `redirect_uri` cố định, `nonce`, `aud` (Phase 5); `auth_states` trong Mongo, `_id` băm, kiểm logout token đủ (Phase 5); nhánh `admin` tách sớm (Phase 1); tách Phase 7 thành 7a/7b/7c; task kiểm loopback, tenant chính (Phase 4); thông báo key hỏng + cờ ngắt (Phase 1); lệnh đối soát sổ (Phase 1).

## Validation Log

### Session 1 — 2026-10-16
| Câu hỏi | Jev / người dùng | Quyết định |
|---|---|---|
| Quota free / plus / trần | Jev 120 phút, gói lớn hơn, $10 (độ tin cậy thấp) | Người dùng: "phần A ok" ⟹ 120 / 3.000 |
| Cấu trúc trần | Jev tách quỹ 0,54 | Người dùng chọn quỹ free $10 + tổng $30 |
| Mở chép lời | Jev cờ riêng, sau vài ngày 0,98 | Áp dụng (7c) |
| Rollback đăng nhập | Jev giữ Firebase song song 0,62 | Người dùng: bỏ Firebase, không song song ⟹ một chiều |
| Mongo prod | đo: standalone 7.0.28 | Không transaction |
| Chống lạm dụng free trước 7c | Jev 0,72–0,79 từng biện pháp | Người dùng: bộ đầy đủ, giữ quỹ $10 ⟹ Phase 9 |
| iOS target | đo: 15.0 | Callback scheme riêng, ghi nhận rủi ro |
