---
phase: 4
title: "Dựng lại các trang theo pattern Localman"
status: completed
priority: P1
effort: "L"
dependencies: [3]
---

# Phase 4: Dựng lại các trang

## Pattern áp dụng (nguồn `localman-next/apps/web` + `docs/design-system.md`)
- Tiêu đề trang + mô tả ngắn + hành động chính; card bo 24 (`Card`), dùng `Card` thay class inline (4 chỗ); empty state (`empty-state.tsx`); loading bằng `use-delayed-flag` + skeleton; toast cho kết quả; lỗi inline trong form.
- Bảng/danh sách: hàng có hành động bằng `dropdown-menu` thay dãy nút khi chật (iPhone).

## Trang
- **Đăng nhập** (`pages/login-page.tsx`): theo trang đăng nhập hoandat-auth (card giữa, nền grid như `hoandat-auth/static/css/app.css` 13–23 nếu tái dùng được bằng token), một nút "Đăng nhập bằng hoandat", lỗi từ `login-error.ts`.
- **Ghi âm** (`pages/record/*`): card ghi (nguồn, nút ghi lớn, đồng hồ), lịch sử ghi dạng danh sách có menu hành động.
- **Cắt file** (`pages/split/*`): bước chọn tệp → tiêu chí → kết quả; `select` cho tiêu chí.
- **Transcript** (`pages/transcript/*`): danh sách + trang chi tiết `/transcripts/:key`; gộp hai widget quota (`components/quota-bar.tsx`, `pages/transcript/transcription-quota.tsx`) thành một nếu cùng dữ liệu (đọc cả hai trước); chỉ đổi khung/khoảng cách của `transcript-reader.tsx`, không tách file (non-goal).
- Chữ mono chỉ cho wordmark/badge/tab theo Localman; tiêu đề trang dùng Inter nếu Localman làm vậy (so ảnh).

## Verification
- Playwright ảnh chụp từng trang light/dark, desktop + iPhone, đặt cạnh ảnh Localman tương ứng trong báo cáo `plans/dattqh/reports/`; axe.
- vitest các trang chạm tới; deploy web prod sau mỗi nhóm trang (đăng nhập + ghi; cắt; transcript) nếu muốn chia nhỏ.

## Kết quả (16/10)

Đã làm xong trong `web/`, chưa commit, chưa deploy. API, các provider (`record-state.tsx`, `split-state.tsx`) và nhánh `isNativeShell` không đổi. Báo cáo so ảnh cạnh nhau: `plans/dattqh/reports/fullstack-developer-261016-1555-phase-4-pages.md`.

**Phần dùng chung (mới)**
- `layout/page-header.tsx`: h1 mono `text-xl` trên hàng `min-h-11`, có thể kèm breadcrumb "Cha / tên" (h1 mang `aria-current="page"`, xuống dòng được trên điện thoại), một dòng mô tả muted và các hành động ở cuối hàng. Dùng ở Ghi âm, Cắt file, danh sách và trang đọc transcript.
- `components/ui/load-error.tsx`: Alert danger có tiêu đề kèm nút "Thử lại" (gọi `refetch`, chỉ chạy lại GET), theo cách báo lỗi danh sách của Localman. Dùng ở lịch sử ghi, tệp trên hệ thống, danh sách và trang đọc transcript.
- `components/ui/row-actions-menu.tsx`: nút ⋮ (`IconButton`, 44px trên màn cảm ứng) mở `DropdownMenu`, như `project-actions-menu`.
- `pages/record/record-timer.tsx`: "● Đang ghi — m:ss", dùng chung cho bản web và bản iPhone.
- `styles/global.css`: utility `grid-backdrop`, vẽ lưới 100px bằng `--c-grid` và `--grid-size` có sẵn, không thêm token. `check:tokens` sạch.

