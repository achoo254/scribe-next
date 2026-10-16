---
phase: 2
title: "Web/iPhone: bỏ nhập key, hiện quota"
status: completed
priority: P1
effort: "S"
dependencies: [1]
---

# Phase 2: Web/iPhone — bỏ nhập key, hiện quota

## Goal

Người được chép lời bấm "Chép lời" là chạy, không hỏi key; luôn biết còn bao nhiêu phút và vì sao bị chặn.

## Files

- Delete `web/src/pages/record/api-key-modal.tsx`, `web/src/features/transcript/api-key.ts`; `use-transcribe.tsx` gọi thẳng `requestTranscripts(fileIds)`.
- Modify `web/src/features/transcript/transcript-api.ts` — bỏ `apiKey`; lỗi `QUOTA_EXCEEDED`/`CAP_REACHED` đọc thành thông báo có số.
- Modify `web/src/auth/auth-api.ts` — kiểu `transcription` trong `/api/me`; `auth-context` làm mới `me` sau mỗi lần xếp chép lời.
- Modify `transcript-panel.tsx`, `transcript-reader.tsx` — trước khi xếp, nếu `Σ durationS` > 10 phút: hộp xác nhận "Dùng khoảng N phút, còn M phút tháng này"; vượt hạn mức thì nút tắt kèm lý do.
- Create một dòng quota trong tab Transcript (đầu danh sách): "Còn N/120 phút tháng này · đặt lại ngày 1/11".
- Modify `auth-context.tsx` — đăng xuất vẫn gọi xoá key cũ trong Keychain một lần (`Secrets.remove`) để dọn key người dùng đã lưu; sau đó bỏ `SecretsPlugin` khỏi app khi không còn ai dùng (giữ plugin Swift tới Phase 6 để tránh build iOS lệch).
- Modify `docs/codebase-summary.md` phần web.

## Verification

- `npm run lint && npm run typecheck && npm run check:tokens && npm test`; test mới: không còn modal key; lỗi quota hiện số; dòng quota tính đúng; hộp xác nhận phiên dài.
- QA trình duyệt dev local 390×844 (Playwright headless như Phase 3 cũ): chép phiên ngắn, chạm hết hạn mức (đặt gói thử nhỏ bằng CLI dev).
