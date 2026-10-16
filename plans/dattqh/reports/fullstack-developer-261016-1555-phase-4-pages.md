# Phase 4: So sánh trang Scribe với pattern Localman

16/10/2026 (Asia/Saigon). Plan: `plans/dattqh/261016-1426-ui-ux-hoandat-ecosystem-refactor/phase-04-pages.md`.

## Cách đọc

- **Ảnh Scribe** là ảnh mốc Playwright trong `web/e2e/specs/*-snapshots/`, dữ liệu giả từ `e2e/support/api-stubs.ts`, đồng hồ cố định 16/10/2026. Tên tệp có dạng `<trang>-<light|dark>-<chromium-desktop|webkit-iphone>-darwin.png`.
- **Ảnh Localman** chụp từ prototype mà `localman-next/docs/design-system.md` dùng làm chuẩn đối chiếu (`localman-next/plans/dattqh/261008-1123-localman-core/prototype/index.html`). Localman không lưu ảnh mốc của trang app: e2e của nó chạy cả stack. Các ảnh này chỉ nằm trong scratchpad của phiên, mất khi phiên kết thúc: `/private/tmp/claude-501/-Users-hoandat-inet-gitlab-one-scribe/7af8b8e5-71e6-4066-8b09-60890f208a76/scratchpad/localman/`. Không lưu vào repo vì phase chỉ được sửa `web/` và tệp plan. Muốn chụp lại thì mở prototype tại `#/projects`, `#/account`, `#/login` ở viewport 1280×720 và iPhone 15.
- Mã nguồn Localman được đối chiếu: `localman-next/apps/web/src/pages/projects/{projects-page,projects-list,project-row,project-actions-menu,projects-skeleton}.tsx`, `pages/account/account-page.tsx`, `pages/project-detail/project-detail-header.tsx`, `components/layout/page-loading.tsx`; trang đăng nhập so với `hoandat-auth/src/interactions/login-page.ts`, `src/ui/layout.ts` và `static/css/app.css` (dòng 13–23 là lưới nền, 130–175 là `.auth` và `.auth-card`).

## Bảng so sánh

| Trang Scribe | Theo pattern Localman | Ảnh Scribe | Ảnh Localman (scratchpad) |
|---|---|---|---|
| `/login` | Trang đăng nhập hoandat-auth: lưới 100px từ `--c-grid`/`--grid-size`, thanh trên có viền dưới và nền `bg/82`, một card rộng 420px nằm cách thanh 48px, tiêu đề mono 22px, dòng phụ muted, nút block, ghi chú căn giữa | `web/e2e/specs/login.spec.ts-snapshots/login-{light,dark}-{chromium-desktop,webkit-iphone}-darwin.png` | `localman/login-{light,dark}-{chromium-desktop,webkit-iphone}.png` |
| `/record` (web) | Đầu trang như `/account` và `/projects` (h1 mono `text-xl` trên hàng cao 44px, dòng mô tả muted); khối nội dung là `Card` bo 24px; hàng lịch sử có nút ⋮ như `project-actions-menu` | `web/e2e/specs/record.spec.ts-snapshots/record-{light,dark}-{chromium-desktop,webkit-iphone}-darwin.png` | `localman/projects-*.png`, `localman/account-*.png` |
| `/record` (app iPhone, stub Capacitor) | Cùng đầu trang, phần mô tả dành cho iPhone; card ghi native | `web/e2e/specs/record.spec.ts-snapshots/record-native-{light,dark}-webkit-iphone-darwin.png` | `localman/projects-*-webkit-iphone.png` |
| Menu ⋮ của một bản ghi | `ProjectActionsMenu`: `IconButton` ⋮ (44px trên màn cảm ứng), `DropdownMenuItem` có icon, tách mục nguy hiểm bằng separator | `web/e2e/specs/pages.spec.ts-snapshots/record-row-menu-{light,dark}-{chromium-desktop,webkit-iphone}-darwin.png` | `localman/projects-*.png` (nút ⋮ ở cuối mỗi hàng) |
| `/split` | Đầu trang như trên; một `Card` gồm ba bước có đánh số; `Select` native (`components/ui/select.tsx`, chép từ Localman) để chọn tiêu chí; card "Tệp trên hệ thống" có menu ⋮ cho từng tệp | `web/e2e/specs/pages.spec.ts-snapshots/split-{light,dark}-{chromium-desktop,webkit-iphone}-darwin.png` | `localman/account-*.png` |
| `/transcripts` | `/projects`: hàng là thẻ `cardClassName`, viền chuyển sang primary 45% khi hover hoặc focus; skeleton lặp lại hình hàng thật; lỗi có nút "Thử lại"; trạng thái rỗng có hành động | `web/e2e/specs/pages.spec.ts-snapshots/transcripts-{light,dark}-{chromium-desktop,webkit-iphone}-darwin.png` | `localman/projects-*.png` |
| `/transcripts/:key` | `project-detail-header`: breadcrumb "Transcript / <tên>" (phần tử cha màu muted, h1 có `aria-current="page"`), hành động nằm cuối hàng; tải .txt, .srt và xoá nằm trong menu ⋮ | `web/e2e/specs/pages.spec.ts-snapshots/reader-{light,dark}-{chromium-desktop,webkit-iphone}-darwin.png`, menu: `reader-menu-*.png` | `localman/projects-*.png` (Localman không có trang chi tiết trong prototype; xem `project-detail-header.tsx`) |

