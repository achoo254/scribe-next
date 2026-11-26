# AGENTS.md

Scribe (`scribe-next`): ghi âm, cắt audio/video và chép lời tiếng Việt tại `scribe.hoandat.com`; backend Rust (`server`, `crates`) và web (`web`). Đọc `README.md`, `docs/codebase-summary.md` và `docs/deploy-web-production.md` trước khi làm.

## Gate trước khi báo xong

- Rust: `cargo clippy --workspace --all-targets -- -D warnings` rồi `cargo test --workspace`.
- Web (trong `web/`): `pnpm run lint` → `pnpm run typecheck` → `pnpm run check:tokens` → `pnpm test`. Chỉ dùng `pnpm`, không dùng npm.

Máy dev dùng chung: ưu tiên chạy test của đúng crate hoặc file liên quan; bộ đầy đủ chạy trên CI (push `main` rồi `gh run watch`).

## Deploy prod

Không có môi trường dev hay staging: xong việc, push hết lên `main`, kiểm `ci`, rồi `bash ../tools/deploy-prod.sh scribe-next` (script của workspace `hoandat-app`). Không cần hỏi trước; bước đổi bản chờ phiên ghi, upload và chép lời đang chạy xong (tối đa 10 phút). Vẫn hỏi trước khi ghi dữ liệu tay, restore, sửa bí mật/OAuth hay chạy script setup. Chuẩn chung: `../docs/deploy-standard.md`; đường dự phòng khi CI hỏng: `docs/deploy-web-production.md`; vận hành prod: skill `scribe-ops-prod`.

## Repo công khai

Không commit IP, cổng SSH, user máy chủ, secret hay file env. Env production nằm ngoài repo ở iCloud `ENV production/<tên dự án>/`.
