# Docs: refactor UI/UX theo hệ sinh thái hoandat

Plan: `plans/dattqh/261016-1426-ui-ux-hoandat-ecosystem-refactor/` (phase 1–5 xong, phase 6 chờ). Chưa commit.

## Đã sửa

- `docs/codebase-summary.md`: Frontend có thêm Router (các route, provider dưới `RequireAuth`, `PageError` nằm dưới shell, trang không lazy), Shell kiểu Localman (account menu với link IdP, xác nhận đăng xuất khi đang ghi/tải, footer có theme toggle), "Lời đã chép", i18n (từ điển `vi`, hai test bảo vệ), `lib/idp.ts` và `external-link.ts`, e2e (stub, hai project, ảnh chụp sáng/tối, axe, `check:dist`). Xoá dòng cũ "Be Vietnam Pro + shell 3 tab".
- `docs/ios-app.md`: thêm dòng Điều hướng (vuốt lùi trong `MainViewController.swift`, link IdP mở bằng trình duyệt hệ thống), thêm QA thật ngày 16/10 (đổi trang, vuốt lùi, khoá màn hình) và dòng QA cuối của phase 6 với trạng thái **Chưa**.
- `README.md`: `pnpm run test:e2e && pnpm run build` để chạy trước khi deploy, tách riêng khỏi phần CI vì CI không chạy e2e.
- `docs/project-roadmap.md`: dòng P0 đổi thành ✅, ghi rõ còn QA cuối trên iPhone; Trạng thái hiện tại ghi giao diện theo khung Localman; dòng refactor 07/12 được đánh dấu là đã bị thay thế.
- `docs/deploy-web-production.md`: thêm `VITE_IDP_ORIGIN` (tuỳ chọn); thêm điều kiện deploy web; lệnh rsync FE đổi thành: `assets/` chạy trước và không `--delete`, phần còn lại `--delete --exclude /assets/`. Đã thử cách này ở scratchpad: tệp asset cũ còn, tệp lạc ở gốc bị xoá.

## Bằng chứng

Các tệp đã đọc: `web/src/routes.tsx`, `web/src/layout/*`, `web/src/lib/idp.ts`, `web/src/i18n/*`, `web/src/i18n/messages/vi/{layout,transcript,errors}.ts`, `web/e2e/{playwright.config.ts,check-dist.ts,support/*}`, `web/package.json`, `web/.env.example`, `web/vite.config.ts`, `web/ios/App/App/MainViewController.swift`, `.github/workflows/ci.yml`, cùng `server/scribe-be/src/{recordings,upload,retention}.rs` (khi finalize thì doc phiên bị xoá). Mọi đường dẫn được nhắc trong docs đều có thật.

## Còn mở (ngoài phạm vi được sửa)

- `docs/deploy-web-production.md` còn lỗi gõ `ppnpm install` (mục CI và bước 2 của Deploy). Lỗi này không thuộc hai mục được phép sửa nên tôi chưa động vào.
- Skill `scribe-ops-prod` §6 vẫn ghi FE `rsync -a --delete`, trái với cách giữ `assets/` của plan. Cần cập nhật skill này (phase 6 có ghi việc đó).
- `phase-06` ghi lệnh `npm run ios:device`, nhưng repo đã chuyển sang pnpm.

Status: DONE_WITH_CONCERNS
Summary: Đã cập nhật 5 tài liệu cho refactor UI, mọi điều ghi vào đều đã đối chiếu với source. Còn lỗi gõ `ppnpm` trong tài liệu deploy và lệnh rsync cũ trong skill `scribe-ops-prod`, cả hai ngoài phạm vi được sửa.
