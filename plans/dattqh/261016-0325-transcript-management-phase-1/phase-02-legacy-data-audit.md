---
phase: 2
title: "Dữ liệu cũ: kiểm tra chỉ đọc trước deploy"
status: done
priority: P2
effort: "S"
dependencies: [1]
---

# Phase 2: Dữ liệu cũ — kiểm tra chỉ đọc trước deploy

<!-- Updated: Red Team 2026-10-16 - thu về chỉ đọc; bỏ xoá doc mồ côi; thêm kiểm doc kẹt running và _id không phải ObjectId -->

## Goal

Biết chắc dữ liệu `transcripts` trên prod tương thích code mới trước khi deploy, **không ghi gì** vào prod ở phase này.

## Files

- Create: `scripts/transcripts-audit.js` — script `mongosh` chỉ đọc, đếm:
  - doc theo `status`;
  - doc thiếu `runId` / `model` / `progress` / `userId` (thiếu `runId` thì worker coi như đã huỷ, `transcript.rs:96-103`);
  - doc `running` có `updatedAt` cũ hơn 1 giờ (kẹt sau `claim_next` `:84-93`);
  - doc có `_id` không phải ObjectId;
  - doc không còn file tương ứng (chỉ đếm, **không xoá**).

## Tasks

1. Chạy trên dev (lab 176) để thử script.
2. Chạy trên prod (chỉ đọc), ghi số liệu vào `## Kết quả`.
3. Nếu có doc lỗi: dừng, viết đề xuất xử lý kèm danh sách `_id` cụ thể và gửi Jev + người dùng duyệt riêng (ngoài phạm vi plan này).

## Verification

- Kết quả audit ghi trong `## Kết quả`; nếu mọi bộ đếm lỗi bằng 0 thì Phase 4 được deploy.

## Kết quả

2026-10-16, `scripts/transcripts-audit.js` qua `mongosh` (chỉ đọc):

| DB | Tổng `transcripts` | Thiếu runId/model/progress/userId | `running` kẹt > 1 giờ | `_id` không phải ObjectId | Mồ côi | `ok` |
|---|---|---|---|---|---|---|
| Dev (lab 176) | 0 | 0 / 0 / 0 / 0 | 0 | 0 | 0 | true |
| Prod (`.65`, chạy qua SSH) | 0 | 0 / 0 / 0 / 0 | 0 | 0 | 0 | true |

Prod có 54 bản ghi (`files`, đều `recording`) và chưa có lượt chép lời nào (`transcripts` rỗng, không có sự kiện `transcript.*` trong `audit_events`); collection `transcript_docs` chưa tồn tại (backend mới tạo khi khởi động). Mọi bộ đếm lỗi bằng 0 nên Phase 4 được deploy; không cần đề xuất xử lý dữ liệu.
