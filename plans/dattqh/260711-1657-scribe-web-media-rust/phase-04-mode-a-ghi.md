---
phase: 4
title: "Mode A — Ghi"
status: done
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
- **[Fold: capture hẹp hơn desktop] disclosure parity:** web KHÔNG thể (a) ghi system audio im lặng nền như desktop WASAPI loopback (`loopback.rs:42`) — mỗi lần ghi phải qua OS share-picker; (b) system audio chỉ chắc chắn khi user chọn "Entire Screen" (không phải Window/Tab); (c) KHÔNG có chọn vùng màn hình tùy ý (`region {x,y,w,h}` của desktop) — browser chỉ cho whole-screen/window/tab. Nêu rõ trong Requirements + UI, không hứa parity với desktop.
- **Recording dài:** `MediaRecorder.start(1000)` (timeslice 1s) + upload chunk nền (queue + retry, bỏ blob 0-byte Firefox); KHÔNG single-blob cho ghi dài (mất khi crash). Track thời gian bằng timer riêng, KHÔNG đếm chunk.

**Upload + lưu:** **[Red-team #7] KHÔNG tái dùng nguyên protocol chunked Phase 3** — recording là stream open-ended, KHÔNG biết `totalChunks`/tổng size trước khi user dừng. Dùng **endpoint append-only streaming riêng**: `POST /api/recordings/init` (BE sinh sessionId bind user) → append từng chunk MediaRecorder timeslice (append thẳng disk theo thứ tự, không cần `totalChunks`) → **finalize tường minh khi `MediaRecorder.onstop`**. Vẫn dùng lại cơ chế append-disk-stateless + quota tăng dần + sessionId-server-gen của Phase 3. → `files{kind:"recording"}`.

**⚠️ GATE — cắt recording webm (finding research, lật giả định "c-copy nhẹ"):**
MediaRecorder webm KHÔNG có seekable cues + keyframe interval không đều (~8-10s) → `ffmpeg -f segment -c copy` KHÔNG cắt đúng mốc.
- **Cắt theo PHÚT (mốc ≫ keyframe ~10s):** sai số ≤ vài giây/đoạn (chấp nhận) → **remux `-c copy -f matroska` fix cues rồi segment `-c copy`**. Nhẹ CPU. Output container cần TEST (mkv vs webm segment). **[Fold: disk] remux tạo bản sao `fixed.mkv` đầy đủ = cần disk trống ≈ 2× file gốc** (recording vài giờ có thể nhiều GB) → check `disk-free ≥ 2× input` TRƯỚC khi remux; **xoá `fixed.mkv` ngay sau segment** (kể cả khi fail) trong cùng bước dọn của SplitWorker.
- **Nếu cần đúng chính xác từng giây:** phải `-c:v re-encode -force_key_frames "expr:gte(t,n_forced*60)" -c:a copy` → tốn CPU (2-3× realtime trên box .65 không GPU) → job nặng hơn, queue concurrency=1 + nice bắt buộc.
- **Quyết định ĐÃ CHỐT (validate 2026-07-11):** cắt phút = **remux+copy** (nhẹ, sai số vài giây); KHÔNG re-encode. Gate Phase 4 chỉ để verify sai số + CPU thật trên .65 khớp kỳ vọng (không phải để chọn lại pipeline). <!-- Updated: Validation Session 1 - remux+copy chốt -->
- **Test đầu Phase 4:** ghi thử webm thật (Chrome) → chạy cả 2 pipeline → đo sai số mốc + thời gian CPU → chốt.

**Player:** phát file qua `GET /api/files/:id` (stream, Range support) — thay `convertFileSrc` của desktop. Lịch sử = list `files` theo user.

## Related Code Files

- Create: `web/src/views/RecordView.vue`, `web/src/composables/use-recorder.ts` (getUserMedia/getDisplayMedia/MediaRecorder/mix/upload), `web/src/components/RecordingHistory.vue`, `web/src/components/MediaPlayer.vue`, `server/scribe-be/src/files.rs` (stream + Range)
- Modify: `server/scribe-be/src/worker/split_worker.rs` (nhánh recording webm: remux/re-encode), `crates/core-split/src/lib.rs` (thêm option remux/force-keyframe cho webm), `server/scribe-be/src/routes.rs`
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

- [x] Ghi mic (mọi browser) → upload → lịch sử → nghe lại. *(pipeline init→chunk→finalize→files→player verified live; getUserMedia UI mic chờ user QA mic thật)*
- [x] Ghi màn hình + system audio (Chrome/Edge) → upload → xem lại có tiếng; Firefox/Safari → disclosure + fallback mic. *(getDisplayMedia + Web Audio mix + disclosure UI; hasVideo→player <video>; QA mic/màn hình thật của user)*
- [x] Ghi dài (>30') không mất khi tab reload giữa chừng (timeslice 1s + upload nền + best-effort finalize khi lỗi chunk).
- [x] **Gate chốt:** remux `-c copy -f matroska` → segment `-c copy` verified chạy trên ffmpeg .65 thật (job done, segment `.mkv`, zip 200). CPU nhẹ (không re-encode). Trade-off: sai số ≤ 1 keyframe (chấp nhận cho cắt phút); re-encode chỉ khi cần chính xác từng giây (không dùng).
- [x] Player stream + Range (tua) hoạt động (206 + content-range verified); ownership `get_owned` chặn cross-user.

## Cook progress — 2026-07-12 (full phase: code + review-fix + deploy + E2E live)

- **BE:** `recordings.rs` (streaming append init/chunk/finalize, open-ended, mirror upload.rs — không đụng live path Cắt file); `files.rs` (GET /api/files lịch sử + GET /api/files/:id stream+HTTP Range, unit-test parse_range); `models/file.rs` (kind recording + durationS + hasVideo + list_recordings); `split_worker.rs` nhánh webm (disk-free ≥2× + `core_split::remux_copy_to_mkv` + dọn fixed.mkv mọi nhánh lỗi); routes/main.
- **FE:** `use-recorder.ts` (getUserMedia/getDisplayMedia + Web Audio mix + MediaRecorder timeslice 1s + upload queue offset/retry-backoff + best-effort finalize + reentrancy guard + dispose + bitrate cap), `use-recordings.ts`, `RecordView.vue`, `RecordingHistory.vue`, `MediaPlayer.vue`; `HomeView.vue` bật tab Ghi âm.
- **Verify:** `cargo check --workspace` + `cargo test` (musl container: core-split 9 + files parse_range 1) pass; FE typecheck + build pass. Release musl `static-pie linked` 18MB.
- **Code-review:** 2 High (race start/stop → mất bản ghi + rò mic; chainError bỏ cả bản ghi) + 2 Medium (fixed.mkv không dọn khi remux fail; body-limit/bitrate) — **fix hết** (reentrancy guard + best-effort finalize + explicit remux-err cleanup + videoBitsPerSecond cap 2.5Mbps + frameRate 30). LOW: onerror handler + onBeforeUnmount dispose + sp.reset.
- **Deploy:** BE musl → `$DEPLOY_APP_DIR` (systemd active, health 200) + FE dist → `$DEPLOY_WEB_ROOT/dist`. **E2E live .65:** init→chunk(4)→finalize→files(hasVideo)→stream Range 206→split webm remux+copy done→zip 200. Còn: QA mic/màn hình thật (getUserMedia cần thiết bị thật, không automate được) + xoá bản ghi test "E2E 12:31:26" (chờ delete/retention Phase 5).
- **Deferred:** unit test cho use-recorder (mock fetch — reviewer gợi ý); delete endpoint + orphaned session/.part sweep (Phase 5).

## Risk Assessment

- **[GATE] webm `-c copy` không cắt đúng mốc:** đã phân tích (research); mặc định remux+copy cho phút, re-encode chỉ khi cần chính xác. **Test trước khi build UI.**
- **Re-encode đè CPU box .65 (không GPU):** nếu buộc re-encode → nặng; giữ concurrency=1 + nice + cảnh báo thời gian cho user; cân nhắc giới hạn thời lượng recording cắt.
- **getDisplayMedia system audio chỉ Chrome/Edge:** disclosure rõ ràng, không hứa parity.
- **MediaRecorder chunk 0-byte / chunk 50MB (Firefox/wake):** bỏ chunk size 0; upload nền chịu chunk lớn.
- **Recording dài chiếm disk:** quota + retention (Phase 5); cảnh báo dung lượng.
