---
title: "Refactor UI/UX Scribe theo format hệ sinh thái hoandat"
description: "Chép khung, primitive và i18n của Localman sang Scribe; điều hướng bằng URL giữ trạng thái ghi; dựng lại các trang; Playwright + ảnh chụp sáng/tối"
status: pending
priority: P1
effort: L
created: 2026-10-16
---

# Refactor UI/UX Scribe theo format hệ sinh thái hoandat

## Overview

Người dùng: "công việc này chắc chắn phải làm", mọi đề xuất/câu hỏi mở giao Jev quyết (16/10). Làm trước khi tiếp lộ trình (realtime iPhone).

Khảo sát 16/10 (hai báo cáo Explore, đo lại bằng `diff`):
- Chuẩn hệ sinh thái là **web Localman** (`localman-next/apps/web`): token `localman-next/packages/design-tokens/tokens.css` được **chép nguyên tệp** sang auth, chat, Scribe; không có package dùng chung.
- Scribe đã cùng stack (React 19, Tailwind 4, Radix, Tabler, Inter + JetBrains Mono). `tokens.css` trùng giá trị; `button`, `card`, `switch`, `input`, `empty-state` trùng từng dòng; `modal`/`toast`/`tabs` lệch 2–9 dòng. Thiếu `dropdown-menu`, `select`, `textarea`, `route-progress`, `copy-field`.
- Chỗ lệch thật nằm ở **khung và cách ghép trang**: Scribe một cột 920 px, top bar "wordmark + email + theme + Đăng xuất", tab trong trang (force-mount để giữ phiên ghi), không URL theo tab, không footer, không account menu/link ứng dụng, không i18n, không e2e. Localman: top bar sticky (wordmark → home, nav ghost có trạng thái active, account menu avatar có link trang tài khoản/ứng dụng của IdP, đăng xuất trong menu), footer (language + theme), skip link, route progress, container 1120 px, i18n, Playwright.

## Quyết định (Jev, 16/10 — người dùng uỷ quyền)

| Điểm | Chọn | Xác suất |
|---|---|---|
| Cách làm | Chép khung + primitive thiếu + i18n của Localman (quy ước copy) | 1,00 |
| Điều hướng | URL thật cho từng trang, nav trên top bar; trạng thái ghi/cắt/transcript đưa lên provider cấp app | 0,85 |
| i18n | Hạ tầng i18n + từ điển tiếng Việt ngay; en/zh sau | 0,47 (đủ 3 ngôn ngữ 0,21; không 0,32) |
| iPhone | Cùng khung, co giãn, safe area; không bottom tab | 0,99 |
| Tách component lớn | Để sau (chỉ chạm phần cần) | 0,78 |
| Test trình duyệt | Playwright + ảnh chụp sáng/tối | 0,89 |
| Phát hành | Deploy prod sau mỗi phase đạt | 0,68 |
| Router + thanh tiến độ | `createBrowserRouter` ở Phase 3, route-progress thêm ở đó (Phase 2 không có) | 0,98 |
| Mục "Ứng dụng" trong account menu | `/account#account-apps` của IdP, như Localman | 0,48 (hai mục 0,40; `/apps` 0,12) |
| Quay lại trên iPhone | Nút quay lại trong trang + bật cử chỉ vuốt lùi native (cài lại ở Phase 3) | 0,57 |

## Phases

| # | Phase | Tier | Status | Phụ thuộc |
|---|---|---|---|---|
| 1 | [Nền: sync primitive, i18n, Playwright](./phase-01-foundation.md) | M | Pending | — |
| 2 | [Khung app: top bar, account menu, footer, layout](./phase-02-app-shell.md) | M | Pending | 1 |
| 3 | [Điều hướng URL + provider giữ trạng thái](./phase-03-routes-and-state.md) | L | Pending | 2 |
| 4 | [Dựng lại các trang theo pattern Localman](./phase-04-pages.md) | L | Pending | 3 |
| 5 | [Từ điển tiếng Việt, rà chuỗi, a11y](./phase-05-strings-a11y.md) | M | Pending | 4 |
| 6 | [QA iPhone + đóng plan](./phase-06-iphone-qa-close.md) | S | Pending | 5 |

## Bất biến (mọi phase)

