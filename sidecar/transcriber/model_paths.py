"""Ánh xạ size PhoWhisper → (repo HF, subfolder CT2) + resolve đường dẫn model thật.

1 nguồn cho main.py khi thêm/đổi model, tránh lệch nhau.

small/medium từ quocphu (mỗi size 1 subfolder CT2). large dùng bản kiendt (không subfolder).
"""
import os

MODELS = {
    "small": ("quocphu/PhoWhisper-ct2-FasterWhisper", "PhoWhisper-small-ct2-fasterWhisper"),
    "medium": ("quocphu/PhoWhisper-ct2-FasterWhisper", "PhoWhisper-medium-ct2-fasterWhisper"),
    "large": ("kiendt/PhoWhisper-large-ct2", ""),
}


def resolve_model_path(model_dir: str, size: str) -> str:
    # Đường dẫn thật chứa config.json/model.bin (small/medium nằm trong subfolder của repo).
    _, subdir = MODELS[size]
    return os.path.join(model_dir, subdir) if subdir else model_dir
