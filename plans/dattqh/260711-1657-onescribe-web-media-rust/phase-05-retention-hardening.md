---
phase: 5
title: "Retention & hardening"
status: pending
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

**Retention (2 lớp — TTL không đủ):** `files.expiresAt = now + N ngày`. MongoDB **TTL index** xoá DOC nhưng **KHÔNG xoá file trên disk** → cần **sweep job** (tokio interval trong BE hoặc systemd timer riêng): quét file quá hạn → **xoá file disk (recording/upload/outputs/zip)** rồi xoá doc. Sweep cũng dọn **orphan** (file disk không có doc, doc không có file). N cấu hình compile-time (đề xuất 14 ngày — chờ user chốt).

**Quota/user:** tổng `sum(files.size)` theo `userId`; enforce ở **finalize upload** (reject 413 nếu vượt `QUOTA_BYTES`, đề xuất 5GB — chờ chốt). Hiển thị usage ở FE.

**Rate-limit + caps:** giới hạn req/phút cho `/api/upload` + `/api/auth` (tower `governor` hoặc middleware custom) theo **real IP = CF `CF-Connecting-IP`/XFF** (trust CF ranges, không tin XFF tuỳ tiện). Size cap/file (đã Phase 3). **Magic-byte** validate header thật (crate `infer`) — chặn upload rác/đánh lừa content-type.

**Allowlist:** enforce ở callback OAuth (Phase 2) — verify hoạt động + audit log email đăng nhập.

**Security pass:** ownership mọi endpoint (`/api/files/:id`, `/api/jobs/:id`, download, stream); cookie `HttpOnly+Secure+SameSite`; secret chỉ nhúng compile-time, **không commit binary/secret vào git** (`.gitignore` target + binary); ffmpeg chạy input không tin cậy có time/size cap; không path-traversal ở filename (sanitize).

## Related Code Files

- Create: `server/onescribe-be/src/retention.rs` (sweep purge + orphan), `server/onescribe-be/src/quota.rs`, `server/onescribe-be/src/middleware/rate_limit.rs`, `web/src/components/QuotaBar.vue`
- Modify: `server/onescribe-be/src/upload.rs` (quota + magic-byte enforce), `server/onescribe-be/src/models/file.rs` (expiresAt + TTL index), `server/onescribe-be/src/auth.rs` (allowlist + audit), `server/onescribe-be/src/routes.rs` (rate-limit layer), `.gitignore`, `deploy/onescribe-be.service`

## Implementation Steps

1. `file.rs`: thêm `expiresAt` + tạo TTL index (`expire_after` hoặc index + sweep). Set `expiresAt` khi tạo file.
2. `retention.rs`: sweep định kỳ (tokio interval) — xoá file disk quá hạn (recording/upload/outputs/zip) + xoá doc + dọn orphan; log số file/dung lượng thu hồi.
3. `quota.rs`: tính usage/user; enforce ở finalize upload (413 nếu vượt); endpoint `/api/me/usage`.
4. `rate_limit.rs`: middleware theo real IP (CF-Connecting-IP), giới hạn `/api/upload` + `/api/auth`.
5. Magic-byte (`infer`) + sanitize filename (chống path-traversal) ở upload.
6. Verify allowlist enforce + audit log; security sweep ownership toàn bộ endpoint.
7. `.gitignore`: `target/`, binary artifact, mọi file secret. Verify không secret trong repo.
8. E2E: tạo file `expiresAt` quá khứ → sweep xoá cả disk+doc; upload vượt quota → 413; spam upload → rate-limited; user A không đọc được file user B.

## Success Criteria

- [ ] File quá hạn N ngày bị xoá **cả disk lẫn Mongo** (kiểm `du` trước/sau); orphan được dọn.
- [ ] Vượt quota → chặn upload (413); `/api/me/usage` đúng.
- [ ] Rate-limit chặn spam `/api/upload` + `/api/auth` theo real IP (CF XFF).
- [ ] Magic-byte + sanitize filename chặn file rác/path-traversal; size cap enforce.
- [ ] Ownership mọi endpoint; cookie flags đúng; **không secret/binary trong git**.

## Risk Assessment

- **TTL xoá doc nhưng bỏ file disk (RT-11 style):** sweep phải xoá disk trước/đồng thời; test đối chiếu `du`.
- **CF XFF spoofing:** chỉ tin `CF-Connecting-IP` + trust CF IP ranges; không tin XFF thô → tránh bypass rate-limit.
- **Sweep xoá nhầm file đang xử lý:** loại trừ file có job `running`; grace period.
- **Quota race (2 upload đồng thời):** tính usage atomic hoặc khoá theo user khi finalize.
- **N ngày/quota chưa chốt:** để config compile-time, giá trị mặc định (14 ngày/5GB) chờ user xác nhận trước deploy.