**Trang**
- Đăng nhập: bố cục như hoandat-auth. Lưới nền; thanh trên có wordmark và theme toggle, viền dưới, nền `bg/82`; card rộng 420px nằm cách thanh 48px; tiêu đề mono 22px "Đăng nhập", một dòng phụ, nút "Đăng nhập bằng hoandat", lỗi từ `login-error.ts`, ghi chú căn giữa.
- Ghi âm: đầu trang có mô tả riêng cho web và cho iPhone. Card ghi gồm nguồn, hướng dẫn, cắt theo phút; dưới một đường kẻ là nút ghi lớn, đồng hồ và các trạng thái. Hàng trong lịch sử: badge nằm cùng dòng với tên; "Nghe lại" vẫn hiện; "Tải về / Lưu vào Tệp" và "Xoá" chuyển vào menu ⋮. Trên điện thoại, "Nghe lại" và "Xem các đoạn" chỉ còn icon (nhãn vẫn là tên truy cập). Lần tải đầu dùng `useDelayedFlag` cùng skeleton có hình như hàng thật. Lỗi tải có nút "Thử lại". Xoá xong hiện toast.
- Cắt file: một card gồm ba bước có đánh số (Chọn file, Cách cắt, Kết quả). Tiêu chí chọn bằng `Select` ("Cắt theo": thời lượng, dung lượng, số phần bằng nhau) thay cho segmented control. Tệp đã tải lên có menu ⋮ (Tải về, Xoá). Lỗi có nút "Thử lại". Xoá xong hiện toast.
- Danh sách transcript: đầu trang, card hạn mức phút, các hàng là thẻ `cardClassName` có viền primary khi hover hoặc focus như `/projects`, skeleton theo hình hàng, lỗi có nút "Thử lại". Trạng thái rỗng có link "Mở trang Ghi âm" và câu chữ "trang Ghi âm" (trước đây là "tab Ghi âm").
- Trang đọc transcript: chỉ đổi khung và khoảng cách, không tách tệp (non-goal; tệp tăng từ 475 lên 529 dòng). Có breadcrumb "Transcript / <tên>" (nút về danh sách vẫn mang `data-id="reader-back"`), dòng thông tin, nút Đổi tên, menu ⋮ (Tải hoặc Lưu .txt/.srt, Xoá transcript). Form đổi tên nằm trong một card riêng. Phần câu và player nằm trong card nội dung. Skeleton hiện sau thời gian trễ; lỗi có nút "Thử lại"; đổi tên hoặc xoá xong hiện toast.
- Hai widget dung lượng **không gộp**: `QuotaBar` hiện số byte lưu trữ (`/api/me/usage`), `TranscriptionQuota` hiện số phút chép lời (`user.transcription`). Khác dữ liệu, khác đơn vị.
- "4 chuỗi class card inline" trong khảo sát ban đầu đã được thay bằng `Card` từ trước phase này. Chỗ inline còn lại là hàng của danh sách transcript, nay dùng `cardClassName`. Các khung con nằm bên trong card (live monitor, transcript panel, "Thiếu đoạn") vẫn giữ class riêng vì chúng không phải card.
- iPhone: tên dài ở hàng ghi và hàng tệp trước đây đẩy card rộng tới 494px trên màn 393px, vì các lưới bao quanh lấy độ rộng theo nội dung. Cột thông tin của hàng giờ có `contain-inline-size` nên tên được cắt ngắn. Playwright kiểm `scrollWidth <= innerWidth` ở mọi trang.

**i18n**: các chuỗi của những phần đã chạm tới nằm ở `messages/vi/{login,record,split,transcript}.ts`, cùng các khoá mới `common.actions.{cancel,delete,retry}` và `layout.breadcrumb`. Chuỗi trong thân trang đọc (từng câu, player, các hộp xác nhận chép lại), `transcript-panel`, `record-sources`, `split-after-recording` và `live-monitor` để Phase 5. Test từ điển đạt (mọi khoá đều được dùng, không có khoá thừa).

