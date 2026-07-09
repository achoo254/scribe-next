"""Chọn device nạp WhisperModel: GPU (CUDA float16) nếu sẵn sàng, else CPU int8.

Tách khỏi main.py (đã dài) để cô lập logic device + DLL search path.

BẮT BUỘC float16 trên GPU: RTX 50-series (Blackwell sm_120) crash CUBLAS_STATUS_NOT_SUPPORTED
với int8/int8_float16 — không expose compute_type ra UI, hardcode float16 khi device=cuda.

add_dll_directory(cuda_dir) phải chạy TRƯỚC lần import faster_whisper/ctranslate2 đầu tiên của
process → chèn ngay trước import ở đây; guard chỉ add 1 lần (Python cache module, add lại vô ích).
GPU init lỗi (driver/DLL) → fallback CPU int8 ngay lần gọi đó, KHÔNG cache trạng thái hỏng (lỗi có
thể tạm thời, cho phép thử lại lần sau).
"""
import os

from cuda_runtime import MARKER_NAME  # DRY: dùng chung tên marker với downloader

_dll_dir_added = False


def _cuda_ready(cuda_dir) -> bool:
    return bool(cuda_dir) and os.path.isfile(os.path.join(cuda_dir, MARKER_NAME))


def _prepare_dll_dir(cuda_dir: str) -> None:
    # add_dll_directory chỉ có trên Windows Python ≥3.8 → guard cho macOS/Linux.
    global _dll_dir_added
    if not _dll_dir_added and hasattr(os, "add_dll_directory"):
        os.add_dll_directory(cuda_dir)
        # QUAN TRỌNG: CTranslate2 nạp cublas64_12/cudnn*64_9 qua LoadLibrary THEO TÊN — KHÔNG tôn trọng
        # add_dll_directory (chỉ tra PATH + thư mục chuẩn). Verified trên máy thật: có add_dll_directory
        # vẫn "cublas64_12.dll cannot be loaded" khi chạy encode. Prepend cuda_dir vào PATH mới đủ để
        # loader tìm thấy DLL lúc ct2 nạp lazy ở lần compute GPU đầu tiên.
        os.environ["PATH"] = cuda_dir + os.pathsep + os.environ.get("PATH", "")
        _dll_dir_added = True


def load_whisper_model(model_path, use_gpu, cuda_dir, cpu_threads, send_info):
    """Trả (WhisperModel, device_str, compute_str). send_info(msg) = callback log (tránh import main)."""
    if use_gpu and _cuda_ready(cuda_dir):
        _prepare_dll_dir(cuda_dir)
        try:
            from faster_whisper import WhisperModel
            model = WhisperModel(model_path, device="cuda", compute_type="float16")
            return model, "cuda", "float16"
        except Exception as e:  # driver/DLL lỗi runtime → không hỏng cả file, tụt xuống CPU
            send_info(f"GPU lỗi ({e}) — chuyển sang CPU")
    elif use_gpu:
        send_info("GPU chưa sẵn sàng (chưa tải CUDA runtime) — dùng CPU")

    from faster_whisper import WhisperModel
    model = WhisperModel(model_path, device="cpu", compute_type="int8", cpu_threads=cpu_threads)
    return model, "cpu", "int8"
