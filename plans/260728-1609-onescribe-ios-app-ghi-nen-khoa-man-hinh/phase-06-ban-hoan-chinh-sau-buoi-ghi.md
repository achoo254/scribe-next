---
phase: 6
title: "Bản transcript hoàn chỉnh sau buổi ghi"
status: pending
priority: P2
effort: "M"
dependencies: [2, 5]
---

# Phase 6: Bản transcript hoàn chỉnh sau buổi ghi

## Overview

Khi phiên ghi kết thúc và mọi đoạn AAC đã upload (Phase 2), backend chép lại toàn bộ file bằng `gpt-4o-mini-transcribe` theo mảnh ≤45 giây, rồi gắn txt/srt/json vào bản ghi trong Lịch sử. Bản này sửa các chỗ realtime nghe kém hơn (khớp TurboScribe 84,4% so với 78,9%) và bù các đoạn hụt khi mất mạng.

Logic đã có bản Python được kiểm chứng ở `sidecar/transcriber/openai_transcriber.py`: cắt tại khoảng lặng, nén Opus, gọi song song 4 mảnh, mảnh lặp thì gọi lại với temperature cao hơn, mốc từ ước lượng theo vùng có tiếng nói (lệch trung vị 1 giây). Phase này chuyển logic đó sang Rust trong `TranscribeWorker` (hiện là stub ở `server/onescribe-be/src/worker/mod.rs`), dùng ffmpeg `silencedetect` thay silero để không phải cài Python lên server.

## Requirements

- Tự tạo job transcribe khi phiên ghi đóng, nếu realtime đang bật cho phiên đó (R5 của Phase 5). Có nút "Chép lại" thủ công trong Lịch sử ghi.
- Kết quả hiện trong Lịch sử ghi: xem chữ, bấm câu để tua tới đoạn đó, tải txt/srt.
- Tính phút vào cùng trần chi phí với Phase 5.
- Lỗi OpenAI (key sai, hết credit) trả thông báo rõ ràng, không làm kẹt hàng đợi job.

## Related code files

- Modify: `server/onescribe-be/src/worker/mod.rs` — `TranscribeWorker` thật (có thể tách `worker/transcribe_worker.rs`)
- Modify: `server/onescribe-be/src/jobs.rs` — job type `transcribe` cho `kind:recording`
- Modify: `server/onescribe-be/Cargo.toml` — `reqwest` thêm feature `multipart`
- Modify: `web/src/components/RecordingHistory*` — hiện transcript + tải file

## Success Criteria

- [ ] Chép lại 5 phút đoạn 10–15' buổi họp Viettel: khớp TurboScribe ≥ 83%, lặp 0% (bản Python đạt 83,9–84,4%).
- [ ] Job lỗi do key sai: thông báo đúng, job sau vẫn chạy.
- [ ] RAM `onescribe-be` trên `.65` khi chạy job 80 phút không tăng quá 150 MB so với lúc rảnh.
- [ ] `cargo clippy` sạch, `cargo test` pass (thêm test cho cắt mảnh và ước lượng mốc từ).
