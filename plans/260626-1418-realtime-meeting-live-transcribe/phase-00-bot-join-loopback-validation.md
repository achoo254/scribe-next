---
phase: 0
title: "Bot-join / Loopback Value Validation"
status: pending
priority: P1
effort: "1-2d"
dependencies: []
---

# Phase 0: Bot-join / Loopback Value Validation — VALUE GATE

## Overview

Trước khi cam kết 24-37 ngày xây platform, **chứng minh rẻ (1-2 ngày)** rằng giá trị cốt lõi — *transcribe tiếng Việt cuộc họp* — thực sự hữu ích, bằng cách **tái dùng nguyên WASAPI loopback + sidecar ĐÃ CÓ SẴN** để bắt tiếng một cuộc Meet/Zoom thật và đo chất lượng transcript. KHÔNG cần SFU, không code mới đáng kể.

Đây là **value gate** (khác với Phase 1 = feasibility gate kỹ thuật). NO-GO ở đây → cân nhắc lại có nên xây platform hay chỉ làm "companion transcribe" cho Meet/Zoom có sẵn.

## Requirements

- **Functional:** mở 1 cuộc Meet/Zoom thật (vài người nói tiếng Việt) → app bắt **tiếng hệ thống (loopback)** → chạy transcribe → xuất transcript. Đo chất lượng + độ trễ cảm nhận.
- **Non-functional:** dùng đúng pipeline hiện tại (loopback + sidecar batch theo đoạn), không xây mới; đo trên máy host mục tiêu (laptop văn phòng).

## Architecture

```
Cuộc Meet/Zoom (browser/app) phát tiếng ra loa
        │ WASAPI loopback (ĐÃ CÓ — src-tauri/src/audio/loopback.rs)
        ▼
Ghi đoạn theo phút → sidecar transcribe (ĐÃ CÓ — main.py batch) → .txt/.srt
        ▼
Đánh giá: WER cảm nhận, tên riêng/thuật ngữ, độ trễ "đủ dùng" cho biên bản?
```

- Đây mô phỏng "companion mode": không cần platform riêng, chỉ cần máy có app + đang trong cuộc họp bất kỳ.

## Related Code Files

- Read/Reuse (KHÔNG sửa nhiều): `src-tauri/src/audio/loopback.rs` (WASAPI loopback Windows), `sidecar/transcriber/main.py` (batch transcribe), `src/composables/use-record-transcribe.ts` (auto-transcribe khi ghi).
- Create: `plans/260626-1418-realtime-meeting-live-transcribe/reports/phase-00-bot-join-value-validation-results.md` — kết luận GO/NO-GO + mẫu transcript thật.

## Implementation Steps

1. Mở 1 cuộc Meet/Zoom thật (hoặc video họp tiếng Việt) phát ra loa; dùng chế độ ghi "Tiếng hệ thống" (loopback) hiện có để bắt + auto-transcribe theo đoạn (medium).
2. Thu transcript của ≥2 cuộc/đoạn hội thoại tiếng Việt đa người, đa chủ đề (có thuật ngữ kỹ thuật).
3. Đánh giá chất lượng: độ chính xác cảm nhận, tên riêng/tiếng Anh lẫn lộn, mất chữ khi nói nhanh/chồng; so với kỳ vọng "biên bản đủ dùng".
4. Ghi nhận hạn chế của companion-mode (không tách được người nói vì loopback là 1 luồng trộn) → đây CHÍNH là lý do cần platform per-track ở các phase sau (xác nhận động lực).
5. Viết báo cáo GO/NO-GO: nếu transcript "đủ dùng" → giá trị được chứng minh, tiếp Phase 1. Nếu transcript quá kém ngay cả batch medium → xem lại model/định hướng TRƯỚC khi xây platform.

## Success Criteria

- [ ] Có transcript thật của ≥2 cuộc họp tiếng Việt bắt qua loopback.
- [ ] Đánh giá định tính chất lượng "đủ dùng cho biên bản" hay không (có mẫu kèm).
- [ ] Xác nhận động lực platform: companion-mode KHÔNG tách được người nói (1 luồng trộn) → per-track của platform là giá trị tăng thêm rõ ràng.
- [ ] Kết luận GO/NO-GO cho việc xây platform, có lý do.

## Risk Assessment

- **Loopback bắt cả tiếng mình + người khác lẫn nhau** → mô phỏng đúng giới hạn "1 luồng trộn", đủ để đánh giá chất lượng ASR (không đánh giá diarization ở đây).
- **Chỉ Windows** (loopback hiện chỉ Windows) — đủ cho validate; không chặn quyết định.
- Đây là phase RẺ, không over-invest. Mục tiêu duy nhất: quyết định có đáng xây platform không.
