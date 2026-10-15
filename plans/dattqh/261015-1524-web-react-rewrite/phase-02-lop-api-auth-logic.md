---
phase: 2
title: "Lớp API, auth và logic nghiệp vụ"
status: pending
priority: P1
effort: M
dependencies: [1]
---

# Phase 2: Lớp API, auth và logic nghiệp vụ

## Overview

Chuyển toàn bộ logic của 8 composable Vue sang TypeScript thuần cộng hook React mỏng. Logic có trạng thái phức tạp (upload, recorder) nằm trong module thuần để test được và để Capacitor tái dùng sau này.

## Hợp đồng API giữ nguyên (đo từ `web/src`)

`/api/me`, `/api/me/usage`, `/api/auth/session`, `/api/auth/logout`, `/api/upload/init`, `/api/upload/chunk?session&offset`, `/api/upload/status?session`, `/api/upload/finalize`, `/api/recordings/init`, `/api/recordings/chunk?session&offset`, `/api/recordings/finalize`, `/api/files`, `/api/files/manage`, `/api/files/archive?from&to`, `/api/files/{id}` (GET stream, DELETE), `/api/files/{id}?download=1`, `/api/jobs`, `/api/jobs/{id}`, `/api/jobs/{id}/download`.

## Thiết kế

- `src/lib/api.ts`: một hàm `apiFetch` duy nhất. Đường dẫn bắt đầu bằng `/api/` được ghép với `API_BASE`: rỗng trên web (cùng origin), là `https://scribe.inetdev.io.vn` khi chạy native. Đây là điểm Gate 0 của plan iOS đang kiểm. Lỗi HTTP thành `ApiError { status, message }`.
- `src/lib/download.ts`: gom 5 chỗ `window.location.href` hiện tại (`use-recordings.ts:49,53,57`, `use-splitter.ts:113`, `use-managed-files.ts:56`) thành `download(url)` và `mediaUrl(fileId)`. Web: giữ hành vi hiện tại. Nhánh native để plan iOS cắm vào sau.
- Auth: `src/lib/firebase.ts` (Google sign-in popup như hiện tại) + `AuthProvider`/`useAuth` (đọc `/api/me` một lần, `login`, `logout`). Biến `VITE_FIREBASE_*` thiếu thì báo lỗi rõ lúc build.
- Dữ liệu danh sách (`/api/files`, `/api/files/manage`, `/api/me/usage`, job đang chạy) dùng TanStack Query như `localman-next`.
- `src/features/upload/chunked-upload.ts`: thuật toán upload theo phần + resume từ `use-upload.ts`, thuần TS, nhận `fetch` qua tham số.
- `src/features/recorder/recorder.ts`: chuyển nguyên thuật toán của `use-recorder.ts` (chọn mimeType, trộn Web Audio, MediaRecorder timeslice 1s, upload chunk theo offset, wake lock, `visibilitychange`, thông báo lỗi tiếng Việt) thành class thuần; `useRecorder` chỉ nối state vào React.
- `src/features/split/split-job.ts`: tạo job, poll trạng thái, tải kết quả (từ `use-splitter.ts`).

## Related code files

- Read: toàn bộ `web/src-vue-legacy/composables/*.ts`, `web/src-vue-legacy/lib/firebase.ts`
- Create: `web/src/lib/{api,download,firebase}.ts`, `web/src/auth/*`, `web/src/features/{upload,recorder,split,files,usage}/*`, kèm `*.test.ts`

## Success Criteria

- [ ] Test đơn vị: `apiFetch` ghép base đúng ở web và native; upload resume đúng offset sau khi `status` trả offset giữa chừng; recorder tính offset chunk tăng đơn điệu và retry không gửi trùng.
- [ ] Mọi endpoint trong danh sách trên được gọi đúng method, query và body như bản Vue (đối chiếu từng hàm).
- [ ] `lint`, `typecheck`, `test` EXIT 0.
