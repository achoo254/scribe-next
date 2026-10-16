---
phase: 3
title: "Cổng kiểm tra, deploy, docs"
status: completed
effort: "M"
---

# Phase 3: Ship

- Review code (code-reviewer) cho cả backend và web, rồi sửa các phát hiện.
- Chạy toàn bộ cổng một lần trên commit cuối.
- Deploy prod theo skill `scribe-ops-prod`:
  - điều kiện: hàng đợi 0, `recording_sessions` 0, `upload_sessions` 0;
  - sao lưu binary và dist;
  - FE giữ `assets/` cũ.
- Kiểm sau deploy: health, NRestarts, journal không có ERROR; một lượt tóm tắt, liên kết và gửi thật trên prod.
- Docs: `docs/codebase-summary.md` (route, collection mới, task long-poll), `docs/deploy-web-production.md` (biến build mới), `docs/project-roadmap.md`; journal.
- Đợt sau khi có iPhone: đánh dấu từ màn hình khoá hoặc AirPods (Now Playing remote command).

## Kết quả (16/10/2026)
- **Review:** 2 lượt.
  - Lượt 1 (backend) có 2 High và 4 Medium: sổ chi phí khi client ngắt, hoàn lượt khi đã tính tiền, liên kết hộ, biên bản của đoạn đã xoá, poll kẹt.
  - Lượt 2 (web + bản sửa) có 1 High và 5 Medium: giây dấu mốc trên iPhone sau khi chạy nền, tab ẩn, tên chat, mốc trùng, mốc mất khi cắt.
  - Đã sửa hết (b1905ee, 80ac509).
- **Deploy:**
  - 22:31, sao lưu `/root/scribe-backup-20261016-223131`, `index-Rr5G4V1P.js`.
  - Bản vá 22:50, sao lưu `/root/scribe-backup-20261016-225016`, binary sha `a7b4d997…`, `index-CO6iJT-G.js`.
  - Hàng đợi, `recording_sessions` và `upload_sessions` đều 0 trước mỗi lần.
- **Cổng trên 80ac509:**
  - clippy sạch;
  - cargo test: 84 đạt, 10 cần Mongo thật (4 test mới chạy riêng đều đạt);
  - lint 0 lỗi, typecheck 0 lỗi;
  - vitest 51 tệp / 281 test;
  - build 0 cảnh báo;
  - e2e 67 đạt / 7 bỏ qua.
- **Sau deploy:** health ok; long-poll bắt đầu và đọc được username bot. Từ 22:31 có 2 lần poll quá thời gian chờ (đường .65 tới Cloudflare), không có ERROR.
- **Chưa kiểm đầu cuối trên prod (chờ chủ sản phẩm):**
  - deep link, xác nhận liên kết trong chat, gửi biên bản;
  - app iPhone cần build lại để có `positionS` (đánh dấu khi ghi nền);
  - đánh dấu từ màn hình khoá hoặc AirPods để đợt sau.
- Docs: 4a435ce.
