# ADR 0001: Scribe phục vụ mọi tenant theo Host, cô lập theo user và `tenantId`

- Trạng thái: chấp nhận
- Ngày: 2026-10-24
- Phạm vi: backend `server/scribe-be` (tenant theo Host, danh tính, phiên, hàng đợi, OIDC của tenant sổ, key AI của tenant), web `web/` (trang "Scribe chưa bật", link theo tenant), nginx `deploy/`. Đi cùng ADR 0012 của hoandat-auth (`auth-next/docs/decisions/0012-scribe-on-tenants.md`): key AI do chủ domain nhập ở `/admin`, niêm phong cho Scribe.

## Bối cảnh

Scribe chỉ chạy cho tenant chính `hoandat.com` ở `https://scribe.hoandat.com`: backend không đọc `Host`, mọi URL dựng từ `PUBLIC_URL`, user ghép theo `idpSub` với index unique toàn cục, một client OIDC cho một issuer. hoandat-auth đã phục vụ nhiều tenant theo domain (ADR 0006 của nó): mỗi tenant có IdP `https://auth.<domain>` riêng, `sub` chỉ có nghĩa trong tenant của nó.

Mục tiêu (24/10/2026, Jev chốt, user uỷ quyền): người dùng của mọi tenant đang phục vụ dùng Scribe web ở `https://scribe.<domain>`, đăng nhập bằng IdP của domain đó, tiền AI do chủ domain trả bằng key của họ. Máy chủ `.65` dùng chung, RAM ít: không thêm process.

## Quyết định

### 1. Một process cho mọi tenant, chọn theo Host

- Middleware `tenant::resolve` ngoài cùng của `/api` (trừ `/api/health`) chuẩn hoá Host (chữ thường, bỏ cổng, bỏ dấu chấm cuối) và gắn `TenantCtx { id, domain, issuer, public_url, display_name, primary, enabled }` vào request.
- Host bằng host của `PUBLIC_URL`, hoặc loopback (dev qua proxy của Vite, lệnh trên máy chủ): **tenant chính**, id `primary` (cùng id với hoandat-auth), tĩnh, không hỏi IdP khi nhận request. Mọi hành vi của tenant chính giữ như trước: key server, gói, hạn mức, app iPhone/Android, allowlist.
- Host `scribe.<domain>` khác: tra `GET /internal/tenants/lookup?host=` của API nội bộ hoandat-auth (Basic `scribe`), cache 60 giây, 404 cache 30 giây, IdP lỗi thì dùng câu trả lời cũ và thử lại sau 10 giây (cùng mốc với Localman). IdP trả tenant chính cho một host khác, hay domain không khớp host: coi như không có. Không có tenant: 404 `UNKNOWN_TENANT`. Host khác nữa: 404.
- Không có API nội bộ thì chỉ phục vụ tenant chính.
- Host lạ không được làm IdP làm việc không giới hạn: host chưa có trong cache chỉ được hỏi trong ngân sách 30 lượt/phút mỗi IP và 300 lượt/phút cả server (hết ngân sách ⟹ 429, host đã biết vẫn chạy), IdP lỗi với host chưa biết được nhớ 10 giây, và block nginx của tenant sổ trả 421 khi `Host` khác tên đã bắt tay TLS.
- `PUBLIC_URL` không có host ⟹ không khởi động (không có tenant chính thì mọi request công khai đều 404).

### 2. Tenant sổ chỉ chạy khi đủ điều kiện, mặc định tắt

`TenantCtx.enabled` của tenant sổ đòi cả bốn:

1. IdP liệt kê `webApps.scribe = https://scribe.<domain>` (`SCRIBE_TENANTS` bật ở IdP và chủ domain đã nhập ít nhất một key chép lời) và tenant `active`.
2. Đăng nhập qua IdP của tenant dựng được ở phía Scribe: API nội bộ, listener loopback của IdP và khoá app `scribe-web` (`tenant::Readiness::oidc`).
3. Scribe mở được key niêm phong của tenant: có vòng khoá niêm phong (`tenant::Readiness::sealed_keys`). Chưa có thì không tenant sổ nào chạy, nên tenant sổ không bao giờ rơi vào key của server.
4. Người vận hành không tắt tenant (`scribe-be admin tenant-disable <id>`, cờ trong `kv`).

Ngoài điều kiện 3, key server và tính năng chỉ của tenant chính còn được chặn ngay tại chỗ: tenant sổ không có key riêng ⟹ 403 `TENANT_KEY_MISSING` (chép lời, biên bản, hỏi đáp, dịch), liên kết chat, gửi biên bản qua bot và chép trực tiếp ⟹ 403 `NOT_ON_TENANT`, đăng nhập app di động ⟹ 404.

