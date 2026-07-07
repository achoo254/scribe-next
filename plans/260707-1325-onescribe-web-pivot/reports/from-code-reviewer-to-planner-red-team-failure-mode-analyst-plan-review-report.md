# Red-Team Plan Review — OneScribe Web Pivot (Failure Mode Analyst)

**Reviewer:** code-reviewer (adversarial, Murphy's Law lens) · **Date:** 2026-07-07
**Target:** `plans/260707-1325-onescribe-web-pivot/` (plan.md + phase-01..05)
**Posture:** tấn công cách thực thi, KHÔNG tấn công quyết định user đã chốt (web thay desktop, small/medium cố định, audio ephemeral, meeting gộp MVP, guardrails systemd).
**Mọi finding có evidence file:line từ codebase.**

---

## Finding 1: Gate Phase 1 đo bằng cờ `--cpu-threads 2` KHÔNG TỒN TẠI trong main.py — GO/NO-GO dựa trên số liệu sai cấu hình; "sidecar KHÔNG sửa" của Phase 2/3 là bất khả thi

- **Severity:** Critical
- **Location:** phase-01 step 5 (dòng 37) "gửi lệnh transcribe qua main.py với `--cpu-threads 2`"; phase-02 Architecture (dòng 31-32) "(medium, --cpu-threads 2) … tái dùng nguyên, KHÔNG sửa"; phase-03 step 2 (dòng 48) "spawn `main.py --stream` (small, threads 2) … sidecar KHÔNG cần sửa" (dòng 32).
- **Flaw:** `main.py` không parse bất kỳ CLI arg nào ngoài check `"--stream" in sys.argv` (main.py:139-147). Batch model load hardcode `cpu_threads=os.cpu_count() or 8` (main.py:67) = **4 threads trên box .65**. Stream path: `_ensure_model` gọi `load_stream_model(model_path)` KHÔNG truyền cpu_threads (stream_server.py:57) → default `os.cpu_count()` (stream_session.py:37). Cờ `--cpu-threads` chỉ tồn tại trong spike harness (stream_asr_spike.py:48) — không phải production path. Truyền `--cpu-threads 2` vào main.py bị **nuốt im lặng, không lỗi**.
- **Failure scenario:** (a) Phase 1 đo batch "@2 threads" nhưng thực chạy 4 threads → RTF đẹp gấp ~2x thực tế → GO sai; sau đó production chạy dưới CPUQuota=200% (=2 core) → file 60' vượt 3h, acceptance MVP fail sau khi đã commit toàn plan. (b) Hoặc: đội thực thi phát hiện thiếu cờ giữa Phase 2, buộc sửa sidecar — phá claim "KHÔNG sửa", regression surface lan sang desktop maintenance (main.py dùng chung binary desktop). (c) Trong lúc đo Phase 1, batch 4-thread không nice-được-hết chiếm cả 4 vCPU → chính bước benchmark làm degrade 15 service — vi phạm non-functional requirement của chính phase đó (phase-01 dòng 19).
- **Evidence:** main.py:66-68, main.py:139-147, stream_server.py:57, stream_session.py:29-38, stream_asr_spike.py:48.
- **Suggested fix:** Thêm bước rõ trong Phase 1: sidecar CẦN sửa tối thiểu (nhận `cpu_threads` qua protocol field hoặc env `ONESCRIBE_CPU_THREADS`, plumb vào cả `get_model` lẫn `_ensure_model`) TRƯỚC khi benchmark; hoặc benchmark qua `taskset -c 0,1` + ghi rõ threads thật vào report. Sửa claim "KHÔNG sửa sidecar" thành "sửa 1 điểm: thread control" trong phase-02/03.

## Finding 2: Không có cơ chế trọng tài CPU giữa batch medium và live stream trong cùng CPUQuota=200% — thiết kế two-pass TỰ TẠO collision; MemoryMax=2500M chỉ dư ~11% so peak thiết kế

- **Severity:** Critical
- **Location:** plan.md Ngân sách (dòng 57-64); phase-02 Related Code Files (dòng 42, MemoryMax=2500M); phase-03 Overview "dừng → medium final tự enqueue" (dòng 14, 51) + Risk (dòng 68, guard 1 chiều); phase-01 Success Criteria (dòng 44-46, chỉ đo batch RIÊNG và stream RIÊNG).
- **Flaw:** Guard duy nhất là 1 chiều: "registry từ chối mở phiên live MỚI khi batch đang chạy nếu RAM <500MB" (phase-03:68). KHÔNG có chiều ngược: batch job start bất cứ lúc nào queue tới lượt, kể cả khi 2 phiên live đang chạy. Mà flow two-pass BẢO ĐẢM kịch bản này: user A đang live-caption, user B bấm dừng → medium final job tự enqueue → load 1.5GB + ngốn CPU trong cùng cgroup 200%. Nhu cầu CPU: 2 stream track + batch = ≥3 luồng transcribe nặng chia nhau 2 core quota → stream RTF >1 → queue sidecar 64 chunk (~16-32s audio, stream_server.py:31) đầy → drop chunk MỚI NHẤT (stream_server.py:94-99) → caption mất đoạn giữa câu của user A. Phase 1 gate không hề đo kịch bản kết hợp này — số GO không bao phủ workload thật.
- **Failure scenario (RAM):** Peak thiết kế 150MB (node) + 1.5GB (medium) + 600MB (small) = 2.25GB, chưa tính bot Python, PyAV decode buffer, SQLite WAL, nginx upload tmp. MemoryMax=2500M → headroom <250MB. Vượt → kernel OOM-killer bắn process TRONG cgroup; victim có thể là **node** (mất caption store in-memory, host-lock, mọi WS session, queue state) chứ không chỉ python. Không set `MemorySwapMax` → trước khi OOM, page spill sang swap (đã dùng 876MB) → ASR chậm như băng thay vì fail nhanh — chế độ chết khó chẩn đoán nhất.
- **Evidence:** stream_server.py:31, stream_server.py:94-99; phase-03:68 (guard 1 chiều); phase-02:42; plan.md:59-64; phase-01:44-46 (không có combined benchmark).
- **Suggested fix:** (1) Thêm rule trọng tài 2 chiều: batch job chỉ start khi stream active = 0, HOẶC batch tự pause/delay khi có stream (queue đã async — user chấp nhận chờ). (2) Phase 1 thêm scenario bắt buộc: batch medium + 2 stream track ĐỒNG THỜI dưới `systemd-run --scope -p CPUQuota=200%` — đúng ràng buộc production. (3) MemoryMax cho batch RIÊNG (unit/slice con hoặc `systemd-run` per-job) tách khỏi node; set MemorySwapMax=0 cho fail-fast.

## Finding 3: Queue SQLite không có boot-time reconciliation — server restart/OOM giữa job → job kẹt "running" vĩnh viễn, queue concurrency=1 có thể wedge toàn hệ

- **Severity:** High
- **Location:** phase-02 step 2 (dòng 49, thiết kế runner) + Success Criteria (dòng 63) — chỉ test "kill -9 python giữa job", tức kịch bản NODE SỐNG SÓT.
- **Flaw:** Không bước nào trong phase-02 steps 1-8 xử lý chiều ngược: node chết giữa job (deploy restart lặp lại suốt Phase 2→5, OOM-kill theo Finding 2, `Restart=on-failure` tự kéo dậy — meeting-server.service:14). SQLite bền → row `status=running` tồn tại mãi; python con bị cgroup teardown giết. Boot mới không có bước "requeue/fail mọi job running mồ côi". Nếu queue-head đọc từ DB đếm running → **wedge vĩnh viễn mọi job sau**; nếu queue in-memory → job mồ côi treo "running" mãi, user không bao giờ nhận transcript, audio file nằm lại disk (lời hứa ephemeral vỡ tới khi TTL 24h quét).
- **Bonus (verified flow):** claim "per-job spawn = tự nhả RAM / model process thoát" (phase-02:49) — main.py KHÔNG tự thoát sau `done`: loop `for line in sys.stdin` (main.py:114) tiếp tục chờ, model 1.5GB giữ ấm (main.py:23,63) tới khi Node đóng stdin hoặc gửi `shutdown`. Runner quên đóng stdin = leak 1.5GB/job "vô hình" mà test fake-runner không bắt được.
- **Failure scenario:** Đêm chạy job 60' (3h @RTF3) → 1h sáng OOM/deploy → systemd restart node trong 3s → job "running" mồ côi → sáng ra mọi job mới "queued" mãi; on-call không có runbook (Phase 5 mới viết) → khởi động lại mù → lặp lại.
- **Evidence:** phase-02:49,63; main.py:114-136, main.py:23,63-72; infra/systemd/meeting-server.service:14.
- **Suggested fix:** Thêm vào phase-02 step 2: (a) boot reconciliation — `UPDATE jobs SET status='failed', error='server restart' WHERE status='running'` (hoặc requeue có đếm attempt); (b) runner contract rõ: gửi `shutdown` + đóng stdin + `SIGKILL` timeout sau `done`; (c) success criterion mới: "restart service giữa job → job đánh dấu failed, queue tiếp tục".

## Finding 4: Restructure `git mv meeting-server → onescribe-server` trên staging đang chạy: 2 systemd unit giành 1 port, không có thứ tự cutover, không có rollback procedure cho tới Phase 5

- **Severity:** High
- **Location:** phase-02 step 1 (dòng 48) + Related Code Files (dòng 42, tạo `onescribe-server.service` MỚI) + Risk (dòng 69, "systemd unit cũ trỏ binary mới") + step 7 deploy (dòng 54, không có backup/rollback); phase-05 step 7 (dòng 41) — runbook rollback viết SAU CÙNG.
- **Flaw:** Unit đang chạy trên box: `WorkingDirectory=/opt/onescribe/server/meeting-server`, `ExecStart=/usr/bin/node index.mjs` (meeting-server.service:11-13). Kế hoạch vừa nói "unit cũ trỏ binary mới" (risk, dòng 69) vừa tạo unit MỚI `onescribe-server.service` (dòng 42) — mâu thuẫn, không bước nào disable/remove unit cũ. rsync repo restructured: nếu `--delete` → thư mục cũ biến mất khi unit cũ còn enable → crash-loop restart; nếu không `--delete` → 2 bản code cùng tồn tại, 2 unit cùng enable → EADDRINUSE hoặc tệ hơn: **split-brain** — LiveKit webhook (participant_left nhả host-lock, index.mjs:160-177) bắn vào process CŨ trong khi token/caption cấp từ process MỚI → host-lock không bao giờ nhả → 409 "phòng đã có host khác" vĩnh viễn tới restart.
- **Failure scenario phụ:** caption store + host-lock đều in-memory (index.mjs:85-87; journal-260701:19-20 — đánh đổi có chủ đích). Plan deploy 4 lần (Phase 2,3,4,5) lên box đang phục vụ meeting staging, không có drain/maintenance-window step → mỗi deploy giữa meeting = mất caption history + đứt WS, không thông báo.
- **Evidence:** infra/systemd/meeting-server.service:11-14; phase-02:42,48,54,69; index.mjs:85-87,160-177; journal-260701-phase345-cook.md:19-20.
- **Suggested fix:** Thêm sub-step cutover tường minh vào phase-02 step 7: (1) backup `/opt/onescribe` + dump env; (2) `systemctl disable --now meeting-server` VÀ cài unit mới trong cùng maintenance window (ghi rõ giờ, tránh meeting active); (3) giữ symlink `server/meeting-server → onescribe-server` 1 release để rollback = re-enable unit cũ; (4) rollback procedure viết NGAY ở Phase 2, không đợi Phase 5.

## Finding 5: Đường phân phối caption tới client meeting (Phase 4) không được định nghĩa — và tự mâu thuẫn với chính mô hình chống giả mạo C7 của nó

- **Severity:** High
- **Location:** phase-04 step 2 (dòng 48, bot `canPublishData=false`) vs step 5 (dòng 51, "caption store → client render — client CHỈ render caption nguồn bot (signed role trong metadata)").
- **Flaw:** Đường realtime duy nhất hiện có tới client meeting là **LiveKit data channel do host publish** (`canPublishData` — jwt.mjs:22; caption-feed.ts:1-10 nhận qua `RoomEvent`, lọc theo `participant.metadata.role`). Phase 4 tước quyền publishData của bot (step 2) rồi lại nói client render caption "nguồn bot, signed role trong metadata" (step 5) — **caption lấy từ store REST không có participant metadata nào để check**; cơ chế như mô tả là không thể implement. Caption store hiện chỉ là poll-GET seed lịch sử (index.mjs:143-145), không có push channel. Vậy 1 trong 3 đường phải chọn, plan không chọn: (a) 8 participant poll GET /captions/:room liên tục — latency sàn = poll interval, VÀ đâm vào rate-limiter (capacity 20, refill 1/s per IP — index.mjs:85); với `TRUST_PROXY` mặc định false sau nginx, MỌI client cùng bucket `req.socket.remoteAddress` = IP nginx (index.mjs:63-71) → 429 storm ngay khi 8 người poll — plan không có bước set TRUST_PROXY=true khi deploy; (b) cấp lại canPublishData cho bot — mâu thuẫn step 2, mở lại surface C7; (c) server-side LiveKit data API — không được nhắc tới đâu cả.
- **Failure scenario:** E2E step 7 (3 browser) pass vì dev vá tạm bằng poll 1s; lên 8 người + TRUST_PROXY quên set → caption 429/chập chờn; hoặc dev "tiện tay" bật canPublishData cho bot → guest inject test (success criteria dòng 61) giờ phải phân biệt bot thật/giả bằng metadata mà guest cũng có thể có phòng khác cấp — mô hình 2 lớp C7 (journal:29-31) âm thầm còn 1 lớp.
- **Evidence:** index.mjs:63-71,85,143-157; jwt.mjs:19-23; caption-feed.ts:1-23; phase-04:48,51,61.
- **Suggested fix:** Chốt cơ chế fan-out TRONG plan trước khi cook: đề xuất — server (đã là nguồn tin cậy, giữ apiSecret) publish caption vào room qua LiveKit server API (`SendData`), client giữ nguyên filter; HOẶC WS caption riêng cho meeting page (tái dùng ws.mjs Phase 3). Ghi rõ thay đổi ngữ nghĩa C8 (POST store từ internal, không còn role=host). Thêm bước deploy: `TRUST_PROXY=true` + nginx `X-Forwarded-For`.

## Finding 6: Vòng đời bot phụ thuộc webhook LiveKit chưa hề được cấu hình ở đâu trong repo; mất `room_finished` = zombie bot chiếm slot ASR toàn cục vĩnh viễn; phòng #2 không bao giờ được re-arm caption

- **Severity:** High
- **Location:** phase-04 Architecture (dòng 24, "webhook room_started/room_finished → spawn/kill per room") + step 4 (dòng 50) + Success Criteria (dòng 62, chỉ test happy-path).
- **Flaw:** (1) `infra/livekit/livekit.yaml` KHÔNG có section `webhook:` nào (toàn file 17 dòng, chỉ port/rtc/keys) — bật webhook đòi sửa config LiveKit ĐANG CHẠY trên .65 + restart `livekit.service` (rớt mọi meeting active) — bước này vắng mặt trong Implementation Steps 1-8. Handler `/webhook` hiện chỉ dùng participant_left (index.mjs:169). (2) Webhook LiveKit là at-least-once, không thứ tự: `room_finished` thất lạc (server đang restart lúc delivery, retry cạn) → bot process sống mãi, giữ 1-2 slot của cap ≤2 toàn cục → **mọi live recording bị "đang bận" vô hạn** tới khi SSH giết tay. "idle → kill" được nhắc 1 lần không có định nghĩa idle/heartbeat. (3) Logic "chỉ spawn khi chưa có phòng caption active" (step 4): room #2 started khi room #1 active → bỏ qua; room #1 finished → KHÔNG có re-arm — room #2 vĩnh viễn không caption, và vì trigger là webhook (không phải request của user) nên "có thông báo" (success criteria dòng 60) không có đường trả về client nào được mô tả. (4) Room recreate cùng tên: `room_finished` của phiên cũ đến SAU `room_started` phiên mới → kill nhầm bot mới.
- **Bonus:** livekit.yaml:15-17 là `devkey: secret` — cặp dev công khai, file tự ghi "BẮT BUỘC đổi trước staging"; phase-05 hardening sweep (dòng 38) không có mục verify/rotate LiveKit keys trên box — nếu staging còn chạy devkey, mọi token host/bot/webhook signature đều forge được.
- **Evidence:** infra/livekit/livekit.yaml:1-17 (không webhook section, devkey); index.mjs:160-177; phase-04:24,50,60,62.
- **Suggested fix:** Thêm vào phase-04: bước cấu hình webhook LiveKit (URL mới + restart có kế hoạch, trong maintenance window Finding 4); bot-manager phải (a) idempotent theo room+session id, (b) reconcile định kỳ bằng LiveKit RoomService `ListRooms` (poll 60s) thay vì tin webhook — webhook chỉ là trigger nhanh, poll là nguồn chân lý; (c) TTL cứng cho bot process (max meeting length); (d) re-arm caption khi slot nhả. Phase 5 thêm mục rotate LiveKit keys.

## Finding 7: Đơn vị đếm của stream-registry (WS session) lệch với đơn vị tải thật (ASR track) — "bot = 1 client WS" có thể mang 2 track, cap ≤2 do user chốt bị đếm hụt thành 3-4 luồng transcribe trên 1 model lock

- **Severity:** High
- **Location:** phase-03 bullet registry (dòng 36, "đếm stream active toàn cục, cap=2"); phase-04 (dòng 19 "bot dùng chung cap ≤2"; dòng 49 "đẩy PCM từng track qua WS với track id = participant identity"); plan.md dòng 48 ("bot = 1 client WS nội bộ").
- **Flaw:** Giao thức sidecar là per-TRACK: 1 client stdio/WS multiplex N track (`stream_pcm` mang track id — stream_server.py:12-15,130-137), mỗi track 1 worker thread, transcribe serialize qua `_model_lock` chung (stream_server.py:35; stream_session.py:44-52). Bot gating ≤2 track ĐỒNG THỜI đẩy 2 track qua 1 WS. Nếu registry đếm SESSION: bot=1 → còn 1 slot cho live recording → thực tế 3 track tranh nhau 1 model lock @2-4 threads trong quota 200% → mỗi track throughput < realtime → queue đầy → drop (Finding 2 khuếch đại). Nếu registry đếm TRACK: 1 meeting 2 người nói = hết sạch cap toàn cục, mọi live recording bị từ chối khi có meeting caption — hệ quả UX lớn mà plan không nói rõ đã chọn nhánh nào. Phase 1 benchmark tối đa 2 track (phase-01:38) — số GO chỉ valid cho đúng 2, không phải 3-4.
- **Failure scenario:** Meeting 2 người nói chồng + 1 nhân viên khác đang ghi mic live → 3 track serialize trên 1 lock → caption cả hai nơi trễ >10s, backpressure drop giữa câu → "caption đúng người nói" (acceptance) fail theo cách gián đoạn khó tái hiện.
- **Evidence:** stream_server.py:12-15,35,130-137; stream_session.py:41-52; phase-03:36; phase-04:19,49; phase-01:38.
- **Suggested fix:** Định nghĩa trong phase-03: **đơn vị cap = TRACK ASR đang mở ở sidecar**, registry đếm track (bot acquire 2, live acquire 1); ghi rõ hệ quả "meeting caption chiếm trọn cap" vào UX copy + acceptance. Phase 1 thêm số đo 3 track để biết cliff ở đâu.

## Finding 8: Respawn/idle-kill persistent stream process — claim "pattern H3/L1 đã có" chỉ đúng cho Rust desktop 1-client; bản Node multi-client là code mới với race EPIPE có thể sập cả server

- **Severity:** Medium
- **Location:** phase-03 bullet lifecycle (dòng 37, "kill sau idle N phút") + step 2 (dòng 48, "Sidecar crash → respawn + báo client (bài học H3/L1 đã có pattern)").
- **Flaw (traced):** Pattern L1 hiện hữu nằm ở `src-tauri/src/meeting_audio.rs:89,171` — Rust, 1 client duy nhất, lazy respawn ở lần dùng kế, KHÔNG có idle timer. Phase 3 là bài toán khác: N WS session multiplex 1 process + idle-kill chủ động. Race không được đề cập: (a) **idle-timer TOCTOU** — timer quyết định kill; đồng thời session mới acquire registry và ghi `stream_start` vào stdin của process đang chết → Node stream `error` (EPIPE); child stdin không gắn error-handler → **uncaught exception sập toàn bộ node** (kéo theo meeting endpoints, không chỉ live). (b) crash giữa chừng: session nào bị lỗi, ai release slot registry — không đặc tả → slot leak = "đang bận" vĩnh viễn (giống Finding 6 nhưng đường live). (c) respawn storm dưới áp lực MemoryMax: load model ~600MB lặp lại × Restart=on-failure → OOM loop.
- **Evidence:** src-tauri/src/meeting_audio.rs:89,107,171; phase-03:37,48; stream_server.py:61-99 (worker/queue per track — chết cùng process).
- **Suggested fix:** Đặc tả trong phase-03: generation/epoch id cho stream process (mọi write đi qua guard "gen hiện tại còn sống"); error-handler bắt buộc trên child stdin/stdout; crash → fail tất cả session gắn gen đó + release registry TRONG cùng bước; respawn có backoff + đếm, quá ngưỡng → từ chối phiên mới với thông báo thay vì loop.

## Finding 9: Điểm mù giao thức đã verify — `stream_stop` không tồn tại (đúng tên là `stream_end`), `transcribe_msg` không phải event của sidecar; gửi lệnh sai tên bị nuốt im lặng → track leak

- **Severity:** Medium
- **Location:** phase-03 Architecture (dòng 31, "stream_start/stream_pcm(base64)/**stream_stop**"); phase-02 Architecture (dòng 31-32, "transcribe cmd, **transcribe_msg** events").
- **Flaw:** Protocol thật: `stream_start` / `stream_pcm` / **`stream_end`** / `shutdown` (stream_server.py:12-15). Dispatch không có nhánh default báo lỗi — lệnh `stream_stop` rơi xuyên qua if-chain, **im lặng không làm gì** (stream_server.py:174-181) → utterance cuối không flush, worker thread + buffer track sống mãi tới khi kill process. `transcribe_msg` là tên KÊNH EVENT Tauri phía desktop (journal-260701:17-18), không phải sidecar; stdout thật của batch: `ready/progress/done/error/info` (main.py:7-8). Ai cook từ plan này mà không đọc lại source sẽ code bridge theo tên sai — và vì sai tên FAIL SILENT (không error), test fake-process tự viết theo cùng tên sai vẫn xanh → phantom test đúng nghĩa.
- **Failure scenario:** Bridge gửi `stream_stop` khi user dừng ghi → sidecar bỏ qua → caption cuối mất + track không bao giờ đóng → registry đếm active mãi (nếu đếm theo sidecar) hoặc RAM worker leak dần qua nhiều phiên → chết chậm sau vài ngày staging.
- **Evidence:** stream_server.py:12-15,140-145,174-181; main.py:3-8; journal-260701-phase345-cook.md:17-18.
- **Suggested fix:** Sửa tên lệnh/event trong phase-02/03 theo source (`stream_end`; batch events `progress/done/error`). Yêu cầu integration test Phase 3 chạy với sidecar THẬT (fake model, protocol thật — pattern tests/test_stream_server.py đã có) chứ không chỉ fake echo process.

## Finding 10: Trust boundary còn hở theo thiết kế — JWT trên WS query string lọt access log nginx; multipart parser tự viết tay cho file 500MB trong server zero-dep; không có ownership check /api/jobs/:id (IDOR nổ chậm khi SSO Phase 5 vào)

- **Severity:** Medium
- **Location:** phase-03 step 1 (dòng 47, "JWT verify trên query/header"); phase-02 step 4 (dòng 51, uploads multipart streaming) + step 3 (dòng 50, middleware chỉ verify JWT); phase-05 (không có criterion cách ly user).
- **Flaw:** (a) Token trong query WS → nginx ghi full URI vào access log trên box dùng chung 15 service — JWT tương đương bearer nằm plaintext trong `/var/log/nginx`, retention/log-shipping ngoài kiểm soát plan. (b) Repo không có multipart parser nào (meeting-server là raw http, readBody cap 1MB — index.mjs:36-50); plan hứa "multipart streaming về disk KHÔNG buffer RAM" cho file 500MB nhưng không chỉ định thư viện — tự tay viết boundary parser streaming là nguồn bug kinh điển (boundary cắt giữa chunk, disk-fill, request treo), trong khi plan ĐÃ cấp ngoại lệ zero-dep cho `ws` (phase-03:29) — chính sách dependency ngược đời: component rủi ro hơn lại không được ngoại lệ. (c) Schema có cột `user` (phase-02:35) nhưng không bước/criterion nào yêu cầu "user A không đọc được job/transcript của user B"; với 1 ACCESS_PASSWORD chung thì vô nghĩa hôm nay, nhưng Phase 5 cắm SSO OneDash (phase-05:23) biến toàn bộ transcript (nội dung họp = nhạy cảm) thành đọc-chéo được nếu handler không filter theo user — invariant auth/ownership thuộc loại "extract NGAY" theo chính rule chất lượng dự án, không phải đợi security-scan Phase 5 vớt.
- **Evidence:** phase-03:47,29; index.mjs:36-50; phase-02:35,50,51; phase-05:23,38-39.
- **Suggested fix:** (a) WS auth qua header `Sec-WebSocket-Protocol` hoặc ticket 1 lần TTL 30s đổi từ JWT qua POST trước khi upgrade. (b) Chỉ định `busboy` (ngoại lệ zero-dep thứ 2, có chủ đích) hoặc đổi contract thành PUT raw body + metadata header — bỏ multipart hẳn. (c) Thêm vào phase-02 step 2/3: mọi query jobs/transcripts có predicate `WHERE user = ?` + 1 test cross-user 403/404; ghi thành success criterion.

---

## Verification Results (FLOW TRACER)

| Claim trong plan | Kết quả | Flow thật |
|---|---|---|
| "protocol stdio batch ĐÃ CÓ (transcribe cmd, transcribe_msg events)" (phase-02:31-32) | **PARTIAL FAIL** | Entry `main()` main.py:107 → `for line in sys.stdin` :114 → dispatch `cmd=="transcribe"` :127 → `transcribe()` :75 emit `progress`(:92)/`done`(:104)/`error`(:136). Cmd `transcribe` CÓ; event tên `transcribe_msg` KHÔNG tồn tại trong sidecar — đó là kênh Tauri desktop (journal:17-18). `--cpu-threads` không được parse (:139-147) — bị nuốt im lặng. |
| "protocol stream stream_start/stream_pcm/stream_stop ĐÃ CÓ, sidecar KHÔNG cần sửa" (phase-03:31-32) | **PARTIAL FAIL** | `run()` stream_server.py:155 → dispatch :174-181: `stream_start`(:113 — guard track regex :114, size whitelist :118)/`stream_pcm`(:130 — early-return nếu track chưa start :133)/`stream_end`(:140). `stream_stop` không match nhánh nào → **silent no-op**. Threads không chỉnh được từ ngoài: `_ensure_model` :48-57 gọi `load_stream_model(model_path)` không truyền cpu_threads → os.cpu_count() (stream_session.py:37). "KHÔNG cần sửa" = FALSE nếu muốn 2 threads. |
| "10 test pass sau restructure" (phase-02:59) | **VERIFIED (count) / ASPIRATIONAL (pass)** | Đúng 10 `test(` trong test/run-tests.mjs (grep, dòng 60-192); import `createApp` từ `../index.mjs` (run-tests.mjs:6) → sau khi tách `meeting.mjs` + compose router, `createApp` phải giữ nguyên signature/behavior hoặc test phải sửa import — "pass NGUYÊN" chỉ đúng nếu handler split bảo toàn cả CORS/404/catch-JSON ở index.mjs:89-183. |
| "bot = 1 client WS nội bộ" (plan.md:48, phase-04:37) | **TRUE transport / FALSE capacity** | Sidecar multiplex per-track qua 1 kênh (stream_server.py:130-137); 1 WS mang N track hợp lệ. Nhưng tải ASR đếm theo TRACK (mỗi track 1 worker :67, serialize `_model_lock` :35) → "1 client" ≠ "1 stream" — xem Finding 7. |
| "spawn per-job tự nhả RAM" (phase-02:49 "model process thoát") | **PARTIAL FAIL** | `main()` KHÔNG exit sau `done` — loop stdin :114 tiếp tục, `_model` global giữ 1.5GB (:23,63). Thoát chỉ khi stdin EOF hoặc cmd `shutdown` (:132-133). "Tự nhả" phụ thuộc runner đóng stdin/kill — contract này phải ghi tường minh (Finding 3). |
| Tái phạm bài học cũ (C2/H3/H4)? | **C2: không tái phạm protocol-level** (batch/stream vẫn 2 process) nhưng tái xuất ở tầng CPU-quota (Finding 2). **H3/L1: pattern chỉ có ở Rust 1-client** (meeting_audio.rs:171) — Node multi-client là mới (Finding 8). **H4 demux: không tái phạm** (2 process → 2 kênh tách tự nhiên). |

## Tổng hợp

| # | Finding | Severity |
|---|---|---|
| 1 | Gate benchmark dùng cờ không tồn tại; "không sửa sidecar" bất khả thi | Critical |
| 2 | Không trọng tài CPU batch↔live trong 1 quota; MemoryMax sát peak, OOM giết node | Critical |
| 3 | Queue không có boot reconciliation; per-job process không tự thoát | High |
| 4 | Restructure 2-unit/1-port, không rollback tới Phase 5, split-brain webhook/host-lock | High |
| 5 | Đường fan-out caption meeting không định nghĩa, tự mâu thuẫn C7; TRUST_PROXY bẫy 429 | High |
| 6 | Webhook chưa cấu hình đâu cả; mất room_finished = zombie chiếm slot; không re-arm; devkey | High |
| 7 | Cap ≤2 đếm session vs track lệch nhau — guardrail user chốt bị đếm hụt | High |
| 8 | Respawn/idle-kill race EPIPE sập cả node; "pattern đã có" là Rust 1-client | Medium |
| 9 | Tên lệnh/event sai trong plan, sidecar fail-silent → phantom test | Medium |
| 10 | JWT query-string vào log; multipart tay 500MB; IDOR nổ chậm khi SSO | Medium |

**Kết luận:** Xương sống plan (gate → batch → live → bot) đúng hướng và tôn trọng các quyết định đã chốt, nhưng **gate Phase 1 như đang viết sẽ đo sai cấu hình** (Finding 1) và **kịch bản tải kết hợp — thứ two-pass design tự sinh ra — chưa hề nằm trong gate lẫn acceptance** (Finding 2). Hai điểm này phải sửa trong plan TRƯỚC khi chạy Phase 1, nếu không GO/NO-GO không có giá trị bảo vệ. Findings 3-7 là lỗ vận hành trên box production-adjacent, sửa được bằng cách thêm bước/criterion vào phase files, không đổi kiến trúc.

## Unresolved questions

1. Staging LiveKit trên .65 đang chạy key gì (devkey hay đã rotate)? Quyết định độ khẩn của bonus Finding 6.
2. Webhook LiveKit hiện có được cấu hình ngoài repo không (participant_left cho host-lock có đang thực sự hoạt động trên staging)?
3. Registry cap đếm theo session hay track — cần planner chốt trước Phase 3 (Finding 7).
