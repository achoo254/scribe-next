---
phase: 3
title: "Web/iPhone: tab Transcript, đọc, sửa, đổi tên, xoá"
status: done
priority: P1
effort: "M"
dependencies: [1]
---

# Phase 3: Web/iPhone — tab Transcript

<!-- Updated: Red Team 2026-10-16 - sửa trình phát (onEnded/onTimeUpdate, đổi đoạn sạch), gộp tải trùng + dọn cache iPhone, chép lại từng đoạn, poll nhẹ, tên mặc định có ngày, trạng thái "chưa chép" -->

## Goal

Tab Transcript dùng được trên màn hình iPhone: danh sách → màn hình đọc (chạm câu để tua, sửa, đổi tên, xoá, chép lại từng đoạn); web dùng chung mã và không vỡ giao diện.

## Files

- Create: `web/src/features/transcript-docs/transcript-docs-api.ts` (+ `.test.ts`) — các hàm gọi API, `fetcher` inject như `transcript-api.ts`
- Create: `web/src/features/transcript-docs/transcript-docs-queries.ts` — React Query; khi có đoạn `queued/running` chỉ poll `/status` mỗi 2 s, tải lại chi tiết khi một đoạn vừa sang `done`; chỉ poll khi tab Transcript đang mở
- Create: `web/src/features/transcript-docs/reader-model.ts` (+ `.test.ts`) — logic thuần: câu → (đoạn, giây trong đoạn), đoạn kế tiếp, câu đang phát theo thời gian, khoảng thiếu đoạn
- Create: `web/src/pages/transcript/transcript-page.tsx`, `transcript-list.tsx`, `transcript-reader.tsx` (+ test cho reader)
- Modify: `web/src/components/media-player.tsx` — export `AudioPlayer` (đang là hàm nội bộ `:116`) với prop `onEnded`, `onTimeUpdate`, `autoPlay`; giữ nguyên hành vi cho nơi đang dùng (`session-row.tsx:117-118`, `transcript-panel.tsx`)
- Modify: `web/src/lib/download.ts` — `useMediaSource` (`:55-70`) tính lại `src` ngay khi `fileId` đổi (web), không để một lượt render giữ `src` cũ
- Modify: `web/ios/App/App/NativeFilesPlugin.swift` — `cache` gộp các lượt tải cùng URL đang chạy (map Task theo key), ghi ra tệp tạm tên duy nhất rồi mới `moveItem` (`:27-47`); thêm `clear()` xoá thư mục `Caches/media`
- Modify: `web/src/lib/native.ts` — khai báo `clear()`; gọi khi đăng xuất
- Modify: `web/src/features/files/recording-groups.ts` — chuyển `startLabel` từ `web/src/pages/record/session-row.tsx:23-32` sang đây, thêm biến thể có ngày cho danh sách transcript; `session-row.tsx` dùng lại
- Modify: `web/src/layout/app-shell.tsx` — `AppTab` thêm `transcript`; bỏ `disabled` + badge "Sắp có"; chỉ hiện tab khi `me.canTranscribe` (`web/src/auth/auth-api.ts:6`); context `openTranscript(key)` để trang khác chuyển tab
- Modify: `web/src/pages/record/transcript-panel.tsx` — đã chép xong: nút "Mở transcript"; khi bấm Chép lại mà phiên có đoạn đã sửa: hộp xác nhận nêu **số đoạn và số câu đã sửa sẽ mất**

## Hành vi phát audio

- Reader dùng `AudioPlayer` với `key={fileId}` (gắn lại phần tử sạch khi đổi đoạn), `seek` đặt sau khi `src` mới sẵn sàng.
- Chạm câu: cùng đoạn → `seek`; khác đoạn → đổi `fileId` + `seek`.
- `onEnded` → sang đoạn kế (`autoPlay`); `onTimeUpdate` → tô câu đang phát và, khi còn ≤ 10 s, tải trước đoạn kế **một lần** (Set các `fileId` đã tải trước).
- Đoạn chưa chép / đang chép / lỗi: hiện dòng trạng thái thay cho câu, vẫn phát được audio. Đoạn thiếu: dòng "Thiếu đoạn N".
- Phát khi khoá màn hình **không** thuộc giai đoạn này (Jev, Validation Session 1).

