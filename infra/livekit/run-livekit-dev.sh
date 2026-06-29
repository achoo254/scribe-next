#!/usr/bin/env bash
# Cài (lần đầu) + chạy LiveKit dev NATIVE trên WSL Ubuntu — KHÔNG Docker.
# Dev mode: apiKey=devkey, apiSecret=secret, ws://localhost:7880. App Tauri (Windows) nối qua localhost.
# Chạy từ WSL:  bash infra/livekit/run-livekit-dev.sh
set -euo pipefail

if ! command -v livekit-server >/dev/null 2>&1; then
  echo "[livekit] chưa có livekit-server — cài qua get.livekit.io ..."
  curl -sSL https://get.livekit.io | bash
fi

echo "[livekit] dev mode: ws://localhost:7880 (devkey/secret). Ctrl+C để dừng."
exec livekit-server --dev --bind 0.0.0.0
