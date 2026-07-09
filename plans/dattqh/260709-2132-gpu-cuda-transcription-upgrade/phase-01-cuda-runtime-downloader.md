# Phase 01 — CUDA Runtime Downloader (sidecar Python)

## Context Links
- Overview: [plan.md](plan.md)
- Research: `plans/dattqh/reports/gpu-transcription-model-upgrade-260709-2124-rtx5070ti-report.md` (mục 3)
- Verify PyPI (thực hiện lúc lập plan, KHÔNG phải research report):
  - `nvidia-cublas-cu12` 12.9.2.10, wheel `nvidia_cublas_cu12-12.9.2.10-py3-none-win_amd64.whl`, 553.2MB, sha256 `623f43027d40d44ceadf0043f002bd25cf353e8f13ce90b9a87057019f560661`
  - `nvidia-cudnn-cu12` 9.24.0.43, wheel `nvidia_cudnn_cu12-9.24.0.43-py3-none-win_amd64.whl`, 737.1MB, sha256 `cbd41a0ab084422c936dc9fb2fc89be5ea9a85bc421c6f23d0243bdfc945fbef`
  - Cả 2 wheel là `py3-none` (KHÔNG gắn cpython tag) → không phụ thuộc Python 3.14 của sidecar, an toàn.
  - Fallback dự phòng (chỉ dùng nếu Phase 07 phát hiện thiếu `cudart64_12.dll`): `nvidia-cuda-runtime-cu12` 12.9.79, wheel `nvidia_cuda_runtime_cu12-12.9.79-py3-none-win_amd64.whl`, 3.59MB — đã verify tồn tại, CHƯA cần dùng (5 GitHub issue Windows khảo sát không ai báo thiếu cudart).

## Overview
- Priority: P1 (chặn Phase 02, 04)
- Status: pending
- Sidecar tải 2 wheel PyPI (cuBLAS + cuDNN 9, tổng ~1.29GB) về `<app_data>/cuda/`, verify sha256, giải nén DLL (bỏ toàn bộ phần Python/metadata của wheel), ghi marker khi xong. KHÔNG dùng pip install — chỉ coi wheel như file zip chứa DLL.

## Key Insights
- Wheel `.whl` = file zip chuẩn — mở bằng `zipfile` stdlib, filter member theo đuôi `.dll`, extract flat vào `cuda_dir` (KHÔNG hardcode tên file — cuDNN 9 chia nhiều DLL con `cudnn_ops64_9.dll`/`cudnn_cnn64_9.dll`/`cudnn_engines_precompiled64_9.dll`/... tên có thể đổi theo version; glob theo đuôi `.dll` là cách bền nhất).
- KHÔNG cần cài `nvidia-cublas-cu12`/`nvidia-cudnn-cu12` qua pip — chỉ tải file .whl thô qua HTTP rồi tự giải nén. Không thêm dependency vào `requirements.txt`.
- Lấy URL + sha256 qua PyPI JSON API (`https://pypi.org/pypi/<pkg>/json`) — tránh hardcode version, tự bắt version mới nhất mỗi lần build lại sidecar (nhưng vẫn cố định version tại THỜI ĐIỂM implement để tránh bất ngờ — xem Implementation Steps).
- Poll tiến độ: Rust đã có sẵn `dir_size()` (transcribe.rs) đệ quy cộng dung lượng mọi file dưới 1 thư mục — nếu Python ghi file tạm `.part` NGAY TRONG `cuda_dir` (không dùng thư mục ngoài `%TEMP%`), Rust poll `cuda_dir` là tự động thấy tiến độ tải, KHÔNG cần thêm cơ chế progress riêng. Xoá `.part` sau khi xong.
- Đánh dấu "đã tải xong + verify + giải nén xong" bằng 1 marker file rỗng `_cuda_ready.marker` trong `cuda_dir` — Rust/Python đều check tồn tại file này, không đoán qua số lượng DLL (tên DLL không ổn định giữa version cuDNN).
- Idempotent: nếu marker đã tồn tại → return ngay, không tải lại.

