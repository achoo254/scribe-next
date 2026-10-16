# Red-Team Plan Review — ASSUMPTION DESTROYER: Scribe Web Pivot

**Reviewer:** code-reviewer (adversarial, assumption-destroyer persona)
**Target:** `plans/260707-1325-scribe-web-pivot/` (plan.md + phase-01..05)
**Date:** 2026-07-07
**Scope note:** Không tấn công quyết định user đã chốt (web thay desktop, small/medium, ephemeral, meeting gộp MVP). Chỉ tấn công giả định THỰC THI. Mọi finding có evidence file:line grep-verified.

---

## Finding 1: Stream process dùng chung = namespace track PHẲNG toàn cục → caption của user A rò sang user B (silent)

- **Severity:** Critical
- **Location:** phase-03 architecture (dòng 29-36) + phase-04 step 3 (dòng 49); `sidecar/transcriber/stream_server.py`
- **Flaw:** Phase 3 chốt "persistent `python3 main.py --stream` (spawn 1 lần)" dùng chung cho MỌI phiên live recording + meeting bot (Phase 4). Nhưng sidecar định tuyến PCM và caption **chỉ bằng track id trong 1 dict toàn cục**: `_tracks: dict[str, "_Track"]` (stream_server.py:39), `_stream_pcm` lookup thuần theo id (stream_server.py:130-137), và `_stream_start` **idempotent im lặng** khi id trùng (`if track_id in _tracks: return` — stream_server.py:123-124). Sidecar được viết cho 1 chủ duy nhất (1 host desktop qua Rust); plan chuyển sang multi-tenant mà không nói một chữ nào về namespace track per-session hay ownership map ở ws.mjs (phase-03 step 2 chỉ ghi "map WS session ↔ track id" — không có scheme).
- **Failure scenario:** 2 user cùng mở RecordLivePage; bridge đặt track id ngây thơ = `"mic"` (hoặc 2 participant identity sanitize trùng nhau). `stream_start` thứ 2 return im lặng, không error. PCM của user B đổ vào track của user A; caption chứa **nội dung nói của B** phát về WS của A. Rò dữ liệu giọng nói/nội dung giữa 2 người dùng, không có bất kỳ signal lỗi nào để phát hiện.
- **Evidence:** `sidecar/transcriber/stream_server.py:39,123-124,130-137`; `phase-03-live-caption-recording.md:29-36,49`; tiền lệ sanitize/collision đã phải xử lý ở desktop: `src/composables/use-live-caption.ts:25-28` (`safeTrackId`).
- **Suggested fix:** Spec bắt buộc trong phase-03: ws.mjs namespace track id = `<sessionId>-<trackId>` (server sinh sessionId), registry giữ ownership map session→track; caption route ngược theo prefix. Thêm test bridge: 2 session cùng track name → caption không cross. Bỏ giả định "sidecar KHÔNG cần sửa" khỏi lý do thiết kế (xem Finding 3).

## Finding 2: Cơ chế phát caption realtime của Phase 4 KHÔNG tồn tại — bot `canPublishData=false` + client chỉ nhận data-channel từ role=host

- **Severity:** Critical
- **Location:** phase-04 architecture (dòng 32-34), step 2 (dòng 48), step 5 (dòng 51), step 6 (dòng 52) + Modify list (dòng 42)
- **Flaw:** Ba mảnh của phase 4 mâu thuẫn nhau, không tồn tại đường đi hợp lệ nào cho caption realtime tới client:
  1. Token bot: `canPublishData=false` (step 2) → bot KHÔNG thể đẩy caption qua LiveKit data channel.
  2. Đường thay thế theo sơ đồ: "caption → caption store (ĐÃ CÓ) → web clients" — nhưng caption store hiện tại là **poll-history-only**: GET trả toàn bộ history (`index.mjs:143-144`), POST đòi `role === "host"` (`index.mjs:147`), KHÔNG có push channel (SSE/WS) nào tới client.
  3. Client render: `useCaptionFeed` nhận realtime DUY NHẤT qua `RoomEvent.DataReceived` với filter `isHostSender` — đòi `metadata.role === "host"` (`caption-feed.ts:14-23,44-53,60-62`). Caption từ bot (role=bot) hay bất kỳ nguồn nào khác đều bị **drop im lặng**. Và `caption-feed.ts` KHÔNG nằm trong Modify list của phase-04 (dòng 42 chỉ có meeting-client.ts, use-livekit-viewer.ts) — step 6 còn ghi "caption overlay (caption-feed.ts đã có)".
