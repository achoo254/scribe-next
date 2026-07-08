#!/usr/bin/env bash
# Build transcriber sidecar (macOS/Linux) → src-tauri/binaries/transcriber-<target> (one-file).
# Đối ứng của build_pyinstaller.ps1 cho Mac/Linux. Dev có thể chạy thẳng python main.py (xem transcribe.rs).
# Yêu cầu: Python (đã có .venv hoặc tạo mới). Đặt PYTHON=python3.12 để ép version cụ thể.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$HERE"

target="$(rustc -Vv | awk -F': ' '/^host:/{print $2}')"
pybin="${PYTHON:-python3}"

venv="$HERE/.venv"
if [ ! -x "$venv/bin/python" ]; then
    "$pybin" -m venv "$venv"
fi
py="$venv/bin/python"

"$py" -m pip install --upgrade pip
"$py" -m pip install -r requirements.txt pyinstaller

# one-file qua transcriber.spec (nguồn duy nhất: collect ct2/av/…; macOS arm64 thêm mlx + loại torch).
"$py" -m PyInstaller --noconfirm transcriber.spec

bin_dir="$HERE/../../src-tauri/binaries"
mkdir -p "$bin_dir"
cp -f "dist/transcriber" "$bin_dir/transcriber-$target"
chmod +x "$bin_dir/transcriber-$target"
echo "Done -> src-tauri/binaries/transcriber-$target"