Chưa bật: route cần đăng nhập và route đăng nhập của app trả 403 `TENANT_DISABLED`, `/api/auth/login` và callback chuyển về `/login?error=not_enabled` của chính tenant đó, `/api/idp` luôn trả `{ displayName, domain, issuer, primary, scribe: "enabled" | "disabled" }` để web hiện trang "Scribe chưa bật trên domain này" kèm link `https://auth.<domain>/admin`. Đăng xuất và back-channel logout chạy cả khi tenant đã tắt.

### 3. Cô lập theo user, `tenantId` ở danh tính và hàng đợi

Mọi dữ liệu Scribe đã khoá theo `userId` (danh sách xoá ở `server/scribe-be/src/account.rs`): tệp, bản chép lời, biên bản, bản dịch, index tìm kiếm, key riêng, sổ dùng. User là của một tenant, nên chỉ cần `tenantId` ở chỗ quyết định "ai là user này":

- `users`: index unique `(tenantId, idpSub)` thay `idpSub_unique` (tạo mới trước, bỏ cũ sau). Cùng `sub` ở hai tenant là hai user. User thời Firebase chỉ được ghép theo email ở tenant chính.
- `sessions`, `devices`, `auth_states`, `logout_jtis` (khoá `<tenantId>:<jti>`): mang `tenantId`, chỉ đọc và xoá được trong đúng tenant của request. Cookie phiên `__Host-scribe` là host-only nên trình duyệt không gửi chéo; `require_auth` vẫn kiểm `tenantId` của phiên (cả phiên trong cache), phiên của tenant khác là 401 và không bị xoá.
- `transcripts`, `jobs`: mang `tenantId` của người xếp. Hàng đợi vẫn một worker (RAM `.65`), nhận lượt xoay vòng: ưu tiên lượt chờ lâu nhất của tenant khác tenant vừa chạy, nên một tenant không chặn hẳn tenant khác.
- Không thêm `tenantId` vào các collection khoá theo `userId` khác: thêm vào là dữ liệu trùng phải giữ đồng bộ mà không thêm bảo đảm nào.

Dữ liệu có trước được gắn `tenantId: "primary"` lúc khởi động (`tenant::migrate`, như `migrateToTenants` của hoandat-auth): `users`, `sessions`, `devices`, `transcripts`, `jobs` chỉ thêm trường; `auth_states`, `logout_jtis` thiếu `tenantId` bị xoá (TTL 10 phút). Lặp lại được; đếm lại sau khi ghi, còn sót thì không phục vụ (`exit(1)`). Chạy trước index unique mới, worker và request đầu tiên.

### 4. Allowlist và tính năng chỉ của tenant chính

- `ALLOWED_DOMAINS`, `ALLOWED_EMAILS`, `TRANSCRIBE_EMAILS`/`TRANSCRIBE_OPEN`, `REALTIME_EMAILS`, `UNVERIFIED_EMAIL_SUBS` là của tenant chính. Tenant sổ: mọi tài khoản đã xác minh email ở IdP của domain đó đăng nhập và chép lời được (bằng key của chủ domain); không chép trực tiếp; không nhận email chưa xác minh.
- Chỉ tenant chính: app iPhone/Android (đăng nhập native), chép trực tiếp, liên kết chat và gửi biên bản qua bot của `chat.hoandat.com`, gói Plus, quỹ free, trần USD tháng. Tenant sổ dùng hạn mức dung lượng gói free.

### 5. Đăng nhập trên tenant sổ theo mẫu Localman

Discovery và JWKS của tenant sổ đọc qua API nội bộ (không tin DNS của domain chủ tenant), token/userinfo/thu hồi gửi tới listener công khai của IdP trên loopback với `Host: auth.<domain>`, `X-Forwarded-Proto: https`. Secret client `scribe-web` của tenant sổ = `base64url(HMAC-SHA256(khoá app scribe-web, tenantId))`, Scribe chỉ giữ khoá app (tệp 600). Tenant chính giữ đường OIDC công khai như cũ.

### 6. Key AI của tenant

Thứ tự key cho mỗi việc: key riêng của user → key của tenant (chủ domain nhập ở `/admin`, niêm phong cho Scribe) → key server, chỉ ở tenant chính. Một hàm chọn (`tenant_keys::choose`) cho chép lời và model chữ (biên bản, hỏi đáp, dịch); tenant chính không bao giờ đọc key tenant, tenant sổ không bao giờ ra key server (test ma trận). Lượt bằng key tenant ghi như key riêng (`byok`) kèm `keySource: "tenant"`: không trừ phút, quỹ, lượt ngày hay trần USD, không vào sổ phút key riêng của user; hạn mức dung lượng gói free vẫn áp. Worker mở key theo `tenantId` của lượt.