- **Failure scenario:** Build đúng theo phase file: bot ghi caption vào store, không client nào render realtime (chỉ thấy khi rejoin qua seedHistory). Hoặc dev "chữa cháy" bằng cách bật `canPublishData=true` cho bot — phá spec token step 2, và client vẫn drop vì role=bot ≠ host; sửa tiếp filter caption-feed = thay đổi contract C7 không có test cập nhật.
- **Evidence:** `phase-04-meeting-web-caption-bot.md:32-34,42,48,51-52`; `packages/meeting-shared/src/caption-feed.ts:4,14-23,44-53,60-62`; `server/meeting-server/index.mjs:143-147`; `server/meeting-server/lib/jwt.mjs:30-31,70-73`.
- **Suggested fix:** Chốt 1 trong 2 đường trong phase file, kèm file phải sửa: (a) bot token `canPublishData=true` + caption-feed filter chấp nhận `role ∈ {host, bot}` + cập nhật test C7 chống giả mạo; hoặc (b) server đẩy caption qua WS/SSE room-scoped (mới) và caption-feed thêm nguồn ingestion không-LiveKit. Cả 2 đều phải thêm `caption-feed.ts` vào Modify list.

## Finding 3: "Sidecar KHÔNG cần sửa" + "--cpu-threads 2" là mâu thuẫn tự hủy — main.py KHÔNG có cách nào set 2 threads

- **Severity:** High
- **Location:** phase-01 step 5 (dòng 37), phase-02 architecture (dòng 31-32), phase-03 step 2 (dòng 48); `sidecar/transcriber/main.py`, `stream_server.py`, `stream_session.py`
- **Flaw:** main.py chỉ parse argv cho `--stream` (main.py:142); flag `--cpu-threads` **không tồn tại** và sẽ bị nuốt im lặng. Batch hardcode `cpu_threads=os.cpu_count() or 8` (main.py:67). Stream: `_ensure_model` gọi `load_stream_model(model_path)` KHÔNG truyền threads (stream_server.py:56) → default `os.cpu_count()` (stream_session.py:29-38). Trên box 4 vCPU, mọi đường production chạy **4 threads**, không phải 2. Đồng thời protocol batch KHÔNG có event `transcribe_msg` như phase-02 mô tả — event thật là `ready/download/model_ready/progress/done/error/info` (main.py:5-8); `transcribe_msg` là tên event Tauri phía Rust, không phải stdio.
- **Failure scenario:** Phase 1 chạy "main.py với --cpu-threads 2" → flag bị bỏ qua, số RTF/RAM đo ở 4 threads nhưng report ghi "2 threads" → **GO/NO-GO gate quyết định trên số sai**. Production: 4 threads dưới CPUQuota=200% = 4 threads chen trên 2 CPU-worth → context-switch, latency cliff không có trong benchmark; hoặc không quota → chiếm cả 4 vCPU, degrade ~15 service (vi phạm chính non-functional requirement của phase-01 dòng 19).
- **Evidence:** `sidecar/transcriber/main.py:5-8,67,142`; `stream_server.py:56`; `stream_session.py:29-38`; `phase-01-benchmark-asr-gate.md:37`; `phase-02-batch-transcribe-core.md:31-32,49`; `phase-03-live-caption-recording.md:48`. (Spike harness CÓ `--cpu-threads` — `stream_asr_spike.py:48` — nhưng đó không phải code production.)
- **Suggested fix:** Chấp nhận sửa sidecar (thêm `cpu_threads` vào cmd JSON hoặc CLI flag cho cả batch lẫn `--stream` — thay đổi nhỏ, backward-compatible) và bỏ claim "KHÔNG sửa". Phase 1 phải benchmark **đúng invocation production** (main.py protocol, không chỉ spike harness). Sửa mô tả protocol trong phase-02 cho khớp main.py.

