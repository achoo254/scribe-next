# Scribe

Ghi âm, cắt file audio/video và chép lời tiếng Việt. Repo `scribe-next`, một app trong hệ sinh thái hoandat (cùng Chat, Localman).

Các ứng dụng dùng chung một backend:

| App | Vị trí | Stack |
|-----|--------|-------|
| **Web** | `web/` | React 19 + Vite 7 + Tailwind 4 + TanStack Query |
| **iPhone** | `web/ios/` | Capacitor 8 đóng gói bản build của `web/` + plugin Swift (ghi nền khi khoá màn hình, tải lên dần) |
| **Android** | `web/android/` | Capacitor 8, cùng bản build `web/` + plugin Kotlin cùng contract JS với iOS (chưa phát hành, chưa kiểm trên máy thật — [docs/android-app.md](./docs/android-app.md)) |
| Backend | `server/scribe-be/` + `crates/core-split/` | Rust/axum, một binary musl tĩnh, MongoDB |

- **Bundle id iPhone / applicationId Android**: `com.hoandat.scribe`
- **Brand**: tên, màu, icon, bundle id, URL scheme, tenant chính và mẫu host nằm ở một file `web/brand.config.json`, kiểm theo schema của gói dùng chung `@shared-next/brand` (`github:achoo254/shared-next#v0.2.0&path:/packages/brand`, repo `shared-next`; mã thương hiệu chỉ sửa ở đó rồi nâng tag). Sửa file đó (hoặc `web/brand/icon*.svg`) rồi chạy `cd web && pnpm run brand:gen` (`web/scripts/brand-gen.ts`, chạy bằng `tsx`) để sinh lại `web/brand/generated/` (web manifest, `capacitor.json`, icon web vẽ bằng Chromium của `@playwright/test`; icon launcher, màu splash của app Android và ảnh Play ở `web/brand/generated/play/` — `web/scripts/brand-android.ts`). Web phục vụ `/manifest.webmanifest`, `/favicon.svg` (icon brand), `/favicon.ico`, `/apple-touch-icon.png` và icon PNG thường + maskable ở gốc site, link sẵn trong `index.html`, nên cài được lên màn hình chính như app (mở `/`, không service worker; trên iPhone app cài từ Safari có cookie riêng nên phải đăng nhập lại một lần). `pnpm run lint` chặn tên sản phẩm viết cứng (`check:brand`, CLI `check-brand` của gói: cấm tên cũ "OneScribe" ở mọi file và `appName` là một từ trong HTML, dùng `%APP_NAME%`) và mã màu ngoài `web/src/styles/tokens.css` (`check:tokens`). Icon app iPhone (`Assets.xcassets`) và tên trong `Info.plist` giữ như bản đã phát hành, không sinh.
- **Tính năng**: ghi mic và màn hình/tiếng hệ thống (trình duyệt), ghi khi khoá màn hình (iPhone), cắt file theo phút / dung lượng / số phần, lịch sử ghi và phát lại, chép lời bằng OpenAI `gpt-4o-mini-transcribe` (key của server, tính phút theo gói).
- **Đăng nhập**: auth-next (`auth.hoandat.com`) cho web, iPhone và Android.
- **Môi trường**: dev local và prod (`https://scribe.hoandat.com`). Không có staging.

## Chạy dev

```bash
# Env nằm trên iCloud, không trong repo: scripts/env-dir.sh (xem docs/deploy-web-production.md)
set -a; . "$(scripts/env-dir.sh)/.env.development"; set +a
cargo run -p scribe-be                 # → 127.0.0.1:8787

# Web (đọc web/.env trong thư mục env trên iCloud, mẫu ở web/.env.example)
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

## Cài app lên Android

```bash
cd web && pnpm run android:device
```

Cần JDK 21 + Android SDK. CI build APK debug ở mỗi push (artifact `scribe-android-debug-apk`); AAB để lên Play do
workflow `android-release.yml` ký bằng upload key trong GitHub secrets. Chi tiết: [docs/android-app.md](./docs/android-app.md).

## Tài liệu

- [docs/codebase-summary.md](./docs/codebase-summary.md) — kiến trúc, module backend/frontend, API, MongoDB, storage
- [docs/project-roadmap.md](./docs/project-roadmap.md) — lộ trình, trạng thái, backlog
- [docs/deploy-web-production.md](./docs/deploy-web-production.md) — build và deploy prod
- [docs/ios-app.md](./docs/ios-app.md) — app iPhone
- [docs/android-app.md](./docs/android-app.md) — app Android
