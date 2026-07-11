#!/usr/bin/env bash
# Build onescribe-be → static musl binary qua Docker (image rust-musl-cross đã cấu hình sẵn
# musl cross-toolchain cho ring/aws-lc → né đau build rustls trên musl bằng rustup thuần).
#
# Chạy trong WSL (có docker). Secret (MONGO_URI...) truyền qua ENV — KHÔNG hardcode/commit.
# Usage: MONGO_URI="mongodb://..." [ALLOWED_DOMAINS=...] ./scripts/build-musl.sh [SRC_DIR]
set -euo pipefail

SRC="${1:-$PWD}"
IMAGE="ghcr.io/rust-cross/rust-musl-cross:x86_64-musl"
TARGET="x86_64-unknown-linux-musl"

: "${MONGO_URI:?cần MONGO_URI để bake compile-time (build.rs → cargo:rustc-env)}"

docker run --rm \
  -v "$SRC":/home/rust/src \
  -e MONGO_URI \
  -e PORT \
  -e ALLOWED_DOMAINS \
  -e ALLOWED_EMAILS \
  -e SESSION_KEY_PATH \
  "$IMAGE" \
  cargo build --release --target "$TARGET" -p onescribe-be

BIN="$SRC/target/$TARGET/release/onescribe-be"
echo "=== Binary: $BIN ==="
ls -la "$BIN"
file "$BIN" 2>/dev/null || true