- Không đổi API backend, sổ phút, đăng nhập; chỉ `web/`.
- **Không mất phiên ghi**: đang ghi (web hoặc native) mà chuyển trang, khoá màn hình, đổi theme thì phiên vẫn chạy và tải lên đủ.
- `npm run lint`, `typecheck`, `check:tokens`, `npm test`, Playwright đạt trước mỗi deploy; ảnh chụp chỉ cập nhật có chủ đích.
- Chép từ Localman thì ghi nguồn ở đầu tệp (như `tokens.css`); sửa tại chỗ chỉ khi Scribe cần, ghi lý do.
- Deploy prod theo skill `scribe-ops-prod` §6 (web: `npm run build` → rsync dist; backend không đổi). App iPhone nhận giao diện mới khi cài lại qua cáp (Phase 3 và 6); app đang cài mang bản build riêng, không bị deploy web làm hỏng.
- **Mỗi lần deploy web**: chỉ khi không có phiên ghi đang mở (`recording_sessions` chưa finalize) và hàng đợi chép lời rỗng; giữ `assets/` của bản trước (rsync không `--delete` cho `assets/`, dọn tệp cũ ở lần deploy sau) để tab đang mở không 404.
- Stub API của Playwright và stub Capacitor chỉ nằm trong `web/e2e/`; build prod kiểm không chứa chúng (grep `dist/`).
- Red-team 16/10: `plans/dattqh/reports/code-reviewer-261016-1430-ui-refactor-red-team.md` (1 Critical, 7 High, đã áp vào các phase).

## Non-goals

Đổi token/màu của hệ sinh thái; tách package dùng chung; tiếng Anh/Trung (sau); bottom tab; tách `transcript-reader` (467 dòng) và các component 250+ dòng trừ phần buộc phải chạm; tính năng mới.

## Success Criteria

- Top bar, account menu, footer, độ rộng, trạng thái loading/empty/toast giống Localman (so ảnh chụp cạnh nhau ở Phase 4).
- `/record`, `/split`, `/transcripts`, `/transcripts/:key` có URL, nút back hoạt động; chuyển trang khi đang ghi không mất phiên (test tự động + thử tay).
- Không còn chuỗi tiếng Việt viết cứng trong `src/` ngoài từ điển (test dictionaries như Localman).
- Playwright: mọi trang render ở light/dark, desktop + viewport iPhone, axe không lỗi nghiêm trọng.
- iPhone thật: đăng nhập, ghi khi khoá màn hình, tải lên, chép lời, mở transcript chạy như trước.

## Rủi ro chính

| Rủi ro | Giảm thiểu |
|---|---|
| Đưa trạng thái ghi lên provider làm rơi phiên đang ghi | Phase 3: Playwright với thiết bị media giả đếm request chunk qua các lần đổi route; vitest cho native recorder; iPhone thật; deploy khi không ai đang ghi |
| Route-progress của Localman cần data router (`useNavigation`), Scribe đang dùng `<BrowserRouter>` (`main.tsx:15`) ⟹ trắng trang | Không chép ở Phase 1–2; Phase 3 chuyển `createBrowserRouter` rồi mới thêm |
| Đăng xuất khi đang ghi (mic còn mở web; native dừng phiên không hỏi `RecorderPlugin.swift:194-197`) | Provider đặt dưới `RequireAuth`; menu đăng xuất hỏi xác nhận khi đang ghi hoặc còn đoạn chờ tải |
| Chép Localman kéo theo phụ thuộc thừa (workspace, notifications, feature flags) | Chỉ chép phần cần; gỡ import không dùng; lint/typecheck sạch |
| Ảnh chụp Playwright không ổn định (font, animation) | Font tự host, `prefers-reduced-motion`, chờ mạng rỗi; ngưỡng diff nhỏ |
| App iPhone còn bản cũ trong lúc web đã đổi | Bản cũ vẫn nói chuyện cùng API (API không đổi); cài lại ở Phase 3 và 6 |

## Validation Log

- 16/10 14:2x: người dùng "chắc chắn phải làm", uỷ quyền Jev cho mọi đề xuất/câu hỏi mở. 7 quyết định khung + 3 quyết định sau red-team ở bảng trên.
- Red-team (4 góc nhìn, đối chiếu code): C1 route-progress cần data router → dời Phase 3 cùng `createBrowserRouter`; H1 hai lượt cắt độc lập; H2 trạng thái tải lên vào provider; H3 provider dưới `RequireAuth` + xác nhận đăng xuất khi đang ghi; H4 i18n chép đủ `translate.ts`/`message-types.ts`, gốc kiểu là `vi`; H5 `useBlocker` cho ô transcript chưa lưu; H6 nút quay lại + bật cử chỉ vuốt lùi; H7 Playwright media giả đếm chunk. Medium/Low đã áp (phụ thuộc ẩn của Localman, hằng `VITE_IDP_ORIGIN`, stub Capacitor cho nhánh native, giữ `assets/` cũ khi deploy, điều kiện deploy mọi phase, khoá có `:`, bỏ `beforeunload`, không chép primitive đã có).
- Lộ trình sau refactor (người dùng đồng ý 16/10 14:40): gói "Sau cuộc họp", "Kho bản ghi", xuất phụ đề/tài liệu, dịch, phân biệt người nói — `docs/project-roadmap.md`.