**Kiểm tra** (máy dùng chung, chạy lần lượt từng việc; trong lúc chạy, session khác ở localman-next và hoandat-auth cũng chạy vitest, vue-tsc và Playwright)
- lint: `Checked 199 files … No fixes applied.` typecheck (src, scripts, e2e): 0 lỗi. check:tokens: không có literal, hai khối dark giống nhau, 82 cặp tương phản đạt, 9 cảnh báo APCA đã có từ trước.
- vitest `--maxWorkers=2`: 47 tệp, 237 test đạt. Mới: `recording-history.test` (xoá qua menu ⋮, hộp xác nhận, toast; thử lại khi lỗi), `split-criteria.test` (select, nhãn và lỗi theo tiêu chí), `transcript-list.test` (h1, link khi rỗng, mở hàng, thử lại khi lỗi), và một test mới trong `transcript-reader.test` (breadcrumb, menu ⋮, xoá kèm toast, `onBack({ replace: true })`). Đã thử phá: bỏ toast xoá thì test lịch sử fail.
- build: `✓ built in 2.12s`, không cảnh báo. `check:dist`: không có stub trong 6 tệp.
- e2e: 54 đạt, 4 bỏ qua có chủ đích. Test ghi âm qua các route trên Chromium đạt. Spec mới `pages.spec.ts` chụp Cắt file (sau khi tải lên, có tệp trên hệ thống), danh sách, trang đọc, menu ⋮ của trang đọc và menu ⋮ của một bản ghi, ở sáng và tối, trên Chromium desktop và WebKit iPhone; axe sạch (khi menu mở thì chỉ quét menu, như Phase 2). `routes.spec.ts` lấy fixture transcript từ `support/transcript-fixtures.ts`.
- Ảnh mốc: 20 ảnh mới (`pages.spec.ts-snapshots/`), 16 ảnh cập nhật có chủ đích. Gồm `login-*` ×4 (bố cục hoandat-auth), `record-*` ×4 và `record-native-*` ×2 (đầu trang, card, hàng có menu), `account-menu-*` ×4 (ảnh chụp cả viewport nên thấy trang Ghi âm mới phía sau), `footer-*-chromium-desktop` ×2 (footer không đổi; chữ lệch dưới 1px vì trang phía trên cao hơn). Ảnh `topbar-*` và `footer-*-webkit-iphone` không đổi.

**Còn mở**
- `pnpm run test:e2e` không chạy được: mọi spec báo "Playwright Test did not expect test.describe() to be called here", kể cả một spec rỗng. Nguyên nhân là commit `05c8e77 build(web): switch from npm to pnpm` (15:59, không thuộc phase này): dưới pnpm, shim `.bin/playwright` trỏ vào `playwright/cli.js`. Chạy bằng CLI của `@playwright/test` thì đạt: `node "$(pnpm root)/@playwright/test/cli.js" test --config e2e/playwright.config.ts`. Mọi kết quả e2e ở trên đều chạy theo cách này. CI không chạy e2e (`ci.yml` chỉ có lint, typecheck, check:tokens, test) nên CI vẫn xanh. Phase này không sửa script `test:e2e` trong `package.json`, vì đó là quyết định của người làm việc chuyển.
- Chưa deploy, chưa thử trên iPhone thật. Lưới nền mới chỉ có ở trang đăng nhập; vẽ cho cả app như Localman hay không là việc chờ Jev quyết.

- Jev 16/10: sửa `test:e2e` cho pnpm ngay (0,81) — gọi CLI của `@playwright/test`; lưới nền chỉ ở trang đăng nhập (0,64; cả app 0,36).

- Deploy web prod 2026-10-16 16:43: điều kiện đạt, sao lưu `/root/scribe-backup-20261016-164319/web`, prod `index-DVxZRNcB.js`, `/login` `/record` `/split` `/transcripts` 200.