## Finding 4: Ngân sách RAM mâu thuẫn với chính spike report plan trích dẫn — MemoryMax=2500M sẽ OOM-kill cả cây service giữa job

- **Severity:** High
- **Location:** plan.md dòng 57-64 (bảng ngân sách), phase-02 dòng 42 (`MemoryMax=2500M`), phase-03 risk dòng 68
- **Flaw:** Plan ước "stream small ~600MB, batch medium ~1.5GB, peak ~2.2GB". Spike report (chính là "Tham khảo mẫu đo" của phase-01 dòng 29) đo được: **stream process với small = peak RSS 940–1031MB** (§9: 940/1026/965/1031MB), và **small+medium đồng thời = 2085.7MB với small alone 349.9MB → medium ≈ 1.74GB** chỉ mới load model, chưa tính buffer decode file 60' (~230MB float32 PCM + working set). Peak thực khi "2 phiên live + 1 batch" ≈ 1.0 + 1.8~2.0 + 0.15 (Node) ≈ **2.9–3.2GB**, không phải 2.2GB. systemd `MemoryMax=2500M` gộp Node + mọi python con trong cùng cgroup.
- **Failure scenario:** Batch 60' đang chạy ở phút 40, user mở 2 phiên live → cgroup chạm 2500M → kernel OOM-kill process lớn nhất trong cgroup (python medium) hoặc systemd kill cả service → job fail + caption chết cùng lúc. Guard của phase-03 (đọc `/proc/meminfo` MemAvailable — dòng 68) **không nhìn thấy giới hạn cgroup**: box còn 3.2GB available nhưng cgroup đã nghẹt → guard cho qua, vẫn chết.
- **Evidence:** `plan.md:57-64`; `phase-02-batch-transcribe-core.md:42`; `phase-03-live-caption-recording.md:68`; `plans/260626-1418-realtime-meeting-live-transcribe/reports/spike-streaming-asr-results.md:68,113-119`.
- **Suggested fix:** Phase 1 bổ sung đo RSS per-process cho: batch medium trên file 60' + stream small 2 track ĐỒNG THỜI, ghi tổng cgroup. Rebudget bảng plan.md từ số đo. Guard registry đọc `memory.current`/`memory.max` của cgroup thay vì `/proc/meminfo`. Cân nhắc tách batch và stream thành 2 slice systemd riêng để 1 bên chết không kéo bên kia.

## Finding 5: "Đường worklet→PCM đã chứng minh" là claim SAI — G6 chưa bao giờ QA runtime, và code "decimation + low-pass" để port KHÔNG tồn tại

- **Severity:** High
- **Location:** phase-03 architecture dòng 25-26, Related Code Files dòng 41, risk dòng 67
- **Flaw:** Hai claim đều fail khi grep:
  1. "đường worklet→PCM **đã chứng minh**": plan cũ ghi rõ G6 = "CODE DONE 2026-06-29, **chờ QA runtime**" (260626 plan.md:45) và phase 3 desktop "runtime WebView2/E2E = checklist Windows" chưa chạy (260626 plan.md:47); spike report §4 G6 = 🟡, §9 "G6... **vẫn chưa làm**, cần app chạy. Đây là gate kỹ thuật còn lại lớn nhất" (spike report:71,132). Compile ≠ chứng minh.
  2. Risk mitigation "dùng đúng code G6 đã viết (**decimation + low-pass**) thay viết mới": grep toàn repo không có decimation/low-pass nào. Code thật chỉ ép `new AudioContext({ sampleRate: 16000 })` và **hy vọng** engine honor (`use-audio-tap-probe.ts:82` — kèm comment "nếu WebView2 không honor, ghi nhận actualSampleRate"); cả 2 worklet chỉ buffer + convert int16, không resample (`pcm-tap-worklet.js`, `meeting-audio-tap-worklet.js:4-5`).
