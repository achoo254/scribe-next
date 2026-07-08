# -*- mode: python ; coding: utf-8 -*-
import platform
import sys

from PyInstaller.utils.hooks import collect_all

datas = []
binaries = []
hiddenimports = []
for _pkg in ('faster_whisper', 'ctranslate2', 'av', 'huggingface_hub', 'tokenizers'):
    tmp_ret = collect_all(_pkg)
    datas += tmp_ret[0]; binaries += tmp_ret[1]; hiddenimports += tmp_ret[2]

# MLX (Metal GPU) chỉ trên macOS Apple Silicon → gói mlx/mlx_whisper/numba khi build ở đó.
# Windows/Intel: mlx không cài (marker requirements) → bỏ qua, sidecar giữ nguyên như cũ.
# torch là dep của mlx-whisper nhưng KHÔNG cần lúc transcribe (đã verify) → loại khỏi bundle
# (excludes bên dưới) để tránh phình ~2GB.
if sys.platform == 'darwin' and platform.machine() == 'arm64':
    for _pkg in ('mlx', 'mlx_whisper', 'numba'):
        tmp_ret = collect_all(_pkg)
        datas += tmp_ret[0]; binaries += tmp_ret[1]; hiddenimports += tmp_ret[2]


a = Analysis(
    ['main.py'],
    pathex=[],
    binaries=binaries,
    datas=datas,
    hiddenimports=hiddenimports,
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=['torch', 'torchaudio', 'torchvision'],
    noarchive=False,
    optimize=0,
)
pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name='transcriber',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=True,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)
