# Phase 07 — Test & Verify trên máy RTX 5070 Ti thật

## Context Links
- Overview: [plan.md](plan.md)
- Depends on: Phase 01-06 (toàn bộ code phải xong)
- Research: mục 2 (Blackwell int8 bug), mục "Unresolved Questions" #3 (root cause 5070 Ti vs 4070 Ti Super chưa chốt)

## Overview
- Priority: P1 (gate cuối trước khi coi feature "done")
- Status: pending
- Repo KHÔNG có test framework nào (đã verify: không `vitest`/`pytest`/`cargo test` cấu hình, `package.json` chỉ có `build`/`dev`/`tauri:*` script) → test matrix của phase này = **compile check tự động** (`cargo check`, `npm run build`) + **verify tay có checklist** trên máy thật (không phải automated test suite mới — KHÔNG tự chế test framework mới cho 1 feature, vi phạm YAGNI so với convention hiện tại của repo).

## Key Insights
- Đây là gate duy nhất xác nhận giả thuyết `compute_type="float16"` KHÔNG crash Blackwell sm_120 trên chính máy user — research report dựa trên báo cáo GitHub (SubtitleEdit #10180) của người khác, CHƯA chạy thực tế trên RTX 5070 Ti của user.
- Benchmark tốc độ GPU vs CPU cùng file → có số liệu thật thay số liệu tham khảo (research report ghi "~4x", "~12x realtime" là số của người khác/GPU khác).
- Phải test cả 2 nhánh fallback: (1) chưa tải CUDA runtime + toggle GPU on → fallback CPU không crash; (2) tải xong + toggle on → chạy GPU thật.

## Requirements
### Functional
- Checklist manual dưới đây PHẢI pass 100% trước khi coi phase/feature "done".
- Nếu phát hiện thiếu `cudart64_12.dll` (xem risk Phase 01) → thêm `nvidia-cuda-runtime-cu12` vào `CUDA_PACKAGES` (`cuda_runtime.py`), build lại, test lại.

### Non-functional
- Không cần viết test code mới (theo Key Insights) — trừ khi trong lúc verify phát sinh bug cần fix code, thì fix trực tiếp tại file gốc của phase liên quan (không tạo file "fix" riêng).

## Architecture
Không áp dụng (phase verify, không có kiến trúc mới).

## Related Code Files
- Không sửa file theo kế hoạch — CHỈ sửa nếu bug phát sinh (ghi rõ file nào bị patch tại mục kết quả test nếu có).

## Implementation Steps (checklist thực thi tuần tự)
1. `cd src-tauri && cargo check` — xanh, không warning mới.
2. `npm run build` (vue-tsc + vite build) — xanh.
3. `npm run tauri dev` — app chạy được, không lỗi console.
4. **Máy KHÔNG có GPU (hoặc giả lập rename `nvidia-smi.exe` tạm)**: mở Mode C → khối GPU KHÔNG hiện → transcribe CPU int8 chạy như cũ (regression check).
5. **Máy CÓ GPU, chưa tải CUDA runtime**: khối GPU hiện, toggle GPU bật lên, KHÔNG bấm tải → transcribe 1 file → verify fallback CPU (info message "GPU chưa sẵn sàng... dùng CPU"), KHÔNG crash.
6. **Tải CUDA runtime**: bấm nút tải → progress chạy 0→~99% → `cuda_runtime_status` true, UI hiện "sẵn sàng". Verify `<app_data>/cuda/` có DLL + marker, không còn `.whl`/`.part`.
7. **Transcribe GPU thật — PhoWhisper-large**: 1 file audio mẫu (≥2 phút) → verify info message "cuda (float16)", output text hợp lý (so sánh nhanh với bản CPU cùng file — không cần khớp 100%, chỉ cần không rác/không lệch nghĩa).
8. **Benchmark**: đo thời gian transcribe CÙNG 1 file trên CPU int8 vs GPU float16 (PhoWhisper-large) — ghi số liệu thật (thay cho số liệu tham khảo trong research report).
9. **Transcribe GPU — whisper-large-v3-turbo**: tải model turbo (Phase 03), transcribe 1 file Việt-Anh lẫn (nội dung DevOps/kỹ thuật) → verify chất lượng code-switch tốt hơn cảm nhận so PhoWhisper (kiểm tra chủ quan, không có metric tự động).
10. **Toggle GPU tắt/mở giữa 2 lần transcribe cùng size** → verify không dùng nhầm cache (info message đổi đúng theo device mỗi lần, xem Phase 02 risk).
11. **Glossary**: xoá `settings.json` (backup trước) → mở app → verify glossary DevOps mặc định hiện đúng (Phase 05).
12. Nếu bước 7 báo thiếu DLL (ví dụ lỗi "cudart64_12.dll not found") → quay lại Phase 01, thêm `"nvidia-cuda-runtime-cu12"` vào `CUDA_PACKAGES`, xoá `<app_data>/cuda/` cũ, lặp lại bước 6-7.

## Todo List
- [ ] `cargo check` + `npm run build` xanh
- [ ] Regression CPU (không GPU) OK
- [ ] Fallback CPU khi GPU chưa tải OK, không crash
- [ ] Tải CUDA runtime thành công, marker + DLL đúng chỗ
- [ ] Transcribe GPU PhoWhisper-large OK, info message đúng "cuda (float16)"
- [ ] Benchmark tốc độ CPU vs GPU — ghi số liệu thật
- [ ] Transcribe GPU turbo model OK
- [ ] Toggle cache-invalidation OK
- [ ] Glossary default OK trên profile sạch
- [ ] (Nếu cần) thêm `nvidia-cuda-runtime-cu12`, re-test

## Success Criteria
- Toàn bộ 12 bước checklist pass, không crash `CUBLAS_STATUS_NOT_SUPPORTED` ở bất kỳ bước nào.
- Có số liệu benchmark thật (thời gian transcribe CPU vs GPU, cùng file) ghi vào changelog/docs (không chỉ để trong đầu — cập nhật `docs/` nếu có mục liên quan, theo `documentation-management.md`).
- Máy không NVIDIA verify KHÔNG có bất kỳ thay đổi hành vi/UI nào so với trước cả 6 phase.

## Risk Assessment
| Risk | Likelihood × Impact | Mitigation |
|---|---|---|
| Thiếu `cudart64_12.dll` ngoài dự kiến (xem Phase 01) | Thấp × TB | bước 12 đã có quy trình fallback sẵn, không cần thiết kế lại |
| Benchmark cho thấy 5070 Ti không nhanh như kỳ vọng (issue #1287 research) | TB × Thấp | không phải blocker — vẫn nhanh hơn CPU nhiều, ghi nhận số liệu thật, không rollback feature |
| Turbo code-switch không "tốt hơn cảm nhận" như research kỳ vọng | Thấp × Thấp | không phải tiêu chí pass/fail cứng — ghi nhận, để user tự quyết dùng PhoWhisper hay turbo tuỳ file |

## Security Considerations
- Không áp dụng.

## Next Steps
- Sau khi pass toàn bộ → cập nhật `README.md` (mục "Sidecar" dòng 30-35 hiện tại) thêm 1 dòng về GPU CUDA + model turbo, theo `documentation-management.md` (docs-manager có thể làm riêng, ngoài scope code của plan này).
- Đóng plan — không có phase tiếp theo.

## Unresolved Questions
1. Root cause 5070 Ti chậm hơn 4070 Ti Super (faster-whisper #1287) — số liệu benchmark bước 8 sẽ cho biết có áp dụng cho máy này không, nhưng KHÔNG có action item để "sửa" (thuộc phía CTranslate2 upstream, ngoài tầm kiểm soát).