- **Failure scenario:** Phase 3 lên lịch "PORT" một thứ không có. Nếu đường AudioContext@16k gặp vấn đề với `getDisplayMedia` system-audio trên Chrome (tab-audio 48k, track constraints khác mic — chưa ai thử), mitigation ghi trong risk là ngõ cụt: không có downsampler nào để port, phải viết + test mới giữa chừng phase — đúng loại "sẽ ổn thôi" làm nổ timeline phase đã gộp meeting MVP.
- **Evidence:** `phase-03-live-caption-recording.md:25-26,41,67`; `src/composables/use-audio-tap-probe.ts:81-84`; `public/pcm-tap-worklet.js:3`; `public/meeting-audio-tap-worklet.js:4-5`; `plans/260626-1418-realtime-meeting-live-transcribe/plan.md:45,47`; `spike-streaming-asr-results.md:71,132`.
- **Suggested fix:** Sửa claim thành "port pattern, chưa từng chạy runtime". Thêm mini-spike đầu phase-03 (giống gate phase-04 step 1): Chrome mic + getDisplayMedia system-audio → AudioContext@16k → dump PCM → verify bằng tai + `actualSampleRate`. Nếu 16k không honor → viết downsampler trong worklet (việc mới, tính vào effort).

## Finding 6: `git mv meeting-server → scribe-server` phá 3 consumer hard-coded KHÔNG có trong Modify list

- **Severity:** High
- **Location:** phase-02 step 1 (dòng 48) + Related Code Files (dòng 42-44)
- **Flaw:** Phase-02 Modify list chỉ có `server/meeting-server/` (move), `apps/web`, `packages/meeting-shared`. Grep-enumerate consumer thật (xem bảng Verification bên dưới) ra **3 hard break** không được liệt kê:
  1. `server/dev-token/generate-token.mjs:6` — `import { signToken } from "../meeting-server/lib/jwt.mjs"` → import chết ngay sau mv. Phase-04 còn dựa vào file này ("Đọc tham khảo" dòng 43).
  2. `infra/run-local.sh:27` — `node server/meeting-server/index.mjs` → dev loop local chết.
  3. `infra/systemd/meeting-server.service:11-13` — `WorkingDirectory=/opt/scribe/server/meeting-server` + `EnvironmentFile=/etc/scribe/meeting-server.env` + `ExecStart=node index.mjs` → unit staging đang chạy trỏ vào path biến mất ở lần deploy kế; risk note "systemd unit cũ trỏ binary mới" (dòng 69) không có step nào decommission/migrate unit + env file cũ, để ngỏ khả năng 2 unit cùng tranh port 8787.
  Thêm ràng buộc ngầm cho gate "10 test pass nguyên": `test/run-tests.mjs:6,24-25` import `createApp` từ `../index.mjs` và inject `deps` (limiter/hostLock/captions) — index.mjs mới compose router **phải giữ nguyên factory signature `createApp(cfg, deps)` + return shape** chứ không chỉ "giữ route path"; phase file không nói điều này.
- **Failure scenario:** Cook phase 2 xong, test pass, deploy: meeting staging chết vì unit cũ trỏ path rỗng (hoặc chạy code cũ mãi mãi — tệ hơn, silent drift); dev khác chạy `run-local.sh`/`generate-token.mjs` để debug meeting → lỗi module-not-found, mất thời gian truy ngược về restructure.
- **Evidence:** `phase-02-batch-transcribe-core.md:42-44,48,69`; `server/dev-token/generate-token.mjs:6`; `infra/run-local.sh:21,27`; `infra/systemd/meeting-server.service:11-13`; `server/meeting-server/test/run-tests.mjs:6,24-25`.
- **Suggested fix:** Thêm vào phase-02 Modify: `server/dev-token/generate-token.mjs`, `infra/run-local.sh`, và step deploy tường minh "disable meeting-server.service + migrate env file → scribe-server.env". Ghi ràng buộc contract `createApp(cfg, deps)` vào step 1.

