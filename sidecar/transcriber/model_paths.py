"""Ánh xạ size model → (repo HF, subfolder) + resolve đường dẫn model thật + backend.

1 nguồn cho main.py khi thêm/đổi model, tránh lệch nhau.

- small/medium/large: PhoWhisper CT2 (faster-whisper, CPU int8). small/medium từ quocphu (mỗi size
  1 subfolder CT2); large dùng bản kiendt (không subfolder).
- large-v3-mlx: Whisper large-v3 bản MLX (mlx-community) — chạy Metal GPU trên Apple Silicon,
  nhanh hơn CT2 CPU nhiều lần. Chỉ khả dụng trên macOS arm64 (xem lib.rs::mlx_supported + UI gating).
"""
import os

MODELS = {
    "small": ("quocphu/PhoWhisper-ct2-FasterWhisper", "PhoWhisper-small-ct2-fasterWhisper"),
    "medium": ("quocphu/PhoWhisper-ct2-FasterWhisper", "PhoWhisper-medium-ct2-fasterWhisper"),
    "large": ("kiendt/PhoWhisper-large-ct2", ""),
    "large-v3-mlx": ("mlx-community/whisper-large-v3-mlx", ""),
}

# Size chạy backend MLX (mlx-whisper). Còn lại = "ct2" (faster-whisper). ensure_model + snapshot
# tải dùng chung; chỉ đường transcribe rẽ nhánh theo backend (main.py).
MLX_SIZES = {"large-v3-mlx"}


def backend_of(size: str) -> str:
    return "mlx" if size in MLX_SIZES else "ct2"


def resolve_model_path(model_dir: str, size: str) -> str:
    # Đường dẫn thật chứa model (CT2: config.json/model.bin trong subfolder; MLX/large: model_dir).
    _, subdir = MODELS[size]
    return os.path.join(model_dir, subdir) if subdir else model_dir
