---
phase: 7
title: "Hạ tầng: tách 7a (DB + service), 7b (đăng nhập + domain), 7c (mở chép lời)"
status: pending
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

## 7b — Đổi đăng nhập + domain (một chiều)

1. DNS `A scribe.hoandat.com → 103.72.98.65` proxied (như `auth.hoandat.com`), zone SSL "full", origin dùng chứng chỉ tự ký chung `/etc/pki/nginx/server.{crt,key}`.
2. vhost `scribe.hoandat.com` từ `deploy/nginx-scribe.conf` (`nginx -t` trước reload); vhost cũ thành 301 sang domain mới (giữ ≥ 30 ngày).
3. IdP đã có `scribe-web`/`scribe-mobile` (Phase 4). Deploy backend + web bản OIDC (Phase 5–6) với `PUBLIC_URL=https://scribe.hoandat.com`; cài app iPhone mới (`VITE_NATIVE_API_ORIGIN=https://scribe.hoandat.com`).
4. Kiểm: đăng nhập web, user cũ được ghép theo email giữ nguyên bản ghi; iPhone đăng nhập, ghi, tải lên; back-channel logout web.
- Rollback 7b: không quay về Firebase (người dùng chốt); lỗi ⟹ sửa tiến. Trước 7b phải có bằng chứng thử đủ trên dev local (Phase 6).

## 7c — Mở chép lời (vài ngày sau 7b)

Bật `TRANSCRIBE_OPEN` (build lại) khi 7b chạy ổn vài ngày (Jev 0,98) **và Phase 9 đã deploy**. Quota `free` 120 phút, quỹ free $10, trần tổng $30 đã có từ phần A. Kiểm một tài khoản mới chép được trong hạn mức, chạm quỹ free bị chặn có số.

## Dọn (mỗi bước hỏi trước)

Tắt hẳn `onescribe-be`, xoá db `onescribe`, thư mục cũ, bỏ 301 sau 30 ngày; xoá sao lưu cũ sau khi người dùng xác nhận ổn định.