## Chép lại và đoạn chưa chép

- Reader có nút "Chép lại đoạn này" trên từng đoạn (gọi `POST /api/transcripts` với một `fileId`, dùng lại `useRequestTranscripts`); đoạn có sửa → xác nhận trước.
- Phiên còn đoạn `none` (iPhone tải lên sau khi đã bấm chép): banner "Còn N đoạn chưa chép" + nút "Chép N đoạn còn lại" (chỉ người `canTranscribe`).

## UI (iPhone trước)

- Chạm mục tiêu ≥ 44 px; không bị Dynamic Island / home indicator che; chỉ dùng token (`npm run check:tokens`).
- Danh sách: tên (mặc định có ngày + giờ cho phiên iPhone, `item.name` cho bản ghi trình duyệt), thời lượng, trạng thái, preview.
- Lưu sửa: gửi kèm `runId` từng đoạn; 409 → "Đoạn này vừa được chép lại, tải lại để sửa tiếp".

## Verification

- `cd web && npm run lint && npm run typecheck && npm run check:tokens && npm test` sạch.
- Test mới: API (fakeFetch `web/src/test/fake-fetch.ts`); `reader-model` (offset cộng dồn, biên đoạn, đoạn cuối, khoảng thiếu); `AudioPlayer` gọi `onEnded`/`onTimeUpdate`; `useMediaSource` đổi `fileId` không trả `src` cũ; app-shell ẩn tab khi `canTranscribe=false`; hộp xác nhận chép lại đếm đúng.
- Test hồi quy nơi đang dùng player: Lịch sử ghi (`session-row`), panel chép lời.
- Chạy dev local trên Chrome khổ 390×844 và khổ desktop.
- Build iOS (`npm run ios:device`): tải trước + chuyển đoạn trên mạng chậm (Network Link Conditioner) không tải trùng, không lỗi phát.

### Kết quả (2026-10-16)

- `npm run lint` (0 lỗi), `typecheck`, `check:tokens`, `npm test` 168/168 (test mới: API, `reader-model`, `AudioPlayer` onEnded/onTimeUpdate, `useMediaSource` không trả `src` cũ — đã thử thấy fail trên mã cũ, app-shell ẩn/hiện tab, reader, hộp xác nhận chép lại).
- Build iOS cho simulator (`xcodebuild … CODE_SIGNING_ALLOWED=NO`): BUILD SUCCEEDED với `NativeFilesPlugin.swift` mới.
- QA dev local bằng Chromium headless khổ 390×844 + desktop 1280, dữ liệu QA 3 đoạn audio thật: 17/17 bước đạt (tab theo quyền, danh sách, chạm câu phát đúng đoạn lệch ≤ 1 s, tự sang đoạn kế, sửa → tải lại còn, xuất txt có bản sửa, đổi/xoá tên, xác nhận chép lại đoạn đã sửa, Mở transcript từ Lịch sử ghi, xoá transcript giữ audio, không lỗi JS, không cuộn ngang). Dữ liệu QA đã xoá.
- **Chưa làm, chuyển sang Phase 4:** cài lên iPhone thật và thử mạng chậm (Network Link Conditioner) — cần máy cắm vào Mac.

## Risks

- Tab Ghi âm vẫn `forceMount` và có panel tự poll `/api/transcripts` (`app-shell.tsx:67-72`); tab Transcript chỉ poll khi mở nên tổng request khi chép một phiên ≈ 30/phút/tab, dưới trần 300/phút/IP (`rate_limit.rs:19-23`).
