# Build transcriber sidecar (Windows) → src-tauri/binaries/transcriber-<target>.exe (one-file).
# Dùng cho đóng gói release (P5). Dev chạy thẳng .venv\Scripts\python.exe main.py (xem transcribe.rs).
# Yêu cầu: Python (đã có .venv hoặc tạo mới).
$ErrorActionPreference = "Stop"
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $here

$target = (rustc -Vv | Select-String "host:").ToString().Split(":")[1].Trim()
$pyexe = Join-Path $here ".venv\Scripts\python.exe"
if (-not (Test-Path $pyexe)) {
    python -m venv .venv
    $pyexe = Join-Path $here ".venv\Scripts\python.exe"
}

& $pyexe -m pip install --upgrade pip
& $pyexe -m pip install -r requirements.txt pyinstaller

# one-file: gom ctranslate2/av native libs. --collect-all để chắc chắn không thiếu.
& $pyexe -m PyInstaller --onefile --name transcriber `
    --collect-all faster_whisper --collect-all ctranslate2 --collect-all av `
    --collect-all huggingface_hub --collect-all tokenizers `
    main.py

$binDir = Join-Path $here "..\..\src-tauri\binaries"
New-Item -ItemType Directory -Force -Path $binDir | Out-Null
Copy-Item "dist\transcriber.exe" (Join-Path $binDir "transcriber-$target.exe") -Force
Write-Output "Done -> src-tauri/binaries/transcriber-$target.exe"
