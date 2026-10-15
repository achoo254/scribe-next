# Nguồn chép OpenAI (gpt-4o-mini-transcribe) cho Mode C

Tier: **M**, gồm 1 phase, đụng sidecar, Rust và giao diện. Trạng thái: ✅ code xong, đã kiểm chứng sidecar end-to-end; chờ QA trong app (keychain + UI).

## Mục tiêu

Mode C có thêm tuỳ chọn "Nguồn chép": **Offline** (mặc định, như hiện tại) hoặc **OpenAI gpt-4o-mini-transcribe**. Mục tiêu là có chất lượng gần TurboScribe với chi phí dưới $10/tháng. Bằng chứng: [báo cáo chẩn đoán](../reports/debugger-261015-1143-transcript-repetition-loop.md). Trên khúc 10–15', mini khớp TurboScribe 84,4% (offline 78%). Cả buổi 80 phút tốn khoảng $0,18.

## Quyết định đã chốt

- Chỉ áp cho Mode C (chép thủ công). Mode A (tự chép khi ghi âm) giữ offline, để không gửi audio ra ngoài ngầm.
- API key lưu trong OS keychain qua crate `keyring` (Keychain trên macOS, Credential Manager trên Windows). Frontend chỉ biết đã có key hay chưa, không bao giờ nhận lại giá trị key. Key đi từ Rust sang sidecar qua stdin (pipe), không qua argv hay env.
- Cắt audio tại khoảng lặng (silero VAD có sẵn) thành mảnh ≤ khoảng 45 giây. Lý do: với mảnh 2 phút, model mini từng lặp nguyên khối câu và chạm trần 2.048 token output. Với mảnh 40 giây thì sạch.
- Mảnh có dấu hiệu lặp (cụm 8 từ trùng > 3, hoặc tốc độ > 260 từ/phút) được gọi lại với temperature 0,4 rồi 0,8.
- Mốc thời gian: model mini không trả timestamp. Mốc từng từ được **ước lượng** bằng cách rải đều theo các vùng có tiếng nói (theo VAD) trong mảnh, sai số trong phạm vi một mảnh. UI ghi rõ mốc là ước lượng.
- Hiện chi phí ước tính trước khi chạy: tổng số phút × $0,003, lấy thời lượng qua `probe_file` có sẵn.
- Gọi API song song tối đa 4 mảnh một lúc.

## File

| File | Thay đổi |
|---|---|
| `sidecar/transcriber/openai_transcriber.py` (mới) | Cắt mảnh, nén Opus bằng PyAV, gọi API, chống lặp, ước lượng mốc từ |
| `sidecar/transcriber/main.py` | Thêm lệnh `transcribe_cloud`; dùng chung `_write_outputs` và message `done` |
| `sidecar/transcriber/transcriber.spec` | Thêm `openai_transcriber` vào `hiddenimports` |
| `src-tauri/Cargo.toml` | Thêm `keyring` |
| `src-tauri/src/openai_key.rs` (mới) | Đọc/ghi/xoá key trong keychain |
| `src-tauri/src/transcribe.rs` | `transcribe()` nhận `engine`; nhánh `openai` bỏ qua kiểm tra model, gửi `transcribe_cloud` |
| `src-tauri/src/commands.rs`, `lib.rs` | Command `openai_key_status`, `set_openai_key`, `delete_openai_key`; `transcribe_files` nhận thêm `engine` |
| `src/composables/use-transcribe.ts` | `engine` ref, truyền khi transcribe |
| `src/composables/use-openai-key.ts` (mới) | Trạng thái key và hàm lưu/xoá |
| `src/views/TranscriptView.vue` | Chọn nguồn, ô nhập key, chi phí ước tính, ẩn picker model khi dùng cloud |

## Tiêu chí nghiệm thu

1. Chọn OpenAI và nhập key một lần; mở lại app vẫn có key; xoá key được.
2. Chép một file bằng OpenAI ra đủ txt/srt/vtt/json; player click-to-seek chạy theo mốc ước lượng.
3. Key sai trả lỗi rõ ràng ("API key không hợp lệ"), hết credit trả "hết credit"; lỗi chỉ hỏng file đó, không làm kẹt batch.
4. Offline không đổi hành vi.
5. Gate: `cargo clippy` sạch, `vue-tsc --noEmit` sạch, chạy end-to-end thật một clip ngắn qua sidecar.

## Rủi ro

- Mốc thời gian chỉ là ước lượng, nên SRT kém chính xác hơn bản offline. Cách giảm: mảnh ngắn và rải theo vùng có tiếng nói.
- Audio rời máy. Chỉ chạy khi người dùng chủ động chọn OpenAI ở Mode C.

## Kết quả kiểm chứng (2026-10-15)

- `cargo clippy --all-targets`: không có warning. `cargo test`: 0 test (crate chưa có test). `vue-tsc --noEmit`: exit 0. `py_compile`: ok.
- Chạy end-to-end lệnh `transcribe_cloud` qua stdin của `main.py` thật, trên clip 10–15' (5 phút): mất 7 giây, 8 mảnh, ra đủ txt/srt/vtt/json, khớp TurboScribe 83,9%, lặp 0%. Mốc thời gian ước lượng lệch trung vị 1,0 giây, p90 3,5 giây.
- Key sai trả lỗi "OpenAI: API key không hợp lệ hoặc đã bị thu hồi", gắn đúng file, batch không kẹt.
- Chưa kiểm chứng: lưu/đọc/xoá key qua keychain và giao diện trong app thật. Việc này cần build lại app.