## Finding 7: Transcript file .txt/.srt do sidecar ghi ra disk NẰM NGOÀI retention — schema `segments_json` không có nguồn dữ liệu

- **Severity:** High
- **Location:** phase-02 architecture dòng 35 (schema), step 2 (dòng 49), step 5 (dòng 52); phase-05 step 2 (dòng 36)
- **Flaw:** Protocol batch "tái dùng nguyên": `transcribe` **ghi file .txt/.srt ra `out_dir`** (main.py:98-103) và event `done` chỉ mang `text + txt_path + srt_path` — **KHÔNG có segments** (main.py:104). Hệ quả kép: (a) cột `transcripts.segments_json` không có nguồn nào ngoài parse ngược file .srt trên disk — việc plan không nhắc; (b) retention Phase 5 chỉ purge SQLite (`transcripts theo expires_at` + "uploads mồ côi" — phase-05:36), **không đụng file .txt/.srt trong out_dir** → transcript full-text tồn tại trên disk vô hạn sau khi DB đã purge. Acceptance "transcript purge đúng retention" (plan.md:72) fail silent; disk cũng phình ngược với non-functional phase-05.
- **Failure scenario:** Sau 30 ngày, DB purge sạch, user tin transcript đã xóa; file `/opt/scribe/.../job-xyz.txt` chứa nguyên nội dung họp vẫn nằm trên box dùng chung ~15 service. Nếu né bằng `outputs: []` thì hết file rơi vãi nhưng `segments_json` chết hẳn → không render timestamp/không xuất .srt cho user (phase-02 step 5 hứa "copy/download .txt/.srt").
- **Evidence:** `sidecar/transcriber/main.py:98-104`; `phase-02-batch-transcribe-core.md:35,49,52`; `phase-05-sso-retention-hardening.md:36`; `plan.md:72`.
- **Suggested fix:** Chốt data flow trong phase-02: hoặc sidecar thêm segments vào `done` (lại phải sửa sidecar — cộng dồn với Finding 3, sửa 1 lần), hoặc jobs.mjs đọc .srt → parse vào segments_json rồi **unlink cả .txt/.srt cùng lúc với audio**; retention purge phải cover mọi artifact file, có test kiểm disk.

## Finding 8: Active-speaker gating cắt PCM giữa câu → utterance treo mở vô hạn, caption cuối của speaker biến mất

- **Severity:** Medium
- **Location:** phase-04 step 3 (dòng 49) + architecture dòng 30-31; `sidecar/transcriber/stream_session.py`
- **Flaw:** Gating "PORT logic từ **thiết kế**" — đúng là chỉ có thiết kế: desktop code tap MỌI track không gating (`use-live-caption.ts:107-111,193-195`), nên gating Python của bot là code mới 100%, chưa từng chạy. Điểm chết cụ thể: `StreamSession` chỉ chốt utterance khi **nhận được chunk im lặng** đủ hangover (`feed_pcm` — stream_session.py:156-159, `_silence` chỉ tăng khi có chunk đến). Nếu bot de-gate 1 track giữa câu (rớt khỏi top-2) và **ngừng gửi PCM**, session đó giữ `_in_speech=True` + buffer audio mãi mãi — không có timer nội bộ nào flush.
- **Failure scenario:** A đang nói dở câu, B và C chen vào → A rớt top-2, bot ngừng đẩy PCM track A. Câu đang nói của A không bao giờ thành caption (đến tận khi track end/flush — có thể là lúc A rời phòng, caption hiện ra trễ nhiều phút, sai ngữ cảnh). Với họp 8 người chen nhau, mất caption kiểu này xảy ra liên tục và trông như "ASR chập chờn" — rất khó debug nếu không spec trước.
- **Evidence:** `stream_session.py:135-160,173-175`; `src/composables/use-live-caption.ts:107-111` (không có gating để port); `phase-04-meeting-web-caption-bot.md:30-31,49`.
- **Suggested fix:** Spec trong phase-04 step 3: khi de-gate track → bot gửi `stream_end` cho track đó (flush chốt câu) rồi `stream_start` lại khi re-gate; hoặc bơm synthetic-silence chunks đủ hangover trước khi ngừng. Thêm test case gating-switch giữa utterance vào mini-spike.

