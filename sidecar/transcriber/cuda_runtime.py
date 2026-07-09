"""Tải CUDA runtime (cuBLAS + cuDNN 9) on-demand từ PyPI cho faster-whisper GPU.

Vì sao cần: CTranslate2 build Windows cần cublas64_12/cublasLt64_12 + cudnn_*64_9 DLL để
chạy trên GPU; sidecar PyInstaller CPU-only KHÔNG gói sẵn (đội ~1.3GB installer). Thay vào đó
chỉ máy có GPU NVIDIA mới tải wheel PyPI về app-data 1 lần, giải nén lấy DLL.

Wheel `.whl` = file zip chuẩn → mở bằng zipfile, chỉ trích `.dll` (KHÔNG chạy code Python trong
wheel, KHÔNG pip install). Toàn bộ dùng stdlib (urllib/zipfile/hashlib/json) → PyInstaller tự
bundle, không thêm dependency.

Tiến độ tải: Rust poll dung lượng cuda_dir (file `.part` ghi ngay trong đó) → không cần cơ chế
progress riêng. Xong cả 2 package → ghi marker rỗng; lần sau thấy marker thì bỏ qua (idempotent).
"""
import hashlib
import json
import os
import sys
import urllib.request
import zipfile
from pathlib import Path

# Verified máy thật (RTX 5070 Ti, ct2 4.8) qua EnumProcessModules: CTranslate2 Whisper inference GPU
# CHỈ nạp cublas64_12 + cublasLt64_12 — KHÔNG nạp cuDNN nào dù chạy encoder(conv) + decoder(attn) +
# word_timestamps + beam search. Nên bỏ nvidia-cudnn-cu12 (~737MB tải / ~1.1GB đĩa) → chỉ cublas
# (~553MB tải / ~771MB đĩa, phần lớn là cublasLt 668MB — dep bắt buộc của cublas). cudart64_12
# static-link sẵn nên không cần nvidia-cuda-runtime-cu12.
# LƯU Ý: nếu nâng CTranslate2 lên bản mới cần cuDNN → thêm lại "nvidia-cudnn-cu12" vào list + sửa
# CUDA_TOTAL_BYTES trong src-tauri/src/cuda.rs cho khớp thanh tiến độ.
CUDA_PACKAGES = ["nvidia-cublas-cu12"]
MARKER_NAME = "_cuda_ready.marker"
_PYPI_JSON = "https://pypi.org/pypi/{pkg}/json"
_CHUNK = 1 << 20  # 1MB


def _pypi_win_wheel(pkg: str) -> tuple[str, str]:
    """(url, sha256) của wheel win_amd64 mới nhất cho pkg. Raise nếu không có."""
    with urllib.request.urlopen(_PYPI_JSON.format(pkg=pkg), timeout=30) as r:
        data = json.loads(r.read().decode("utf-8"))
    version = data["info"]["version"]
    for f in data["releases"].get(version, []):
        if "win_amd64" in f["filename"] and f["filename"].endswith(".whl"):
            return f["url"], f["digests"]["sha256"]
    raise RuntimeError(f"Không tìm thấy wheel win_amd64 cho {pkg} (version {version})")


def _download(url: str, dest_part: Path, sha256_expect: str, on_progress=None) -> None:
    """Tải url → dest_part (chunked), verify sha256. Lệch/lỗi → xoá file dở + raise."""
    h = hashlib.sha256()
    got = 0
    try:
        with urllib.request.urlopen(url, timeout=60) as r:
            total = int(r.headers.get("Content-Length", 0))
            with open(dest_part, "wb") as out:
                while True:
                    chunk = r.read(_CHUNK)
                    if not chunk:
                        break
                    out.write(chunk)
                    h.update(chunk)
                    got += len(chunk)
                    if on_progress:
                        on_progress(got, total)
    except Exception as e:
        # Bắt rộng (gồm http.client.IncompleteRead — KHÔNG phải OSError) để luôn dọn .part dở dang;
        # lần sau ghi đè sạch (không resume). Disk đầy / mạng rớt / server đóng giữa chừng đều rơi vào đây.
        dest_part.unlink(missing_ok=True)
        raise RuntimeError(f"Tải/ghi thất bại ({url}): {e}") from e
    actual = h.hexdigest()
    if actual != sha256_expect:
        dest_part.unlink(missing_ok=True)
        raise RuntimeError(f"sha256 lệch: {actual} != {sha256_expect} ({url})")


def _extract_dlls(whl_path: Path, dest_dir: Path) -> int:
    """Trích mọi `.dll` trong wheel (flat, theo basename) vào dest_dir. Trả số DLL đã ghi.

    Không hardcode tên DLL — cuDNN 9 chia nhiều file con (cudnn_ops64_9/cudnn_cnn64_9/
    cudnn_engines_precompiled64_9/...) tên đổi theo version; glob theo đuôi .dll bền nhất.
    """
    count = 0
    with zipfile.ZipFile(whl_path) as z:
        for name in z.namelist():
            if name.lower().endswith(".dll"):
                (dest_dir / os.path.basename(name)).write_bytes(z.read(name))
                count += 1
    return count


def ensure_cuda_runtime(cuda_dir: str, on_progress=None) -> None:
    """Đảm bảo DLL CUDA runtime có mặt trong cuda_dir. Idempotent (marker → return ngay)."""
    root = Path(cuda_dir)
    root.mkdir(parents=True, exist_ok=True)
    marker = root / MARKER_NAME
    if marker.is_file():
        return
    for pkg in CUDA_PACKAGES:
        url, sha = _pypi_win_wheel(pkg)
        part = root / f"{pkg}.whl.part"
        whl = root / f"{pkg}.whl"
        _download(url, part, sha, on_progress=on_progress)
        part.replace(whl)
        _extract_dlls(whl, root)
        whl.unlink(missing_ok=True)  # dọn ngay, đỡ giữ ~1.3GB disk dư
    marker.touch()


if __name__ == "__main__":
    # Dev test tay không cần build lại sidecar: python cuda_runtime.py <cuda_dir>
    if len(sys.argv) < 2:
        print("usage: python cuda_runtime.py <cuda_dir>", file=sys.stderr)
        sys.exit(2)

    def _stderr_progress(got: int, total: int) -> None:
        pct = f"{got / total * 100:5.1f}%" if total else f"{got} bytes"
        print(f"\r  {pct} ({got / 1e6:.0f}MB)", end="", file=sys.stderr, flush=True)

    target = sys.argv[1]
    print(f"Tải CUDA runtime → {target}", file=sys.stderr)
    ensure_cuda_runtime(target, on_progress=_stderr_progress)
    print("\nXong.", file=sys.stderr)