## Requirements
### Functional
- Hàm `ensure_cuda_runtime(cuda_dir: str) -> None` (raise Exception nếu lỗi — main.py bắt và gửi `{"type":"error"}` theo pattern try/except đã có ở `main()`).
- Tải tuần tự `nvidia-cublas-cu12` rồi `nvidia-cudnn-cu12` (constant `CUDA_PACKAGES = ["nvidia-cublas-cu12", "nvidia-cudnn-cu12"]` — thêm `nvidia-cuda-runtime-cu12` vào list này là đủ nếu Phase 07 cần).
- Verify sha256 trước khi giải nén — mismatch → raise, xoá file tạm, KHÔNG để lại state hỏng dở.
- Giải nén xong 1 wheel → xoá file `.whl` tạm ngay (không giữ lại, đỡ tốn ~1.3GB disk dư).
- Ghi marker `_cuda_ready.marker` (empty) sau khi CẢ 2 package xong.
- main.py: thêm nhánh `cmd == "ensure_cuda"` trong dispatch (dòng ~303-311) → gọi `cuda_runtime.ensure_cuda_runtime(req["cuda_dir"])` → `send({"type": "cuda_ready"})`.

### Non-functional
- KHÔNG thêm dependency mới vào `requirements.txt` / `transcriber.spec` (chỉ dùng `urllib.request`, `zipfile`, `hashlib`, `json` — toàn bộ stdlib, PyInstaller tự bundle không cần hook riêng).
- File `cuda_runtime.py` < 200 dòng (theo quy tắc modularize).
- Chạy được độc lập qua `python cuda_runtime.py <cuda_dir>` để dev tự test không cần build lại sidecar.

## Architecture
```
main.py (cmd="ensure_cuda", cuda_dir)
   └─► cuda_runtime.ensure_cuda_runtime(cuda_dir)
          ├─ đã có _cuda_ready.marker? → return (idempotent)
          ├─ for pkg in CUDA_PACKAGES:
          │     ├─ GET https://pypi.org/pypi/{pkg}/json → chọn release['<version>'] có filename chứa "win_amd64"
          │     ├─ download url → {cuda_dir}/{pkg}.whl.part (chunked, urllib.request)
          │     ├─ sha256({pkg}.whl.part) == digests.sha256 ? else raise
          │     ├─ rename .part → .whl; zipfile mở, extract member *.dll → cuda_dir (flat, os.path.basename)
          │     └─ xoá {pkg}.whl
          └─ write cuda_dir/_cuda_ready.marker (empty)
```
Rust (Phase 04) poll `dir_size(cuda_dir)` / hằng số tổng ~1.29GB → emit `cuda_download` pct; khi thấy marker → dừng poll, coi như `cuda_ready`.

## Related Code Files
- **Create:** `sidecar/transcriber/cuda_runtime.py`
- **Modify:** `sidecar/transcriber/main.py` (dispatch `cmd == "ensure_cuda"` trong `main()`, khoảng dòng 301-314; import `cuda_runtime` đầu file cạnh `from model_paths import ...` dòng 25)

