---
phase: 7
title: "Hạ tầng: tách 7a (DB + service), 7b (đăng nhập + domain), 7c (mở chép lời)"
status: in-progress
priority: P1
effort: "M"
dependencies: [6]
---

# Phase 7: Hạ tầng — ba bước tách rời

Thao tác ghi prod; người dùng đã đồng ý các hạng mục phần B. Mỗi bước một cửa sổ riêng, sao lưu trước, kiểm xong mới sang bước sau.

## 7a — Đổi DB, service, thư mục (vẫn bản đang chạy, chưa đổi đăng nhập)

1. Hàng đợi rỗng; `mongodump --db onescribe` đầy đủ vào `/root/onescribe-backup-<ts>/` (URI qua stdin); sao lưu binary, dist, `data/` (rsync giữ quyền).
2. Tạo db `scribe` + user riêng (mật khẩu sinh trên server, không in); `mongorestore --nsFrom 'onescribe.*' --nsTo 'scribe.*'`; đối chiếu số doc từng collection.
3. **Sửa đường dẫn tuyệt đối:** `files.path` lưu `{DATA_DIR}/{userId}/…` (`recordings.rs:166`, `upload.rs:186`): `updateMany` pipeline `$replaceOne` tiền tố `/opt/onescribe-be/data` → `/opt/scribe-be/data` trên db `scribe`; đếm trước/sau; `stat` 5 tệp ngẫu nhiên.
4. Unit `scribe-be` (user `scribe`, `/opt/scribe-be`, web root `/var/www/scribe/dist`) theo `deploy/scribe-be.service`; chép `data/`, `.session-key`, tệp key OpenAI (chủ mới, 600). Build với `DB_NAME=scribe`, `DATA_DIR`, `SESSION_KEY_PATH`, `OPENAI_KEY_PATH` mới.
5. vhost hiện tại `scribe.inetdev.io.vn` trỏ sang unit mới (cùng cổng) hoặc đổi cổng; dừng `onescribe-be`, bật `scribe-be`.
6. Kiểm: health, đăng nhập (Firebase vẫn chạy ở bước này), mở 3 bản ghi cũ phát + xuất được, chép một bản ngắn, sổ phút đúng.
- Rollback 7a: bật lại `onescribe-be` (db `onescribe` + `data/` cũ còn nguyên); ghi mới trong `scribe` sau thời điểm sao lưu sẽ thiếu ở bản cũ — báo người dùng.

### Kết quả 7a (2026-10-16 12:43)

- Jev: dừng bản cũ rồi mới sao chép (0,83), giữ cổng 8787 (0,94). Làm 7a trước, giữ 7b tới khi người dùng đăng nhập thử trên dev (0,94).
- Build `a90a7e8` (vẫn Firebase) với `DB_NAME=scribe`, `DATA_DIR`/`SESSION_KEY_PATH`/`OPENAI_KEY_PATH` dưới `/opt/scribe-be`; sha256 `ba60c557…`.
- User hệ thống `scribe` (uid 968), unit `scribe-be` từ `deploy/scribe-be.service`; user Mongo `scribe` (chỉ `readWrite`+`dbAdmin` trên db `scribe`) tạo bằng tài khoản quản trị, mật khẩu chỉ nằm trong `.env.production` (gitignore) và trên server.
- Cửa sổ chuyển: hàng đợi 0 → dừng `onescribe-be` 12:43:02 → sao lưu `/root/onescribe-backup-20261016-124301/` (dump db `onescribe` 9 collection, binary, dist, vhost, unit) → `mongorestore` sang `scribe`: 9/9 collection khớp số doc (users 1, files 53, audit_events 124, còn lại 0) → `files.path` sửa 53/53, còn 0 đường dẫn cũ → `data/` 268 MB, 56/56 tệp → vhost `root /var/www/scribe/dist`, `nginx -t` OK, reload → `scribe-be` active 12:43:05. Downtime khoảng 3 giây.
- Nghiệm thu: FE 200 (`index-8BNOtOKg.js`), `/api/health` ok, `/api/me` 401, journal "chép lời bật (key server)", NRestarts 0, RSS ~11 MB, 5 `files.path` ngẫu nhiên tồn tại và thuộc `scribe`; tệp cấu hình admin tạm đã xoá.
- Chưa: người dùng đăng nhập (Firebase) mở bản ghi cũ và chép một bản ngắn.
- Dọn 12:55 theo yêu cầu người dùng ("ko cần giữ cũ gì hết"; Jev giữ bản dump mới nhất 0,95): drop user Mongo + db `onescribe` (sau khi kiểm `scribe` đủ dữ liệu), xoá unit `onescribe-be`, `/opt/onescribe-be`, `/var/www/onescribe`, user hệ thống `onescribe`, tệp `.bak` vhost, bốn thư mục sao lưu cũ (15/10 và 16/10 sáng). Còn giữ `/root/onescribe-backup-20261016-124301/` (17 MB, bản sao lưu duy nhất của dữ liệu hiện tại). Không còn đường lùi về bản cũ.

## 7b — Đổi đăng nhập + domain (một chiều)

