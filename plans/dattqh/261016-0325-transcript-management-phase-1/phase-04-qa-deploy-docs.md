---
phase: 4
title: "QA iPhone, deploy prod, docs"
status: pending
priority: P1
effort: "S"
dependencies: [2, 3]
---

# Phase 4: QA iPhone, deploy prod, docs

<!-- Updated: Red Team 2026-10-16 - sao lưu DB ngay trước deploy, lệnh khôi phục đúng, chờ hàng đợi rỗng, hạn xoá bản sao lưu -->

## Goal

Tính năng chạy trên iPhone thật với dữ liệu prod; tài liệu khớp hành vi mới.

## Tasks

1. Dev local: chép lời một phiên iPhone (dữ liệu dev), đi hết luồng đọc → sửa → chép lại một đoạn → đổi tên → xuất → xoá.
2. Xin người dùng đồng ý deploy prod.
3. Ngay trước deploy:
   - Hàng đợi rỗng: `db.transcripts.countDocuments({status:{$in:["queued","running"]}})` = 0 (restart làm hỏng lượt đang chạy, `transcribe/mod.rs:274-279`). Khác 0 thì chờ.
   - Sao lưu: `mongodump --db "$DB_NAME" --collection transcripts` vào `/root/<app>-backup-<YYYYMMDD-HHMMSS>/` (`chmod 700`), cùng binary + FE cũ.
4. Deploy theo `docs/deploy-web-production.md` (nạp `.env.production`), kiểm `/api/health`, journal, index `transcript_docs` đã tạo.
5. `cd web && npm run ios:device`.
6. QA trên iPhone (người dùng bấm, tôi đối chiếu log server và DB):
   - Phiên 48 phút sáng 2026-10-16: bấm Chép lời → tab Transcript hiện **một** transcript, tiến độ tăng dần.
   - Chạm 3 câu ở 3 đoạn khác nhau → phát đúng chỗ (lệch ≤ 1 s); để phát qua ranh giới đoạn.
   - Sửa 2 câu → Lưu → thoát app → mở lại vẫn còn; xuất txt có bản sửa.
   - Chép lại một đoạn đã sửa → có xác nhận, đoạn đó về câu máy mới, các đoạn khác giữ bản sửa.
   - Đổi tên, xoá tên; xoá transcript → audio vẫn phát trong Lịch sử ghi.
   - Đăng xuất → cache audio trên máy bị xoá.
   - Tài khoản không có quyền chép lời: không thấy tab.
7. Docs: `docs/codebase-summary.md` (collection `transcript_docs`, route mới, tab Transcript), `docs/project-roadmap.md`, `docs/ios-app.md` (bảng QA), `docs/deploy-web-production.md` (bước "chờ hàng đợi rỗng" trước mỗi lần restart).
8. Sau 14 ngày hoặc khi người dùng xác nhận ổn định: xoá thư mục sao lưu.

## Tiến độ (2026-10-16)

- [x] Review độc lập trước deploy (`../reports/code-reviewer-261016-1105-transcript-phase-1.md`): 1 High + 1 Medium + 3 Low đã sửa, commit `ae0039d`.
- [x] Người dùng đồng ý deploy prod, commit và push (push `main` tới `002d0d5`).
- [x] Hàng đợi chép lời prod = 0; sao lưu `/root/onescribe-backup-20261016-110644/` (dump `transcripts`, `onescribe-be.prev`, `dist/`).
- [x] Deploy 11:07: binary sha256 khớp hai đầu (`b46606f9…`), `onescribe-be` active, `/api/health` OK, index unique `{userId, key}` của `transcript_docs` đã tạo, FE trả `assets/index-Fhu8MBkS.js` đúng bản build, `/api/transcript-docs` trả 401 khi chưa đăng nhập.
- [x] Docs: `docs/codebase-summary.md`, `docs/deploy-web-production.md` (bước chờ hàng đợi rỗng, sao lưu/khôi phục `transcripts`, sự kiện audit).
- [x] Skill vận hành prod `.claude/skills/scribe-ops-prod/` (SSH/mongosh chỉ đọc qua script).
- [ ] QA trên iPhone thật (mục 5–6), gồm autoplay đoạn kế trong WKWebView và mạng chậm.
- [ ] Journal 30 phút sau deploy không có `ERROR` mới.
- [ ] `docs/ios-app.md` (bảng QA), `docs/project-roadmap.md` sau QA iPhone.
- [ ] Xoá `/root/onescribe-backup-20261016-110644/` sau 2026-10-30 hoặc khi người dùng xác nhận ổn định (hỏi trước).

## Verification

- CI GitHub xanh trên commit cuối.
- `audit_events` có `transcript.rename/edit/rerun/delete` tương ứng QA, `details` không chứa tiêu đề hay câu.
- Không có `ERROR` mới trong `journalctl -u "$DEPLOY_SERVICE"` 30 phút sau deploy.

## Rollback

1. Khôi phục binary + FE từ thư mục sao lưu (lệnh trong `docs/deploy-web-production.md`).
2. Bản cũ bỏ qua `transcript_docs`; vì phần sửa gắn `runId`, chép lại trong lúc chạy bản cũ chỉ làm phần sửa hết hiệu lực, không lệch câu khi deploy lại.
3. Chỉ khi dữ liệu `transcripts` hỏng: `mongorestore --drop --nsInclude "$DB_NAME.transcripts" <thư mục sao lưu>` (người dùng đồng ý trước).