## Implementation Steps
1. Tạo `cuda_runtime.py`: hằng `CUDA_PACKAGES = ["nvidia-cublas-cu12", "nvidia-cudnn-cu12"]`, `MARKER_NAME = "_cuda_ready.marker"`.
2. Hàm `_pypi_win_wheel(pkg: str) -> tuple[str, str]` (url, sha256): GET JSON `https://pypi.org/pypi/{pkg}/json`, lấy `info.version`, lọc `releases[version]` theo `"win_amd64" in f["filename"]`, trả `(f["url"], f["digests"]["sha256"])`. Không tìm thấy → raise `RuntimeError(f"Không tìm thấy wheel win_amd64 cho {pkg}")`.
3. Hàm `_download(url, dest_part, sha256_expect)`: `urllib.request.urlopen(url)` đọc chunk 1MB ghi `dest_part`; xong so `hashlib.sha256(...).hexdigest()` với `sha256_expect` — lệch → xoá file + raise.
4. Hàm `_extract_dlls(whl_path, dest_dir)`: `zipfile.ZipFile(whl_path)`, với mỗi `name` trong `namelist()` nếu `name.lower().endswith(".dll")` → đọc bytes, viết `dest_dir/os.path.basename(name)`.
5. Hàm chính `ensure_cuda_runtime(cuda_dir: str)`: mkdir cuda_dir; marker tồn tại → return; for pkg in CUDA_PACKAGES: gọi 3 hàm trên tuần tự + dọn `.whl`/`.part`; cuối cùng `Path(cuda_dir, MARKER_NAME).touch()`.
6. Thêm `import cuda_runtime` vào `main.py` (cạnh dòng 25).
7. Thêm nhánh dispatch trong `main()`:
   ```python
   elif cmd == "ensure_cuda":
       cuda_runtime.ensure_cuda_runtime(req["cuda_dir"])
       send({"type": "cuda_ready"})
   ```
   (chèn giữa nhánh `ensure_model` và `transcribe`, main.py dòng ~303-309 — lỗi tự rơi vào except chung dòng 312-314, nhất quán pattern có sẵn).
8. `if __name__ == "__main__":` cuối `cuda_runtime.py` thêm CLI nhỏ: `sys.argv[1]` = cuda_dir, gọi `ensure_cuda_runtime`, in tiến độ ra stderr để dev test tay không cần Rust.

## Todo List
- [ ] Tạo `cuda_runtime.py` với 5 hàm trên
- [ ] Test độc lập: `python cuda_runtime.py <tmp_dir>` trên máy dev → confirm tải xong, có DLL + marker
- [ ] Wire dispatch `cmd == "ensure_cuda"` vào `main.py`
- [ ] Test qua sidecar thật: gửi `{"cmd":"ensure_cuda","cuda_dir":"..."}` qua stdin thủ công → nhận `cuda_ready`
- [ ] Verify sha256 mismatch case (sửa 1 byte sha256 giả để test raise) — dọn file tạm đúng

## Success Criteria
- Chạy `python cuda_runtime.py <dir>` xong → `<dir>` có ≥5 file `.dll` (cublas + cublasLt + cudnn family) + `_cuda_ready.marker`, không còn `.whl`/`.part` nào sót.
- Gọi lại lần 2 (đã có marker) → return ngay, không tải lại (kiểm tra qua log/thời gian chạy < 1s).
- sha256 sai → raise rõ ràng, không để file `.whl` hỏng nằm lại.

## Risk Assessment
| Risk | Likelihood × Impact | Mitigation |
|---|---|---|
| PyPI đổi version mới, wheel filename pattern khác | Thấp × TB | filter theo `"win_amd64" in filename` (không hardcode version), raise rõ nếu không tìm thấy |
| Tải giữa chừng mất mạng → file `.part` dở dang | TB × Thấp | luôn tải lại từ đầu (không resume) — đơn giản, wheel không đổi giữa các lần chạy nên an toàn ghi đè |
| Ổ đĩa không đủ ~1.3GB tạm | Thấp × TB | để implement bắt `OSError` khi write, raise message rõ ("hết dung lượng đĩa") |

## Security Considerations
- Tải qua HTTPS (PyPI/files.pythonhosted.org) — TLS chống MITM; sha256 check chống lỗi truyền tải, KHÔNG chống PyPI bị compromise (đánh đổi chấp nhận được, cùng mức tin cậy như `huggingface_hub` đang dùng cho model).
- Không chạy code trong wheel — chỉ đọc bytes DLL, không import/exec bất kỳ file `.py` nào trong wheel.

## Next Steps
- Phase 02 dùng `cuda_dir` này để `os.add_dll_directory()` trước khi import `ctranslate2`.
- Phase 04 (Rust) implement poller theo `dir_size(cuda_dir)` + tổng ~1.29GB (553.2+737.1 MB) và check marker.
