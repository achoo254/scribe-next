# Research — Browser capture + ffmpeg webm split + upload

**Date:** 2026-07-11 · Confidence ~85% (MDN + ffmpeg + Chromium issue). Snippet minh hoạ.

## 1. MediaRecorder (mic)
- MIME: `audio/webm;codecs=opus` (Chromium/Firefox); Safari chỉ `audio/mp4` (AAC) → `isTypeSupported` fallback.
- Ghi dài: `start(1000)` (timeslice 1s) + upload chunk nền → không mất khi tab crash. Bỏ chunk `size===0` (bug Firefox Windows). KHÔNG single-blob cho ghi dài. Track thời gian bằng timer riêng (chunk không đều size/timing).

## 2. getDisplayMedia (màn hình + system audio)
- `getDisplayMedia({video, audio:{systemAudio:'include'}})` — **chỉ Chrome/Edge** có system/tab audio; **Firefox/Safari KHÔNG** → disclosure + fallback mic-only.
- Mix system + mic: Web Audio `createMediaStreamSource` x2 → `MediaStreamAudioDestination` → 1 track → `new MediaStream([videoTrack, mixedAudio])` → MediaRecorder `video/webm;codecs=vp9,opus`. (MediaRecorder chỉ nhận 1 audio track → phải mix.)

## 3. ⚠️ CRITICAL — cắt webm không re-encode (lật giả định)
**MediaRecorder webm KHÔNG có seekable Cues + keyframe interval không đều (~8-10s)** → `ffmpeg -f segment -c copy` cắt tại keyframe kế, KHÔNG đúng mốc.
- **`-c copy` thô:** FAIL cho mốc chính xác (đoạn theo keyframe). Nếu segment_time < keyframe interval → đoạn to bằng keyframe interval, không phải giá trị yêu cầu.
- **Cắt theo PHÚT (mốc 60s ≫ keyframe ~10s):** sai số ≤ vài giây/đoạn → chấp nhận. Pipeline: **remux fix cues `ffmpeg -i in.webm -c copy -f matroska fixed.mkv` → segment `-c copy`**. Nhẹ CPU. Container output cần TEST (mkv ổn định hơn webm cho segment muxer).
- **Cần đúng chính xác từng giây:** phải re-encode video `-c:v libvpx-vp9 -force_key_frames "expr:gte(t,n_forced*60)" -c:a copy` → đúng mốc nhưng **2-3× realtime CPU** (nặng trên .65 không GPU).
- **Verdict:** KHÔNG dựa `-c copy` cho mốc chính xác với recording webm. Mặc định plan: remux+copy cho cắt-phút (sai số vài giây), re-encode chỉ khi user cần chính xác.
- **File UPLOAD (Mode B) KHÔNG bị:** file người dùng (mp4/mkv/mp3) có cues/keyframe chuẩn → `-c copy` cắt đúng như desktop `splitter.rs`.

## 4. Upload lớn (hàng trăm MB)
- **Khuyến nghị: chunked POST** (KISS) thay tus: `file.slice()` 5MB/chunk → POST `{sessionId, chunkIndex, totalChunks, chunk}` → BE **append thẳng disk theo offset** (⚠️ NON giữ chunk trong RAM như ví dụ HashMap — box yếu OOM) → finalize assemble+validate. Progress qua XHR upload events. Retry chunk lỗi.
- tus = chuẩn interop (mobile+web) nhưng thêm overhead HEAD; không cần cho single web client.

## Unresolved / cần TEST
- webm segment container (mkv vs webm) + sai số mốc thực đo trên .65 — **gate Phase 4**.
- Re-encode VP9 thời gian thực trên CPU Broadwell .65 — đo trước khi hứa.
- Firefox audio mix stereo/mono; ts-ebml client-side cost (không dùng, để server lo).

## Sources
- MDN MediaRecorder / getDisplayMedia · Chromium issue #40482588 (seekable WebM) · FFmpeg segment muxer guide · addpipe getDisplayMedia + huge-chunks blog · tus.io spec · imfeld.dev axum streaming upload · fix-webm-meta (ts-ebml).
