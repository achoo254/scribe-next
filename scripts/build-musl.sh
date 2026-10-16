#!/usr/bin/env bash
# Build scribe-be → static musl binary qua Docker (image rust-musl-cross đã cấu hình sẵn
# musl cross-toolchain cho ring/aws-lc → né đau build rustls trên musl bằng rustup thuần).
#
# Cần docker. Cấu hình truyền qua ENV — KHÔNG hardcode/commit:
#   set -a; . ./.env.production; set +a; ./scripts/build-musl.sh [SRC_DIR]
set -euo pipefail

SRC="${1:-$PWD}"
IMAGE="ghcr.io/rust-cross/rust-musl-cross:x86_64-musl"
TARGET="x86_64-unknown-linux-musl"

: "${MONGO_URI:?cần MONGO_URI để bake compile-time (build.rs → cargo:rustc-env)}"
# Thiếu các biến này thì binary dừng ngay khi khởi động — chặn từ lúc build cho sớm.
for v in DB_NAME FIREBASE_PROJECT_ID SESSION_KEY_PATH DATA_DIR; do
  [ -n "${!v:-}" ] || { echo "thiếu $v (nạp .env.production trước)" >&2; exit 1; }
done

docker run --rm \
  -v "$SRC":/home/rust/src \
  -e MONGO_URI \
  -e PORT \
  -e ALLOWED_DOMAINS \
  -e ALLOWED_EMAILS \
  -e TRANSCRIBE_EMAILS \
  -e SESSION_KEY_PATH \
  -e DATA_DIR \
  -e DB_NAME \
  -e FIREBASE_PROJECT_ID \
  "$IMAGE" \
  cargo build --release --target "$TARGET" -p scribe-be

BIN="$SRC/target/$TARGET/release/scribe-be"
echo "=== Binary: $BIN ==="
ls -la "$BIN"
file "$BIN" 2>/dev/null || true