## Khớp và chỗ khác

- **Đầu trang**: h1 mono đậm `text-xl`, nằm trên hàng `min-h-11`, giống `/account` của Localman; tiêu đề dùng JetBrains Mono như Localman, không dùng Inter. Riêng Scribe có thêm một dòng mô tả muted dưới tiêu đề, đúng như `PageLoading` của Localman dự kiến (tiêu đề, mô tả, rồi các thẻ).
- **Hành động chính**: Localman đặt nút "+" ngay cạnh tiêu đề vì nút đó đứng riêng, không gắn với form nào. Ở Scribe, nút "Bắt đầu ghi" phụ thuộc các công tắc nguồn còn nút "Cắt" phụ thuộc tiêu chí, nên chúng nằm trong card, ngay dưới phần điều khiển của mình. Chỉ trang đọc transcript có hành động cấp trang (đổi tên và menu ⋮), và chúng nằm trên hàng tiêu đề.
- **Thanh dung lượng** vẫn nằm phía trên tiêu đề trang (quyết định ở Phase 2). Localman không có thanh nào tương ứng.
- **Lưới nền**: Localman vẽ lưới cho cả `body`; Scribe chỉ vẽ ở trang đăng nhập, đúng phạm vi được giao. Đây là câu hỏi mở bên dưới.
- **Không gộp hai widget dung lượng**: `QuotaBar` hiện dung lượng lưu trữ tính bằng byte (`GET /api/me/usage`), còn `TranscriptionQuota` hiện số phút chép lời của tháng cùng trạng thái tạm ngưng hoặc hết hạn mức (`user.transcription` từ `/api/me`). Hai nguồn dữ liệu, hai đơn vị khác nhau. `TranscriptionQuota` chỉ được bọc trong một `Card`.

## Câu hỏi mở (gửi Jev)

1. Có vẽ lưới 100px cho mọi trang như `body` của Localman không? Hiện chỉ trang đăng nhập có. Nếu bật cho toàn app thì mọi ảnh mốc sẽ đổi.
2. `pnpm run test:e2e` hỏng từ commit `05c8e77` (chuyển npm sang pnpm): shim `.bin/playwright` trỏ vào `playwright/cli.js`, nên mọi spec báo "did not expect test.describe()". Chạy `node "$(pnpm root)/@playwright/test/cli.js" test --config e2e/playwright.config.ts` thì đạt. Có sửa script `test:e2e` theo cách này không? Chi tiết ở mục Kết quả của phase 4.
