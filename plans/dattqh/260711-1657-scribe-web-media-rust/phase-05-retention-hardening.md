---
phase: 5
title: "Retention & hardening"
status: done
priority: P1
effort: "2d"
dependencies: [4]
---

# Phase 5: Retention & hardening

## Overview

Bảo vệ disk box .65 + chống lạm dụng: **auto-purge N ngày + quota/user**, rate-limit, size cap, magic-byte, allowlist enforce, security pass (ownership, cookie, secret). Áp lên mọi phase trước.

## Requirements

- **Functional:** file quá hạn N ngày bị xoá (cả disk lẫn Mongo); user vượt quota bị chặn upload; anonymous/cross-user bị chặn.
- **Non-functional:** purge chạy định kỳ không đè box; rate-limit theo real IP (CF XFF); không secret trong git.

## Architecture

**Retention (sweep job là NGUỒN DUY NHẤT — [Red-team #1] KHÔNG dùng TTL index native):** `files.expiresAt = now + N ngày`, index THƯỜNG (KHÔNG `expire_after`). **Lý do bỏ TTL native:** TTL monitor Mongo xoá doc out-of-band (~60s/lần) độc lập chu kỳ sweep → sweep query `expiresAt<now` sẽ KHÔNG còn thấy doc → file disk tương ứng mồ côi vĩnh viễn (đúng bug orphan mà plan định né). **Sweep job** (tokio interval trong BE hoặc systemd timer) là tiến trình DUY NHẤT xoá: quét doc quá hạn → **xoá file disk (recording/upload/outputs/zip) TRƯỚC → rồi xoá doc** (thứ tự cố định, không đua). Sweep cũng dọn **orphan** (file disk không doc + doc không file) + `.part` mồ côi. **N = 14 ngày** (chốt validate 2026-07-11), cấu hình compile-time. <!-- Updated: Validation Session 1 - N=14 ngày -->

**Quota/user:** tổng `sum(files.size)` theo `userId`. **[Red-team #3] enforce TĂNG DẦN khi append chunk** (không chỉ finalize) — cộng dồn byte đã ghi của session vào usage tạm, abort 413 khi vượt `QUOTA_BYTES = 5GB` (chốt validate); tính atomic/khoá theo user để tránh race 2 upload đồng thời. Kèm `MAX_FILE_SIZE_BYTES = 2GB` (Phase 3 #14). Hiển thị usage ở FE. <!-- Updated: Validation Session 1 - quota 5GB, size cap 2GB -->

**Rate-limit + caps:** giới hạn req/phút cho `/api/upload` + `/api/auth` (tower `governor` hoặc middleware custom) theo **real IP**. **[Red-team #2] Trust boundary phải enforce ở tầng dưới, không chỉ đọc header:** header `CF-Connecting-IP` chỉ tin được khi nginx đã `set_real_ip_from <dải CF>; real_ip_header CF-Connecting-IP;` VÀ firewall .65 chặn :80 ngoài dải CF (cấu hình ở Phase 1) — nếu không, attacker connect thẳng origin IP tự set header → bypass. Middleware Rust đọc real IP đã được nginx chuẩn hoá (`$remote_addr`), KHÔNG đọc trực tiếp header client. Size cap/file (đã Phase 3). **Magic-byte** validate header thật (crate `infer`) — chặn upload rác/đánh lừa content-type.

**Allowlist:** **[Validate #2] user chốt MỞ mọi tài khoản Google** → allowlist mặc định TẮT; enforce ở callback OAuth (Phase 2) chỉ khi được bật + **audit-log mọi email đăng nhập** (bắt buộc, để phát hiện abuse). Vì mở đăng ký, phòng thủ abuse dồn sang: quota 5GB + size cap 2GB + quota tăng dần + rate-limit (firewall CF #2) + ffmpeg timeout/version-pin (#15). Allowlist compile-time giữ như "công tắc khẩn". <!-- Updated: Validation Session 1 - allowlist MỞ (audit-only) -->

**Security pass:** ownership mọi endpoint (`/api/files/:id`, `/api/jobs/:id`, download, stream); cookie `HttpOnly+Secure+SameSite`; secret chỉ nhúng compile-time, **không commit binary/secret vào git** (`.gitignore` target + binary); ffmpeg chạy input không tin cậy có time/size cap; không path-traversal ở filename (sanitize).

## Related Code Files

- Create: `server/scribe-be/src/retention.rs` (sweep purge + orphan), `server/scribe-be/src/quota.rs`, `server/scribe-be/src/middleware/rate_limit.rs`, `web/src/components/QuotaBar.vue`
- Modify: `server/scribe-be/src/upload.rs` (quota tăng dần + size cap + magic-byte enforce), `server/scribe-be/src/models/file.rs` (expiresAt + index thường, KHÔNG TTL native), `server/scribe-be/src/auth.rs` (allowlist + audit), `server/scribe-be/src/routes.rs` (rate-limit layer đọc real IP đã nginx chuẩn hoá), `.gitignore`, `deploy/scribe-be.service`, `deploy/nginx-scribe.conf` (set_real_ip_from + firewall CF)

## Implementation Steps

1. `file.rs`: thêm `expiresAt` + index THƯỜNG trên `expiresAt` (**KHÔNG `expire_after`/TTL native** — xem #1). Set `expiresAt` khi tạo file.
2. `retention.rs`: sweep định kỳ (tokio interval) — xoá file disk quá hạn (recording/upload/outputs/zip) + xoá doc + dọn orphan; log số file/dung lượng thu hồi.
3. `quota.rs`: tính usage/user; enforce TĂNG DẦN khi append chunk (413 nếu vượt, không chỉ finalize — #3) + `MAX_FILE_SIZE_BYTES`; atomic/khoá theo user chống race; endpoint `/api/me/usage`.
4. `rate_limit.rs`: middleware theo real IP (CF-Connecting-IP), giới hạn `/api/upload` + `/api/auth`.
5. Magic-byte (`infer`) + sanitize filename (chống path-traversal) ở upload.
6. Verify allowlist enforce + audit log; security sweep ownership toàn bộ endpoint.
7. `.gitignore`: `target/`, binary artifact, mọi file secret. Verify không secret trong repo.
8. E2E: tạo file `expiresAt` quá khứ → sweep xoá cả disk+doc; upload vượt quota → 413; spam upload → rate-limited; user A không đọc được file user B.

## Success Criteria

- [x] File quá hạn N=14 ngày bị xoá **cả disk lẫn Mongo** (sweep disk-TRƯỚC→doc; lỗi disk non-NotFound → giữ doc retry, không mồ côi); orphan `.part`/session/job cũ được dọn.
- [x] Vượt quota → chặn upload/ghi (413, tăng dần khi append — có sẵn Phase 3/4); `/api/me/usage` + QuotaBar hiển thị đúng.
- [x] Rate-limit theo real IP (X-Real-IP nginx set): auth 20/60s, protected 300/60s (miễn `/chunk`); 429.
- [x] Magic-byte (`infer`) enforce (Phase 3/4); size cap 2GB; filename = ObjectId server-gen + UUID validate → path-traversal N/A.
- [x] Ownership mọi endpoint (`get_owned`); cookie HttpOnly+Secure+SameSite=Lax; `.gitignore` target/.session-key/.env → không secret/binary trong git.

## Cook progress — 2026-07-12 (code + review-fix + deploy)

- **Đã có sẵn (Phase 3/4):** quota tăng dần + size cap + magic-byte + ownership + allowlist(mở)+audit-log + cookie flags + gitignore secret. Phase 5 chỉ bổ sung phần còn thiếu.
- **Mới:** `expiresAt` + index thường (KHÔNG TTL native, RT#1) + backfill doc cũ (tính từ `createdAt`); `retention.rs` sweep 1h (disk→doc, dọn orphan .part/session/job, panic-isolation từng vòng); `rate_limit.rs` (in-memory theo X-Real-IP, auth 20 / api 300, miễn /chunk); `/api/me/usage` + FE `QuotaBar` + `use-usage`; durationS fallback (timer client) cho webm.
- **Code-review:** 1 High (sweep nuốt lỗi disk vẫn xoá doc → mồ côi) + High (rate-limit chặn nhầm chunk upload lớn) + Medium (dọn jobs cũ; refresh quota sau job done; panic-isolation; backfill từ createdAt) — **fix hết**.
- **Rủi ro chấp nhận (RT plan "loại trừ file có job running"):** KHÔNG implement exclude — với N=14 ngày ≫ ffmpeg timeout 900s + worker concurrency=1, không thể có file quá hạn còn job chạy; nếu xảy ra thì fail graceful ("input file không tồn tại"), không crash/corrupt. Ghi nhận thay vì thêm join phức tạp (YAGNI).
- **Verify:** cargo check + cargo test (musl container) pass; FE typecheck+build pass. Deploy .65 + E2E.
- **Bổ sung (2026-07-12):** `DELETE /api/files/:id` xoá recording thủ công (ownership + disk-trước→doc) + nút Xoá (xác nhận inline) ở lịch sử + refresh QuotaBar. E2E live: DELETE 200 → list rỗng + usage giảm đúng size; DELETE id không sở hữu → 404.

## Risk Assessment

- **Orphan disk (RT-11 style) — ĐÃ GIẢI bằng #1:** bỏ TTL index native (nguồn gây race); sweep là tiến trình DUY NHẤT, xoá disk TRƯỚC rồi doc → không còn cửa sổ đua. Test đối chiếu `du` trước/sau sweep.
- **CF XFF spoofing:** chỉ tin `CF-Connecting-IP` + trust CF IP ranges; không tin XFF thô → tránh bypass rate-limit.
- **Sweep xoá nhầm file đang xử lý:** loại trừ file có job `running`; grace period.
- **Quota race (2 upload đồng thời):** tính usage atomic hoặc khoá theo user khi finalize.
- **N ngày/quota/size cap — ĐÃ CHỐT (validate):** 14 ngày / 5GB per user / 2GB per file, config compile-time. Đổi giá trị = rebuild (hệ quả env compile-time).
