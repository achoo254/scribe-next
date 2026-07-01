#!/usr/bin/env bash
# Dev local-first NATIVE (KHÔNG Docker) trên WSL Ubuntu: chạy LiveKit dev + meeting-server cùng lúc.
# Dùng: bash infra/run-local.sh   (Ctrl-C để dừng cả hai)
#
# LiveKit dev key/secret mặc định = devkey/secret (livekit-server --dev). meeting-server phải dùng
# ĐÚNG cặp này để token verify được.
set -euo pipefail
cd "$(dirname "$0")/.."

LK_KEY="${LK_API_KEY:-devkey}"
LK_SECRET="${LK_API_SECRET:-secret}"

echo "[run-local] LiveKit dev (ws://localhost:7880) …"
if ! command -v livekit-server >/dev/null 2>&1; then
  echo "  ⚠ chưa cài livekit-server → curl -sSL https://get.livekit.io | bash" >&2
  exit 1
fi
livekit-server --dev &
LK_PID=$!

echo "[run-local] meeting-server (http://localhost:8787) …"
LK_API_KEY="$LK_KEY" \
LK_API_SECRET="$LK_SECRET" \
LIVEKIT_URL="ws://localhost:7880" \
HOST_SECRET="${HOST_SECRET:-dev-host-secret}" \
ALLOW_ORIGINS="${ALLOW_ORIGINS:-*}" \
  node server/meeting-server/index.mjs &
MS_PID=$!

trap 'echo "[run-local] dừng…"; kill $LK_PID $MS_PID 2>/dev/null || true' INT TERM EXIT
echo "[run-local] Đang chạy. Host token:  curl -s localhost:8787/token/host -d '{\"room\":\"demo\",\"secret\":\"dev-host-secret\"}'"
wait
