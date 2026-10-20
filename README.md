# Scribe

Ghi âm, cắt file audio/video và chép lời tiếng Việt. Repo `scribe-next`, một app trong hệ sinh thái hoandat (cùng Chat, Localman).

Hai ứng dụng dùng chung một backend:

| App | Vị trí | Stack |
|-----|--------|-------|
| **Web** | `web/` | React 19 + Vite 7 + Tailwind 4 + TanStack Query |
| **iPhone** | `web/ios/` | Capacitor 8 đóng gói bản build của `web/` + plugin Swift (ghi nền khi khoá màn hình, tải lên dần) |
| Backend | `server/scribe-be/` + `crates/core-split/` | Rust/axum, một binary musl tĩnh, MongoDB |

- **Bundle id iPhone**: `com.hoandat.scribe`
- **Brand**: tên, màu, icon, bundle id, URL scheme, tenant chính và mẫu host nằm ở một file `web/brand.config.json` (schema `web/brand.config.schema.json`). Sửa file đó (hoặc `web/brand/icon*.svg`) rồi chạy `cd web && pnpm run brand:gen` để sinh lại `web/brand/generated/` (web manifest, `capacitor.json`, icon web; web manifest và icon PNG/ICO chưa được phục vụ, web chỉ dùng `/favicon.svg` lấy từ icon brand). `pnpm run lint` chặn tên sản phẩm viết cứng (`check:brand`) và mã màu ngoài `web/src/styles/tokens.css` (`check:tokens`). Icon app iPhone (`Assets.xcassets`) và tên trong `Info.plist` giữ như bản đã phát hành, không sinh.
- **Tính năng**: ghi mic và màn hình/tiếng hệ thống (trình duyệt), ghi khi khoá màn hình (iPhone), cắt file theo phút / dung lượng / số phần, lịch sử ghi và phát lại, chép lời bằng OpenAI `gpt-4o-mini-transcribe` (key của server, tính phút theo gói).
- **Đăng nhập**: auth-next (`auth.hoandat.com`) cho cả web và iPhone.
- **Môi trường**: dev local và prod (`https://scribe.hoandat.com`). Không có staging.

## Chạy dev

```bash
# Backend (cần .env.development ở gốc repo, xem docs/deploy-web-production.md)
set -a; . ./.env.development; set +a
cargo run -p scribe-be                 # → 127.0.0.1:8787

# Web (cần web/.env, mẫu ở web/.env.example)
cd web && pnpm install && pnpm run dev   # → http://localhost:5173, proxy /api → backend
```

## Kiểm tra

```bash
cd web && pnpm run lint && pnpm run typecheck && pnpm test   # lint gồm check:brand và check:tokens
cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

CI GitHub Actions (`.github/workflows/ci.yml`) chạy đúng các lệnh này cho mỗi push `main` và pull request. Deploy prod làm tay.

Ngoài CI, chạy thêm trước khi deploy web:

```bash
cd web && pnpm run test:e2e && pnpm run build
```

`test:e2e` là bộ Playwright trong `web/e2e/` (stub API và Capacitor, không cần backend; ảnh chụp sáng/tối trên Chromium desktop và WebKit iPhone, kiểm axe). `build` kết thúc bằng `check:dist`, báo lỗi nếu stub của e2e lọt vào `dist/`.

## Cài app lên iPhone

```bash
cd web && pnpm run ios:device
```

Chi tiết: [docs/ios-app.md](./docs/ios-app.md).

## Tài liệu

- [docs/codebase-summary.md](./docs/codebase-summary.md) — kiến trúc, module backend/frontend, API, MongoDB, storage
- [docs/project-roadmap.md](./docs/project-roadmap.md) — lộ trình, trạng thái, backlog
- [docs/deploy-web-production.md](./docs/deploy-web-production.md) — build và deploy prod
- [docs/ios-app.md](./docs/ios-app.md) — app iPhone
