# Docs Phase 8 — cập nhật tài liệu theo prod 2026-10-16 14:00

## Tệp đã sửa (chưa commit)

- `docs/deploy-web-production.md` — tên prod thật (`scribe-be`, `scribe`, `/opt/scribe-be`, `/var/www/scribe/dist`, `/etc/nginx/conf.d/scribe.conf`, db `scribe`); topology 443 + SSL Full; mục mới "Tệp bí mật trên server" và "Đăng nhập hoandat-auth"; biến build theo `build.rs`/`config.rs` (bỏ `FIREBASE_PROJECT_ID`); FE env chỉ `VITE_NATIVE_API_ORIGIN`; đăng nhập dev qua `scribe-web-dev`; restart không mất lượt; mảnh im lặng; build bằng `cargo zigbuild`; trạng thái đã xác minh 16/10; thêm `auth.link`, `auth.backchannel_logout`, `auth.device_revoked` vào danh sách audit; sửa ghi chú level meter (trỏ `LiveMonitor`, bỏ `RecordView.vue`).
- `docs/codebase-summary.md` — auth OIDC, `oidc.rs` (bỏ `jwks.rs` không còn), phiên Mongo, route auth mới, rate-limit đúng số (120/10/300, ngân sách IdP 45), collection mới, sổ ngày, CLI admin đủ lệnh, `notify.rs`, deploy.
- `docs/ios-app.md` — `HoandatAuthPlugin`, token thiết bị + `native/refresh`, chép lời bằng key server, bỏ ghi chú Firebase/GoogleService, kết quả QA đăng nhập hoandat-auth.
- `docs/project-roadmap.md` — mục "Bước 2" (A, Phase 9, mảnh im lặng, 7a, 7b xong; 7c chờ), trạng thái hiện tại, backlog.
- `README.md` — domain prod, đăng nhập hoandat-auth, key server.
- `.claude/skills/scribe-ops-prod/SKILL.md` — luật 4 (restart không mất lượt, vẫn tốn tiền), luật 8 (đủ tệp bí mật), bảng chỉ đường theo tiêu đề mới, định danh user, DNS domain cũ đã xoá, bỏ trigger domain cũ và trigger trùng. Phần cổng bắt buộc giữ nguyên.

## Phát hiện cần xử lý (ngoài phạm vi docs, không sửa)

1. `scripts/build-musl.sh` còn đòi `FIREBASE_PROJECT_ID` và không chuyển biến OIDC/`OPENAI_KEY_PATH`/hạn mức/báo động vào Docker ⟹ không dùng được. Docs đã ghi rõ và chuyển sang `cargo zigbuild`.
2. `web/ios/App/App/GoogleService-Info.plist` vẫn nằm trong git (không còn trong `project.pbxproj`, không còn mã dùng) — nên xoá.
3. Comment code cũ: `SecretsPlugin.swift` ("the Firebase refresh token"), `models/transcript.rs` `RESTARTED` ("key chỉ nằm trong RAM"), `main.rs:71` (firebaseUid), `vite.config.ts` (Firebase project), `deploy/nginx-scribe.conf` ("giới hạn chép lời theo IP", "Vue dist").
4. `.env.production` có header cũ (staging / `scribe.inetdev.io.vn` / `docs/deploy-web-staging.md`) — chỉ đọc tên biến, không sửa.
5. Frontmatter plan `261016-1114-…` vẫn `status: pending` cho phase 1–8 dù đã làm.

## Chưa xác minh được

- Trạng thái live (DNS domain cũ đã xoá, SSL Full, chủ/quyền tệp, bản sao lưu): lấy theo dữ kiện đã kiểm do lead cung cấp; plan 7b còn ghi "DNS Cloudflare còn bản ghi" — tôi theo dữ kiện của lead (đã xoá).
- Cổng dev của `scribe-web-dev`: phase 4 ghi `http://localhost:5174`, `vite.config.ts` cố định `5173` (`strictPort`). Docs chỉ ghi "`PUBLIC_URL` phải bằng origin đã khai ở IdP", không nêu cổng.
- Back-channel logout thật và đăng xuất ở IdP làm app mất phiên: chưa có bằng chứng chạy thật (plan 7b ghi "Chưa"); docs mô tả cơ chế theo mã, không ghi là đã kiểm.

## Kiểm

- Đối chiếu mã: `build.rs`, `config.rs`, `auth.rs`, `oidc.rs`, `session.rs`, `models/{session,user,usage,transcript}.rs`, `admin.rs`, `notify.rs`, `transcribe/{mod,chunking,openai}.rs`, `routes.rs`, `rate_limit.rs`, `deploy/*`, `web/.env.example`, `web/vite.config.ts`, `web/scripts/ios-device.sh`, `HoandatAuthPlugin.swift`, `SessionAuth.swift`, `SecretsPlugin.swift`.
- Link tương đối trong `README.md` và `docs/*.md`: không có link hỏng; mọi đường dẫn mã được trích đều tồn tại.
- Không còn Firebase / `scribe.inetdev.io.vn` / `onescribe-be` / `/opt/onescribe-be` / db `onescribe` ở dạng hiện trạng (chỉ còn trong ghi chú lịch sử có ngày).
- Số dòng: deploy 266, codebase 96, roadmap 82, ios 49, README 51, SKILL 216 (đều < 800).
- Không ghi giá trị bí mật nào; chỉ tên biến và đường dẫn tệp.