## Finding 9: "Track id = participant identity" vỡ trên regex sidecar — identity có `.`/`@`/unicode bị từ chối hoặc va chạm sau sanitize

- **Severity:** Medium
- **Location:** phase-04 step 3 (dòng 49); `sidecar/transcriber/stream_server.py:32`
- **Flaw:** Sidecar chặn track id ngoài `^[A-Za-z0-9_-]{1,64}$` (`_TRACK_RE`, stream_server.py:32,113-115). Room id cho phép `.` và `:` (`ROOM_RE`, index.mjs:19) và host identity là chuỗi tự do từ client (`String(b.identity || ...)`, index.mjs:123); Phase 5 SSO tập trung gần như chắc chắn sinh identity dạng email. Desktop đã phải viết `safeTrackId` sanitize + map ngược tên (use-live-caption.ts:25-28,48) — plan phase-04 không nhắc sanitize, cũng không nhắc xử lý va chạm sau sanitize (2 identity khác nhau → cùng 1 id → dính Finding 1: merge track im lặng).
- **Failure scenario:** SSO bật, identity `dat.tqh@example.com` join phòng → bot `stream_start` bị `stream_error "track id không hợp lệ"` → speaker đó vĩnh viễn không có caption; hoặc sanitize ngây thơ biến `dat.tqh` và `dat_tqh` thành cùng id → caption 2 người trộn vào nhau dưới 1 tên.
- **Evidence:** `stream_server.py:32,113-115`; `server/meeting-server/index.mjs:19,123`; `src/composables/use-live-caption.ts:25-28`; `phase-04-meeting-web-caption-bot.md:49`.
- **Suggested fix:** Phase-04 spec: track id = hash/uuid server-sinh per (room, participant), map id→identity→display name giữ ở bot/bridge; cấm dùng identity thô làm id. Cộng hưởng với fix namespace ở Finding 1.

## Finding 10: "caption-feed.ts tái dùng cho nguồn WS" — module coupling cứng với livekit-client, phần tái dùng thật chỉ là 1 hàm push-vào-mảng

- **Severity:** Medium
- **Location:** phase-03 architecture dòng 33 + Modify dòng 42
- **Flaw:** `caption-feed.ts` import cứng `livekit-client` (`RoomEvent`, `Room`, `Participant` — caption-feed.ts:4) và toàn bộ đường ingestion realtime là `bind(room)` + filter `isHostSender`. Trang RecordLivePage (Phase 3) không có Room nào — phần "tái dùng" thực tế = gọi `addLocal()` (1 wrapper quanh `addCaption` ~15 dòng dedupe/cap-200-dòng). Phần việc thật của Phase 3 (WS client, reconnect, ordering, hiển thị draft) đều mới. Import module này vào page không-LiveKit còn kéo `livekit-client` vào bundle của trang ghi âm. Claim này là 1 viên gạch của con số "~70% tái dùng" (plan.md:21) — với live/meeting path, các Finding 2/5/6/10 cho thấy tỷ lệ tái dùng thật thấp hơn đáng kể (tái dùng thật nằm ở batch protocol + token/TURN/store + spike harness).
- **Failure scenario:** Effort phase 3/4 ước theo "tái dùng" → thực tế viết mới WS feed + refactor caption-feed tách core khỏi livekit-client (đụng cả desktop + web đang import) — scope nở giữa phase trong khi meeting đã gộp MVP.
- **Evidence:** `packages/meeting-shared/src/caption-feed.ts:1-8,44-53,60-66,68-70`; `phase-03-live-caption-recording.md:33,42`; `plan.md:21`.
- **Suggested fix:** Ghi thẳng trong phase-03: tách `caption-buffer` core (addCaption/dedupe/cap) framework-agnostic, `caption-feed` LiveKit + `caption-ws` là 2 adapter; hoặc chấp nhận viết feed WS riêng và bỏ claim tái dùng. Điều chỉnh câu "~70% tái dùng" trong plan.md thành per-path (batch cao, live/meeting thấp).

