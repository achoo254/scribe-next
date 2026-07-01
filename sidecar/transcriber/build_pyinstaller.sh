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

# one-file: gom ctranslate2/av native libs. --collect-all để chắc chắn không thiếu.
"$py" -m PyInstaller --onefile --name transcriber \
    --collect-all faster_whisper --collect-all ctranslate2 --collect-all av \
    --collect-all huggingface_hub --collect-all tokenizers \
    main.py

bin_dir="$HERE/../../src-tauri/binaries"
mkdir -p "$bin_dir"
cp -f "dist/transcriber" "$bin_dir/transcriber-$target"
chmod +x "$bin_dir/transcriber-$target"
echo "Done -> src-tauri/binaries/transcriber-$target"