- **Vòng khoá niêm phong:** `TENANT_SEALING_KEYS_DIR` (700), mỗi tệp `<recipient>.pem` (600) là private key RSA ≥ 3072 bit PKCS#8; `recipient` = 16 hex đầu SHA-256 của SPKI DER. Tệp lệch vân tay, quyền rộng hay khoá nhỏ bị bỏ (log lỗi); không còn khoá nào ⟹ tắt (`Readiness::sealed_keys`). Nhiều khoá cùng lúc khi xoay, chọn theo `recipient` của bản niêm phong.
- **Mở:** RSA-OAEP SHA-256 (MGF1 SHA-256) mở khoá AES, AES-256-GCM với AAD `<tenantId>\nscribe-<vendor>`; vector chéo ngôn ngữ của auth-next nằm ở `server/scribe-be/tests/fixtures/scribe-sealed-vector.json`, test mở đúng và từ chối sai AAD, sai `recipient`, một byte bị sửa. Bản rõ trong `Zeroizing`, không kiểu nào có `Debug` in nội dung.
- **Cache:** bản niêm phong theo tenant 60 giây, một lần hỏi IdP mỗi tenant cùng lúc; IdP lỗi ⟹ dùng bản cũ tối đa 10 phút (hỏi lại sau 10 giây), quá hạn ⟹ 503 "chưa đọc được key của domain". Không giữ bản rõ.
- **Key lỗi:** `invalid | quota | unreadable` ⟹ đánh dấu tại chỗ theo `(tenant, nhà cung cấp, tag)` (lượt sau không dùng lại bản đó, bản owner nhập lại có tag mới), báo IdP kèm `recipient` và `tag`, audit `tenant_key.failed`. Không đổi cờ tạm ngưng của key server, không báo bot, không chuyển sang key server.

Scribe mở bản niêm phong bằng crate `rsa` 0.9 (thuần Rust, giữ binary musl tĩnh). Crate có cảnh báo RUSTSEC-2023-0071 (Marvin): giải mã RSA không hằng thời gian, đo thời gian rất nhiều lượt giải mã bản mã do kẻ tấn công chọn có thể lộ bản rõ. Chấp nhận vì Scribe không phải oracle giải mã: chỉ mở blob do IdP tạo, nhận qua loopback có Basic; lỗi giải mã không trả ra mạng và không phân biệt loại lỗi. Thêm vào đó Scribe giải mã có blinding (`decrypt_blinded` với RNG của hệ thống), làm thời gian giải mã khó gắn với bản mã.

### 7. Không chọn

- Mỗi tenant một process hay một database: RAM `.65` không đủ, và dữ liệu đã khoá theo `userId`.
- Thêm `tenantId` vào mọi collection: không thêm bảo đảm (user đã là của một tenant), thêm chỗ quên.
- Đọc thẳng `tenant_secrets` của IdP: hai database riêng, phá ranh giới sở hữu.

## Hệ quả

- Production không đổi cho tới khi đủ điều kiện ở mục 2: tenant chính, app di động và mọi tenant chưa bật giữ nguyên hành vi. Thay đổi duy nhất lúc deploy là migration gắn `tenantId` và đổi index `users`.
- Lệnh quản trị theo email nhận `--tenant <id>` (mặc định `primary`); thêm `tenants`, `tenant-disable`, `tenant-enable`, `purge-tenant <id> [--apply]`. Gỡ tenant ở IdP không tự xoá dữ liệu Scribe: người vận hành chạy `purge-tenant`.
- Bundle tự host của hoandat-auth không có Scribe; tenant chuyển sang tự host không mang dữ liệu Scribe theo.
- Thứ tự deploy: binary mới trước, block nginx của tenant sổ sau. Binary cũ không đọc `Host`: gặp `scribe.<domain>` nó phục vụ như tenant chính.
- Rollback: gỡ block nginx của tenant sổ (`scribe-tenants.conf`, `nginx -t`, reload) TRƯỚC, rồi chạy binary trước. Binary cũ bỏ qua `tenantId`, tạo lại `idpSub_unique` (sub của tenant sổ khác sub của tenant chính nên không trùng) và chỉ phục vụ tenant chính; không cần khôi phục bản sao lưu trừ khi dữ liệu hỏng. Doc nó ghi trong lúc chạy (chưa có `tenantId`) được binary mới gắn `primary` ở lần khởi động sau, nên migration chạy (rẻ: một lượt đọc mỗi collection) ở mọi lần khởi động.
