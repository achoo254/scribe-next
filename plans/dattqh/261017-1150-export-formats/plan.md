---
title: Xuất phụ đề và tài liệu — VTT, Markdown, Word
status: completed
created: 2026-10-17
tier: M
---

# Xuất phụ đề và tài liệu

Gói P2 trong `docs/project-roadmap.md` (phần xuất tệp). Dịch Việt↔Anh **không** nằm trong gói này.

## Quyết định (Jev, chốt)
- Thêm `vtt`, `md`, `docx` vào `GET /api/transcripts/export` (`format=`), tạo ở máy chủ để web và iPhone nhận cùng một tệp.
- MD và DOCX là tài liệu cuộc họp: tiêu đề, ngày giờ và thời lượng, biên bản nếu đã tạo (tóm tắt, quyết định,
  việc cần làm kèm mốc), dấu mốc, rồi toàn bộ lời chép `[mm:ss]` (từ 1 giờ: `[h:mm:ss]`) theo câu đã sửa. Mục trống thì bỏ.
- DOCX tạo bằng Rust với `docx-rs` (MIT), tắt tính năng `image`; phải build được `x86_64-unknown-linux-musl` bằng zigbuild.
  Mốc thời gian chữ xám, phông Arial. Tên tệp UTF-8 (RFC 5987) theo tiêu đề.
- Giao diện: thay hai nút "Tải .txt / Tải .srt" bằng menu gọn "Tải xuống" (Radix dropdown) gồm TXT, SRT, VTT, Markdown,
  Word (.docx) — ở dòng lịch sử ghi/phiên và màn hình đọc bản chép lời.

## Phase
| Phase | Tier | Trạng thái |
|---|---|---|
| 1. Backend: định dạng mới + test Rust | M | done |
| 2. Web: menu "Tải xuống" + vitest + e2e | M | done |
| 3. Tài liệu, cổng kiểm tra, commit, push | S | done |

## Tiêu chí nhận
- `format=vtt|md|docx` trả đúng Content-Type và Content-Disposition; định dạng lạ → 400.
- VTT có `WEBVTT` đầu tệp, mốc dùng dấu `.`; MD đúng thứ tự mục; DOCX là zip hợp lệ có `word/document.xml` chứa chữ tiếng Việt.
- Menu "Tải xuống" ở dòng lịch sử và màn hình đọc; iPhone vẫn tải qua đường native (`lib/download.ts`).
- Cổng: clippy 0, cargo test, lint, typecheck, vitest, build, e2e; ghi kích thước binary musl trước/sau.

## Kết quả 17/10
- Tiêu đề tài liệu và tên tệp: tiêu đề người dùng đặt cho phiên, rồi tên client gửi (dòng lịch sử gửi tên file).
- Màn hình đọc: nút "Tải xuống" riêng cạnh nút đổi tên; menu ⋮ chỉ còn "Xoá". Trên điện thoại nút chỉ hiện biểu tượng.
- Binary musl release: 19.443.704 → 20.889.368 byte (+1,38 MiB; `docx-rs` kéo thêm `zip` 8, `quick-xml`, `encoding_rs`).
- Cổng: clippy 0 cảnh báo; cargo test 131 đạt (16 bỏ qua, cần Mongo/OpenAI); biome sạch; typecheck sạch; vitest 340/340; build đạt;
  e2e 85/85 (lần chạy đầu có test liên kết chat chờ popup quá 30 giây, chạy riêng 3/3 đạt, chạy lại cả bộ đạt).
- Chưa deploy.