1. DNS `A scribe.hoandat.com → 103.72.98.65` proxied (như `auth.hoandat.com`), zone SSL "full", origin dùng chứng chỉ tự ký chung `/etc/pki/nginx/server.{crt,key}`.
2. vhost `scribe.hoandat.com` từ `deploy/nginx-scribe.conf` (443 ssl với chứng chỉ chung, 80 → https; `nginx -t` trước reload). **Bỏ hẳn domain cũ** (người dùng chốt 16/10 13:05, không 301): xoá vhost `onescribe.conf` (`scribe.inetdev.io.vn`) trong cùng cửa sổ, sau khi domain mới đã chạy; sao lưu tệp vhost cũ vào thư mục sao lưu của lượt.
3. IdP đã có `scribe-web`/`scribe-mobile` (Phase 4). Deploy backend + web bản OIDC (Phase 5–6) với `PUBLIC_URL=https://scribe.hoandat.com`; cài app iPhone mới (`VITE_NATIVE_API_ORIGIN=https://scribe.hoandat.com`).
4. Kiểm: đăng nhập web, user cũ được ghép theo email giữ nguyên bản ghi; iPhone đăng nhập, ghi, tải lên; back-channel logout web.
- Rollback 7b: không quay về Firebase (người dùng chốt); lỗi ⟹ sửa tiến. Trước 7b phải có bằng chứng thử đủ trên dev local (Phase 6).

### Kết quả 7b (2026-10-16 13:27, phần web)

- Điều kiện: người dùng đăng nhập dev thành công 13:24 sau khi sửa lỗi đọc email (hoandat-auth chỉ trả `email`/`email_verified` ở `userinfo` với code flow; commit `4516f28`; tài liệu IdP sửa `d858e7a`). Jev: làm web ngay, cài iPhone sau (0,98).
- IdP: thêm `SCRIBE_WEB_CLIENT_SECRET` vào `/var/www/hoandat-auth/.env` (sao lưu `.env.bak-*`), `pm2 reload hoandat-auth`; healthz 200, `/auth?client_id=scribe-web` → trang đăng nhập; localman, chat vẫn 200.
- Build `e44badf`+`4516f28` (`PUBLIC_URL=https://scribe.hoandat.com`, bỏ `FIREBASE_PROJECT_ID`), sha256 `6d325d75…`; FE `index-Co8MngNB.js` (không còn Firebase, không còn domain cũ).
- Cửa sổ: hàng đợi 0, `firebaseUid: null` 0 → sao lưu `/root/scribe-backup-20261016-132747/` (dump db `scribe` 9 collection, binary, dist, vhost cũ) → thêm `/etc/nginx/conf.d/scribe.conf`, `nginx -t` OK → thay binary + dist, restart 13:27:48 → index `firebaseUid_unique`, `idpSub_unique` → xoá `onescribe.conf`, `nginx -t`, reload.
- Nghiệm thu ngoài: FE 200, `/api/health` ok, `/api/me` 401, `/api/auth/login` 303 sang IdP (`scribe-web`), cookie `__Host-scribe_login; Secure`, http 301 → https, callback sai → `/login?error=expired`, `/api/auth/native/config` đúng client `scribe-mobile`. Domain cũ không còn tới Scribe (DNS Cloudflare còn bản ghi, rơi vào vòng 301 của vhost khác).
- 13:30 người dùng đăng nhập prod: user cũ được ghép theo email (`idpSub` gắn vào user Firebase), giữ gói plus, đủ 53 file.
- 13:4x báo động qua bot chat.hoandat.com `@bot_scribe_bot` (chat riêng id 12): token `/opt/scribe-be/.notify-bot-token` (600, chủ scribe), build `c24bb6a` sha `4a2e6114…`, hàng đợi 0 trước restart, `admin notify-test` gửi được.
- App iPhone mới (origin `https://scribe.hoandat.com`, đăng nhập hoandat-auth) cài qua cáp: BUILD SUCCEEDED, đã cài; người dùng mở app thủ công (máy đang khoá).
- Lỗi đăng nhập iPhone 13:39: sau trang đồng ý, IdP chuyển hướng về `com.hoandat.scribe:/auth/callback` nhưng CSP `form-action` của IdP không có scheme riêng nên WebKit chặn, sheet không đóng. Sửa ở hoandat-auth `52f5b11` (thêm scheme của `CHAT_MOBILE_REDIRECT_URI`, `SCRIBE_MOBILE_REDIRECT_URI` vào `form-action`; 519 test + 10 e2e), deploy `infra/deploy.sh`, healthz 200, localman/chat 200.
- 13:4x iPhone: `native/exchange` 200, 1 thiết bị + 1 phiên mobile, ghi âm (`recordings/init` → `finalize`), chép lời bằng key server (`transcript.create` → `transcript.done`), phát lại file; journal 0 ERROR.
- Chưa: back-channel logout thật, xoá bản ghi DNS `scribe.inetdev.io.vn` ở Cloudflare (chờ người dùng).

## 7c — Mở chép lời (vài ngày sau 7b)

Bật `TRANSCRIBE_OPEN` (build lại) khi 7b chạy ổn vài ngày (Jev 0,98) **và Phase 9 đã deploy**. Quota `free` 120 phút, quỹ free $10, trần tổng $30 đã có từ phần A. Kiểm một tài khoản mới chép được trong hạn mức, chạm quỹ free bị chặn có số.

## Dọn (mỗi bước hỏi trước)

Bản cũ đã xoá ngày 16/10 (xem kết quả 7a); domain cũ bỏ ở 7b. Còn: xoá sao lưu `/root/onescribe-backup-20261016-124301/` khi người dùng xác nhận ổn định.