---

## Verification Results (CONTRACT VERIFIER + SCOPE AUDITOR)

### A. Kiểm claim đáng ngờ của plan

| Claim | Kết quả | Evidence |
|---|---|---|
| Protocol stdio stream = `stream_start/stream_pcm/stream_stop` | **SAI TÊN** — lệnh thật là `stream_end`; sự cẩu thả cùng loại với `transcribe_msg` (không tồn tại; event thật: ready/download/model_ready/progress/done/error/info) | `stream_server.py:12-15`; `main.py:5-8` |
| "sidecar KHÔNG cần sửa" | **SAI** — không set được 2 threads (Finding 3), không lấy được segments (Finding 7), cần namespace/flush semantics (Finding 1/8) | main.py:67,142; stream_server.py:56 |
| stream_server/stream_session chứa "cap logic" (phase-03:43) | **SAI** — chỉ có `_QUEUE_MAX` backpressure per-track; cap ≤2 là ghi chú docstring "ở tầng trên", không có code | `stream_server.py:31`; `stream_session.py:10-11` |
| "10 test meeting pass nguyên" | **ĐÚNG số lượng** — đúng 10 `test(...)`; kèm ràng buộc ngầm factory contract (Finding 6) | `test/run-tests.mjs` (10 matches) |
| "port code G6 AudioWorklet (decimation + low-pass)" | **SAI** — G6 chưa QA runtime; không có decimation/low-pass nào tồn tại | Finding 5 |
| "PyAV decode webm/opus trực tiếp" | **ĐỨNG VỮNG (có caveat)** — faster-whisper 1.2.1 decode qua PyAV, `av==17.1.0` pinned; main.py nhận path trực tiếp. Caveat: pin verify trên Python 3.14 dev; Python box chưa kiểm (plan thừa nhận) — wheel av/ctranslate2 cho python cũ cần confirm ở Phase 1 step 1 | `requirements.txt:3-5`; `main.py:78` |
| "node:sqlite (Node ≥22.5) hoặc better-sqlite3" | **CÓ ĐIỀU KIỆN** — better-sqlite3 = native addon: cần prebuilt khớp Node ABI + glibc box, hoặc toolchain compile trên box (chưa kiểm). Plan mới chỉ kiểm Node version (phase-02:70), chưa kiểm glibc/toolchain | `phase-02:35,70` |
| "livekit-rtc chạy trên box" | **CHƯA CHỨNG MINH — plan TỰ THỪA NHẬN và có gate cứng** (mini-spike step 1 + phương án B). Không phải finding; ghi nhận gate là đúng cách | `phase-04:47,67` |
| "MediaRecorder song song + AudioWorklet cùng lúc" | **KHẢ THI trên Chrome** (nhiều consumer trên 1 MediaStream được hỗ trợ) — chưa từng chạy trong repo này; rơi vào cùng mini-spike đề xuất ở Finding 5. Caveat: user quên tick "share audio" ở getDisplayMedia → cả 2 đường im lặng, cần detect | `phase-03:24-27` |
| Ước tính RAM/RTF | **RAM: MÂU THUẪN với spike đã đo** (Finding 4). RTF: ngoại suy từ dev 16-core@4threads sang Broadwell@"2 threads" — gate Phase 1 là chỗ đúng để đo, NHƯNG đo phải bằng invocation production (Finding 3), nếu không gate vô nghĩa | spike report §9; plan.md:57-64 |
| "MeetingRoomPage hiện là viewer-oriented; thêm publish camera/mic" (phase-04:42) | **SAI THEO HƯỚNG NGƯỢC** — `useLiveKitViewer` ĐÃ publish cam+mic (`enableCameraAndMicrophone`, toggleMic/toggleCamera). Việc "thêm publish path" phần lớn đã tồn tại — scope phase 4 chỗ này nhỏ hơn plan tả (hiếm hoi: sai lệch có lợi) | `use-livekit-viewer.ts:89-91,104-116` |

