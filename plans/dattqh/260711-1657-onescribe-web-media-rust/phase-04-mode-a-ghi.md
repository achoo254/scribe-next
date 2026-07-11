---
phase: 4
title: "Mode A — Ghi"
status: pending
priority: P2
effort: "3d"
dependencies: [3]
---

# Phase 4: Mode A — Ghi

## Overview

Ghi **mic / màn hình / tiếng hệ thống** trong browser (getUserMedia + getDisplayMedia + Web Audio mix) → MediaRecorder → upload lưu server + lịch sử + player nghe/xem lại. **Cắt live theo phút = ghi liền → server cắt** (tái dùng job queue Phase 3). **GATE kỹ thuật:** MediaRecorder webm thiếu cues → xác định pipeline cắt tin cậy trước khi hoàn thiện.

## Requirements

- **Functional:** ghi mic (mọi browser); ghi màn hình + system audio (Chrome/Edge); ghi liền → upload → lịch sử → nghe/xem lại; tuỳ chọn cắt theo phút → nhiều đoạn.
- **Non-functional:** recording dài an toàn (timeslice + upload nền, không mất khi tab crash); browser không hỗ trợ system audio → disclosure rõ; cắt recording đúng kỳ vọng (xem gate).

## Architecture

**Capture (FE, research):**
- Mic: `getUserMedia({audio})` → MediaRecorder `audio/webm;codecs=opus`.
- Màn hình + system audio: `getDisplayMedia({video, audio:{systemAudio:'include'}})`; mix system + mic qua **Web Audio API** (`createMediaStreamSource` → `MediaStreamAudioDestination`) → 1 track → `MediaStream([videoTrack, mixedAudio])` → MediaRecorder `video/webm;codecs=vp9,opus`.
- Browser support: Chrome/Edge system audio OK; **Firefox/Safari không có system audio** → disclosure "chỉ Chrome/Edge" + fallback mic-only.
- **Recording dài:** `MediaRecorder.start(1000)` (timeslice 1s) + upload chunk nền (queue + retry, bỏ blob 0-byte Firefox); KHÔNG single-blob cho ghi dài (mất khi crash). Track thời gian bằng timer riêng, KHÔNG đếm chunk.

**Upload + lưu:** tái dùng chunked upload Phase 3 → `files{kind:"recording"}`.

**⚠️ GATE — cắt recording webm (finding research, lật giả định "c-copy nhẹ"):**
MediaRecorder webm KHÔNG có seekable cues + keyframe interval không đều (~8-10s) → `ffmpeg -f segment -c copy` KHÔNG cắt đúng mốc.
- **Cắt theo PHÚT (mốc ≫ keyframe ~10s):** sai số ≤ vài giây/đoạn (chấp nhận) → **remux `-c copy -f matroska` fix cues rồi segment `-c copy`**. Nhẹ CPU. Output container cần TEST (mkv vs webm segment).
- **Nếu cần đúng chính xác từng giây:** phải `-c:v re-encode -force_key_frames "expr:gte(t,n_forced*60)" -c:a copy` → tốn CPU (2-3× realtime trên box .65 không GPU) → job nặng hơn, queue concurrency=1 + nice bắt buộc.
- **Quyết định mặc định (chờ user xác nhận ở validate):** cắt phút = remux+copy (nhẹ, sai số vài giây); KHÔNG re-encode trừ khi user cần chính xác tuyệt đối.
- **Test đầu Phase 4:** ghi thử webm thật (Chrome) → chạy cả 2 pipeline → đo sai số mốc + thời gian CPU → chốt.

**Player:** phát file qua `GET /api/files/:id` (stream, Range support) — thay `convertFileSrc` của desktop. Lịch sử = list `files` theo user.

## Related Code Files

- Create: `web/src/views/RecordView.vue`, `web/src/composables/use-recorder.ts` (getUserMedia/getDisplayMedia/MediaRecorder/mix/upload), `web/src/components/RecordingHistory.vue`, `web/src/components/MediaPlayer.vue`, `server/onescribe-be/src/files.rs` (stream + Range)
- Modify: `server/onescribe-be/src/worker/split_worker.rs` (nhánh recording webm: remux/re-encode), `crates/core-split/src/lib.rs` (thêm option remux/force-keyframe cho webm), `server/onescribe-be/src/routes.rs`
- Delete: none

## Implementation Steps

1. **GATE test trước:** ghi webm thật (mic + màn hình Chrome) → thử remux+copy segment vs re-encode force_key_frames → đo sai số mốc + CPU trên .65 → chốt pipeline + container.
2. FE `use-recorder`: getUserMedia mic; getDisplayMedia + Web Audio mix system+mic; MediaRecorder timeslice 1s + upload queue nền; disclosure browser.
3. RecordView: chọn nguồn (mic/màn hình/system), toggle cắt theo phút + giá trị, nút Ghi/Dừng, level meter (tùy chọn).
4. Upload recording → `files{kind:recording}`; nếu bật cắt → tạo split job (nhánh webm theo gate).
5. `files.rs`: stream file + HTTP Range (player tua); ownership.
6. RecordingHistory + MediaPlayer (nghe/xem lại, click đoạn → tua).
7. E2E: ghi mic → lịch sử → nghe lại; ghi màn hình+system (Chrome) → cắt phút → đoạn OK; Firefox → disclosure.

## Success Criteria

- [ ] Ghi mic (mọi browser) → upload → lịch sử → nghe lại.
- [ ] Ghi màn hình + system audio (Chrome/Edge) → upload → xem lại có tiếng; Firefox/Safari → disclosure + fallback mic.
- [ ] Ghi dài (>30') không mất khi tab reload giữa chừng (timeslice + upload nền).
- [ ] **Gate chốt:** pipeline cắt recording webm cho sai số mốc chấp nhận được (phút) + CPU đo thật trên .65; ghi rõ trade-off copy vs re-encode.
- [ ] Player stream + Range (tua) hoạt động; ownership chặn cross-user.

## Risk Assessment

- **[GATE] webm `-c copy` không cắt đúng mốc:** đã phân tích (research); mặc định remux+copy cho phút, re-encode chỉ khi cần chính xác. **Test trước khi build UI.**
- **Re-encode đè CPU box .65 (không GPU):** nếu buộc re-encode → nặng; giữ concurrency=1 + nice + cảnh báo thời gian cho user; cân nhắc giới hạn thời lượng recording cắt.
- **getDisplayMedia system audio chỉ Chrome/Edge:** disclosure rõ ràng, không hứa parity.
- **MediaRecorder chunk 0-byte / chunk 50MB (Firefox/wake):** bỏ chunk size 0; upload nền chịu chunk lớn.
- **Recording dài chiếm disk:** quota + retention (Phase 5); cảnh báo dung lượng.
