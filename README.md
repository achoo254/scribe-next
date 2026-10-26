# Scribe

Ghi âm, cắt file audio/video và chép lời tiếng Việt, chạy tại <https://scribe.hoandat.com>. Một app trong hệ sinh thái hoandat (cùng Chat, Localman…), đăng nhập qua IdP `auth-next` (`auth.hoandat.com`).

Các ứng dụng dùng chung một backend:

| App | Vị trí | Ghi chú |
|-----|--------|---------|
| Web | `web/` | React + Vite, cài được lên màn hình chính như app |
| iPhone | `web/ios/` | Capacitor bọc bản build của `web/` + plugin Swift để ghi khi khoá màn hình — [docs/ios-app.md](./docs/ios-app.md) |
| Android | `web/android/` | Capacitor, cùng bản build `web/` + plugin Kotlin giữ đúng contract JS của iOS; trạng thái phát hành ở [docs/android-app.md](./docs/android-app.md) |
| Backend | `server/scribe-be/` + `crates/core-split/` | Rust/axum, một binary tĩnh, MongoDB; workspace ở [`Cargo.toml`](./Cargo.toml) |

## Quyết định và ràng buộc

- **Đơn tenant.** Scribe chỉ phục vụ `hoandat.com`. Đa tenant theo domain đã chạy rồi bị bỏ ([ADR 0001](./docs/decisions/0001-tenants-by-domain.md), superseded); dữ liệu vẫn mang `tenantId: "primary"` và giữ shape `GET /api/idp` vì app di động đã phát hành đọc nó.
- **Chép lời bằng key server có hạn mức, hoặc key riêng của người dùng.** Nhà cung cấp và giá mỗi phút nằm ở `server/scribe-be/src/config.rs`; việc chạy bằng key riêng không tính vào hạn mức của Scribe (lý do và quy tắc ở đầu `server/scribe-be/src/user_keys/mod.rs`).
- **Backend đọc cấu hình lúc build** (`option_env!`), không phải lúc chạy: đổi biến là phải build lại. Chi tiết ở mục "Ràng buộc then chốt" của [docs/deploy-web-production.md](./docs/deploy-web-production.md).
- **Hai môi trường: dev local và prod.** Không có staging, không dựng môi trường xem trước; deploy prod làm tay.
- **Env nằm ngoài repo.** [`scripts/env-dir.sh`](./scripts/env-dir.sh) in ra thư mục chứa env (dùng chung giữa các máy của chủ dự án); mẫu biến web ở [`web/.env.example`](./web/.env.example).
- **Brand ở một file.** Tên, màu, icon, bundle id, URL scheme và mẫu host chỉ khai trong [`web/brand.config.json`](./web/brand.config.json), kiểm theo schema của gói dùng chung `@shared-next/brand` (repo `shared-next`; mã thương hiệu chỉ sửa ở đó rồi nâng tag). Asset sinh bằng `pnpm run brand:gen` trong `web/`; `pnpm run lint` chặn tên sản phẩm viết cứng và mã màu ngoài `web/src/styles/tokens.css`. Icon app iPhone và tên trong `Info.plist` giữ như bản đã phát hành, không sinh.

## Chạy và kiểm tra

- Dev local (nạp env, chạy backend rồi web): mục "Chạy dev local" của [docs/deploy-web-production.md](./docs/deploy-web-production.md).
- Lệnh web: `scripts` trong [`web/package.json`](./web/package.json). Lệnh backend: `cargo clippy` và `cargo test` trên workspace.
- CI ([`.github/workflows/ci.yml`](./.github/workflows/ci.yml)) chạy bộ kiểm của web, Rust (gồm test trên Mongo thật, `--skip live_` bỏ các test gọi API trả tiền) và Android. CI bỏ qua so ảnh chụp e2e, nên trước khi deploy web chạy `pnpm run test:e2e` và `pnpm run build` trong `web/`; `build` kết thúc bằng `check:dist` để chặn stub của e2e lọt vào `dist/`.
- AAB Android cho Play do [`.github/workflows/android-release.yml`](./.github/workflows/android-release.yml) ký.

## Tài liệu

- [docs/codebase-summary.md](./docs/codebase-summary.md) — kiến trúc, module backend/frontend, API, MongoDB, storage
- [docs/deploy-web-production.md](./docs/deploy-web-production.md) — chạy dev, build và deploy prod
- [docs/ios-app.md](./docs/ios-app.md) — app iPhone, cài lên máy và phát hành
- [docs/android-app.md](./docs/android-app.md) — app Android
- [docs/app-store/](./docs/app-store/) — metadata và ảnh cho store
- [docs/project-roadmap.md](./docs/project-roadmap.md) — lộ trình và backlog (bản ghi theo thời gian)
- [docs/decisions/](./docs/decisions/) — quyết định kiến trúc