### B. ENUMERATE consumer của `server/meeting-server` (cho restructure Phase 2)

Grep toàn repo (loại node_modules, plans): **11 file tham chiếu**, phân loại:

| # | Consumer | Kiểu phụ thuộc | Sau `git mv` |
|---|---|---|---|
| 1 | `server/meeting-server/test/run-tests.mjs:6-8` | import tương đối `../index.mjs`, `../lib/*` | Sống (move cùng) — NHƯNG đòi giữ factory contract `createApp(cfg,deps)` |
| 2 | `server/dev-token/generate-token.mjs:6` | import cross-dir `../meeting-server/lib/jwt.mjs` | **VỠ** — không có trong Modify list |
| 3 | `infra/run-local.sh:27` | path `server/meeting-server/index.mjs` | **VỠ** — không có trong Modify list |
| 4 | `infra/systemd/meeting-server.service:11-13` | WorkingDirectory + ExecStart + EnvironmentFile `/etc/scribe/meeting-server.env` | **VỠ trên staging** — chỉ có unit MỚI trong Create, không có step decommission |
| 5 | `apps/web/src/MeetingRoomPage.vue:13` + `vite-env.d.ts:4` | env `VITE_MEETING_SERVER` (URL runtime) | Sống nếu route path giữ nguyên |
| 6 | `packages/meeting-shared/src/meeting-client.ts` | fetch route `/token/guest`, `/captions/:room` | Sống nếu route path giữ nguyên |
| 7 | `src/views/MeetingRoomView.vue:28` (desktop maintenance) | URL runtime nhập tay | Sống nếu route path giữ nguyên |
| 8 | `docs/deployment-guide.md`, `docs/system-architecture.md` | mô tả path + unit + runbook | **STALE từ Phase 2 → Phase 5** (docs dời hết về phase-05:41) |
| 9 | `infra/coturn/turnserver.conf`, 2 package.json | tên/mô tả, không phụ thuộc path code | Sống |

Kết luận: 3 hard break + 1 khoảng stale docs 3-phase + 1 ràng buộc contract test — "giữ nguyên route path + env contract" (phase-02:69) là cần nhưng KHÔNG đủ.

---

## Tổng kết

Plan có kỷ luật gate tốt (Phase 1 GO/NO-GO, mini-spike phase 4) nhưng dựng trên một cụm claim "đã có / đã chứng minh / không cần sửa" mà codebase bác bỏ khi grep: sidecar bắt buộc phải sửa (threads, segments, namespace), đường caption realtime của Phase 4 hiện không có cơ chế hợp lệ nào, G6 chưa từng chạy runtime và code mitigation được viện dẫn không tồn tại, ngân sách RAM thấp hơn chính số spike mà plan trích dẫn. Con số "~70% tái dùng" đúng cho batch protocol + token/store/TURN, nhưng cho live/meeting path thì phần lõi (WS bridge, gating, caption delivery, downsample) đều là code mới.

**Khuyến nghị ưu tiên cho planner:** (1) sửa Finding 2 trước khi cook phase 4 — đây là lỗ kiến trúc, không phải chi tiết; (2) gộp mọi thay đổi sidecar (threads + segments + namespace/flush) thành 1 mục "sidecar contract v2" trong phase-02/03 thay vì rải rác "không sửa"; (3) Phase 1 đo bằng invocation production + đo RSS đồng thời để rebudget RAM/MemoryMax; (4) bổ sung 3 consumer vỡ vào phase-02.

Status: DONE
Summary: Đã adversarial-review 6 file plan đối chiếu 15+ file code/report; 10 finding có evidence file:line, trong đó 2 Critical (rò caption cross-user qua namespace track phẳng; cơ chế phát caption Phase 4 không tồn tại) và 5 High tập trung vào cụm claim "không cần sửa sidecar / đã chứng minh / ngân sách RAM".
Findings: Critical 2 · High 5 · Medium 3
