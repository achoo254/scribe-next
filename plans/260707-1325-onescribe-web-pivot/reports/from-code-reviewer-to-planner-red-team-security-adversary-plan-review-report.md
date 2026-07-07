# Red-Team Security Adversary Review — OneScribe Web Pivot Plan

**Reviewer:** code-reviewer (security adversary lens) · **Date:** 2026-07-07
**Target:** `plans/260707-1325-onescribe-web-pivot/` (plan.md + phase-01..05)
**Posture:** hostile / attacker mindset. Advisory only — no plan/code mutated.
**Threat context:** public web app on shared staging box .65 (~15 prod/staging services co-located: mongod, cloudpanel, onemail, drive, VictoriaMetrics, LiveKit). New public surface: upload endpoint, self-rolled JWT auth, WebSocket PCM ingest, meeting bot token role, SQLite store, nginx vhost.

Verdict: **the plan is not safe to execute as written.** It inherits the hardened C6/C7/C8 model from meeting-server but breaks it in three places, adds a public media-decode RCE surface with only a spoofable MIME check, ships no object-level authorization, and its central CPU guardrail (`--cpu-threads 2`) does not exist in the code it claims to reuse unmodified.

---

## Finding 1: Public upload endpoint feeds PyAV/ffmpeg decoder with only a spoofable MIME allowlist — media-parser RCE surface on a shared prod box, with all secrets in the child's env

- **Severity:** Critical
- **Location:** Phase 2, "Architecture" (PyAV decode) + "Implementation Steps" step 4 (uploads.mjs) + step 2 (spawn `main.py`)
- **Flaw:** The plan opens a public, authenticated-by-shared-password upload endpoint accepting `webm/mp3/mp4/wav/m4a` and hands the raw file to `main.py` → faster-whisper, which decodes via PyAV/libav (`av==17.1.0`, `requirements.txt:5`). The only stated input defense is a "MIME allowlist" (step 4). MIME/`Content-Type` is client-controlled and trivially spoofed; the actual bytes reach ffmpeg's demuxer/decoder stack, one of the largest CVE surfaces in existence (crafted MP4/Matroska/opus → heap overflow / OOB). The plan specifies no magic-byte validation, no decode sandbox (seccomp/bwrap/separate low-priv uid), and no bound on decode-time memory beyond the cgroup cap. Worse: the batch runner spawns Python from Node, and Node inherits the systemd `EnvironmentFile` secrets (`LK_API_SECRET`, `HOST_SECRET`, the new JWT signing secret) — `infra/systemd/meeting-server.service:12` shows `EnvironmentFile` is already the pattern, and Phase 2 reuses it. The child Python inherits that full environment.
- **Failure scenario:** Attacker with the shared `ACCESS_PASSWORD` (see Finding 6) or, once SSO lands, any authenticated user uploads a crafted `.mp4`. libav decode achieves code execution inside the transcriber process. That process already holds `LK_API_SECRET` + `HOST_SECRET` + JWT secret in its environment and read access to `/opt/onescribe/uploads` and the SQLite DB (every user's audio + transcripts). With `NoNewPrivileges`/`ProtectSystem=strict` (Phase 5), the attacker cannot escalate to root but can exfiltrate all OneScribe secrets, forge host/bot tokens for any LiveKit room, read all transcripts, and probe the 14 co-located services from inside the box's network.
- **Evidence:** `sidecar/transcriber/requirements.txt:5` (`av==17.1.0`); `sidecar/transcriber/main.py:78` (`model.transcribe(file, ...)` — no pre-validation); `plans/.../phase-02-batch-transcribe-core.md` Architecture block ("PyAV ... decode webm/opus trực tiếp") + step 4 ("MIME allowlist"); `infra/systemd/meeting-server.service:12` (`EnvironmentFile` with secrets, inherited by children).
- **Suggested fix:** (a) Validate by magic bytes/container probe, not `Content-Type`. (b) Run decode+ASR under a dedicated unprivileged uid with a seccomp profile or `bwrap`/`systemd-run --scope` sandbox, filesystem-isolated to the single job's tmp dir. (c) Scrub the spawn environment — the runner must pass only `model_dir`/paths to Python, never `LK_API_SECRET`/`HOST_SECRET`/JWT secret (use `spawn(..., { env: { PATH, HOME, PYTHONPATH, ... } })`, not inherited `process.env`). (d) Add a per-job decode watchdog (wall-time + RSS) independent of the cgroup cap.

---

## Finding 2: No object-level authorization on `/api/jobs/:id` and transcripts — IDOR the moment SSO makes users distinct

- **Severity:** High
- **Location:** Phase 2, "Architecture" (SQLite `jobs(id, user, ...)`) + step 3 (`auth.mjs` middleware "verify cho mọi /api/*") + "Success Criteria" (`Anonymous request /api/* → 401`)
- **Flaw:** The plan defines a `user` column on `jobs` and an auth middleware that checks *authentication* ("verify(req)→user|null", 401 if anonymous) but never *authorization*. There is no stated ownership filter — nowhere does the plan say `GET /api/jobs/:id` must assert `job.user === req.user`. This is textbook broken object-level authorization (OWASP API1). It is masked during the interim phase because a single shared `ACCESS_PASSWORD` means every caller is the same principal — but that only defers the bug: Phase 5 introduces per-user SSO, at which point the missing ownership check becomes a live cross-tenant read of transcripts (which may contain meeting audio content, PII).
- **Failure scenario:** After OneDash SSO lands, user B enumerates/guesses job ids (sequential SQLite rowids or UUIDs seen in a shared link) and calls `GET /api/jobs/<A's id>` → receives A's full transcript and segment text. The auth middleware returns 200 because B is authenticated; nothing checks B owns the job.
- **Evidence:** `plans/.../phase-02-batch-transcribe-core.md` Architecture (`jobs(id, user, file, status, created)`), step 3 (middleware described as identity-only), Success Criteria (only tests anonymous→401, never cross-user). No ownership predicate appears in any phase. Contrast the hardened room-scoped check the codebase already does for captions: `server/meeting-server/index.mjs:140` (`verifyToken(tok, cfg.apiSecret, room)` binds token to room).
- **Suggested fix:** Make the ownership filter a data-access invariant from Phase 2, not a Phase-5 add-on: every job/transcript query is parameterized by `req.user` (`WHERE id=? AND user=?`), returning 404 (not 403) on mismatch. Add a Phase 2 success criterion: "user A cannot read user B's job/transcript." Use non-enumerable ids (UUIDv4), matching the room-id guidance already in `index.mjs:19`.

---

## Finding 3: WebSocket JWT passed in the URL query string — token leaks into nginx/proxy access logs on a shared box

- **Severity:** High
- **Location:** Phase 3, "Architecture" (`WS binary frames → /ws/live?token=JWT`) + step 1 ("JWT verify trên query/header")
- **Flaw:** The live-caption WS authenticates via `?token=JWT` in the URL. Query strings are logged by default in nginx `access_log`, by any upstream/proxy, and appear in `Referer`/browser history. On this box the vhost is nginx under shared `*.inetdev.io.vn` infra (brainstorm §2), so OneScribe's access logs likely sit alongside logs that other operators/services can read, and log retention outlives the token TTL. A bearer token in a log line is a replayable credential.
- **Failure scenario:** An operator (or a compromised co-located service with log read access, or log shipping to a shared VictoriaLogs/Loki) reads `GET /ws/live?token=eyJ...` from access logs and, within the 6h `TOKEN_TTL_SEC` window (`.env.example` default `TOKEN_TTL_SEC=21600`), replays it to open a live ASR session or, if it is the app-auth JWT, to hit `/api/*`. The victim never sees anything.
- **Evidence:** `plans/.../phase-03-live-caption-recording.md` Architecture diagram line (`/ws/live?token=JWT`) and step 1; `server/meeting-server/lib/config.mjs:12` + `.env.example` (`TOKEN_TTL_SEC=21600` — 6h replay window). Browsers cannot set custom headers on the `WebSocket` handshake, which is *why* query-string tokens are a common trap here.
- **Suggested fix:** Do not put the JWT in the URL. Use the `Sec-WebSocket-Protocol` subprotocol header to carry the token (server echoes the accepted subprotocol), or issue a short-lived (≤60s), single-use WS ticket from an authenticated `/api/ws-ticket` POST and pass only that opaque one-time ticket in the query. Additionally set `access_log off` or a token-scrubbing log format on the `/ws/live` location and cap `TOKEN_TTL_SEC` for the WS credential.

---

## Finding 4: Meeting caption trust model regresses C7 — the bot cannot publish captions the current client accepts, and the human "host" retains `canPublishData` (fake-caption injection)

- **Severity:** High
- **Location:** Phase 4, "Non-functional" ("host" = room manager, KHÔNG transcribe) + step 2 (bot token `canPublishData=false`) + step 5 ("caption store: nguồn ghi = bot thay host"; "giữ 2 lớp chống giả mạo như C7")
- **Flaw:** The existing anti-spoofing model has two coupled halves: (1) only `role==="host"` tokens get `canPublishData` (`jwt.mjs:22`), and (2) the client only renders data-channel captions from a participant whose signed metadata role is `"host"` (`caption-feed.ts:14-23,44-45`). Additionally, POSTing to the HTTP caption store requires `role==="host"` (`index.mjs:147`). Phase 4 redefines roles: captions now originate from a subscribe-only **bot** (`canPublish=false, canPublishData=false`), and "host" becomes a mere room manager. This breaks the model three ways, none reconciled by the plan:
  1. The bot is `canPublishData=false`, so it **cannot** push captions over the LiveKit data channel — yet `caption-feed.ts` realtime path (`RoomEvent.DataReceived`) is data-channel-only (`caption-feed.ts:61`). The plan says "tái dùng caption-feed.ts" but never specifies the new realtime transport (poll the store? a second WS to clients?). The realtime caption path is undefined.
  2. `signToken` grants `canPublishData` iff `role==="host"` (`jwt.mjs:22`). If the human host keeps `role==="host"` (needed for the C6 host-lock they still hold as room manager), that host **still gets `canPublishData=true` and the client still renders their data-channel captions** (`isHostSender` returns true for role host, `caption-feed.ts:18-22`). So a malicious room creator can inject fake captions — exactly the C7 attack, now reopened because "host" no longer means "the trusted transcriber."
  3. The caption store POST gate is `role==="host"` (`index.mjs:147`); a `role==="bot"` token gets 403. Phase 4 must therefore *modify* this authorization (contradicting "route đã có, JWT room-scoped" which implies reuse), and the plan doesn't state the new predicate or how bot tokens are scoped per-room.
- **Failure scenario:** Room creator (legitimately obtains a host token via `HOST_SECRET`) opens the meeting, then uses their `canPublishData=true` grant to publish crafted `{type:"caption"}` data messages. Every participant's `caption-feed.ts` renders them as authoritative because the sender's signed role is `"host"`. The plan's own success criterion ("Guest không thể publish caption giả") tests only *guests* (who already have `canPublishData=false`), so it passes while the real hole — the host — is untested.
- **Evidence:** `server/meeting-server/lib/jwt.mjs:22` (`canPublishData: o.role === "host" && ...`); `packages/meeting-shared/src/caption-feed.ts:14-23` (`isHostSender` trusts `role==="host"`), `:44-45` (`onData` gate), `:61` (data-channel-only realtime); `server/meeting-server/index.mjs:147` (caption POST requires `role==="host"`); `plans/.../phase-04-meeting-web-caption-bot.md` step 2 + step 5.
- **Suggested fix:** Introduce a distinct `role="bot"` that is the *only* role granted caption-write, and flip both gates to it: (a) client `isHostSender` → `isCaptionSource` checking `role==="bot"`; (b) caption store POST predicate → `role==="bot"`. Explicitly set the human host to `canPublishData:false` (they manage the room, not captions) — pass `canPublish/canPublishData` false in `signToken` for the new host semantics. Define the realtime client transport concretely (recommend: clients poll/subscribe the caption store, drop data-channel captions entirely so no participant grant can inject). Add a success criterion that a **host** cannot inject captions.

---

## Finding 5: The `--cpu-threads 2` guardrail does not exist in the code the plan reuses "unmodified" — CPU-exhaustion DoS on the shared box rests entirely on a systemd cap that may not apply

- **Severity:** High
- **Location:** Phase 1, step 5 (`main.py ... --cpu-threads 2`) + step 6; Phase 2, "Architecture" (`main.py ... --cpu-threads 2`) + step 2 ("threads 2 ... tái dùng nguyên, KHÔNG sửa"); Decision #7 / plan.md guardrails ("batch queue=1; ... batch nice 10; 2 threads")
- **Flaw:** The plan repeatedly commits to a 2-thread CPU guardrail and simultaneously insists the sidecar is reused "nguyên, KHÔNG sửa" (unmodified). Both cannot be true. `main.py` has no argument parser and no `--cpu-threads` flag — it only checks `"--stream" in sys.argv` (`main.py:139-147`) and hardcodes `cpu_threads=os.cpu_count() or 8` (`main.py:67`). The streaming path defaults identically: `load_stream_model` is called with no thread arg, falling back to `os.cpu_count() or 4` (`stream_session.py:37`). On the 4-vCPU box, every batch job and the stream process each spin **4 threads**, not 2. So either (a) the sidecar must be modified — invalidating the "reuse tested code unmodified" safety claim and the 10-test regression gate — or (b) it isn't, and the only thing standing between OneScribe and CPU starvation of the 14 co-located services is `CPUQuota=200%` in a not-yet-written systemd unit.
- **Failure scenario:** A batch job (4 threads) runs while two live/meeting streams are active (stream process, 4 threads, serialized by `_model_lock` but still scheduling 4) on 4 physical vCPUs. If `CPUQuota` fails to apply — cgroup v1 vs v2 mismatch on the AlmaLinux/CentOS box, unit typo, or the child escaping the service cgroup — CPU saturates and mongod/onemail/LiveKit RTP jitter/VictoriaMetrics scrape all degrade. The "measure before/after" acceptance test in Phase 1 uses the correct 2-thread assumption that production won't honor, so the benchmark under-reports real contention.
- **Evidence:** `sidecar/transcriber/main.py:139-147` (only `--stream` parsed; no argparse), `main.py:67` (`cpu_threads=os.cpu_count() or 8`); `sidecar/transcriber/stream_session.py:37` (`cpu_threads or (os.cpu_count() or 4)`, called with no arg from `stream_server.py:56`); `plans/.../phase-01` step 5/6, `plans/.../phase-02` step 2. Existing unit has **no** `CPUQuota`/`MemoryMax` (`infra/systemd/meeting-server.service` — absent), so the guardrail is entirely net-new and unverified.
- **Suggested fix:** Decide explicitly: either (a) add a `--cpu-threads` CLI arg (and a size arg) to `main.py`/`stream_server.py` and re-run the sidecar test suite — treat this as a *modification*, not reuse; or (b) set `OMP_NUM_THREADS`/`CT2` thread env in the spawn environment and pass thread count via the existing JSON protocol. Verify `CPUQuota`/`MemoryMax` actually bind the child cgroup on the target OS (`systemctl status` showing the python PIDs under the unit's cgroup) as a hard Phase-1/Phase-2 gate, not an assumption. Correct Phase 1's benchmark to run at the true production thread count.

---

## Finding 6: One shared `ACCESS_PASSWORD` guarding a public app, a weak reused rate limiter, and no fail-fast on empty secret → brute-force + accidental auth bypass

- **Severity:** High
- **Location:** Phase 2, step 3 (`auth.mjs` JWT HS256, login by `ACCESS_PASSWORD` env) + "Success Criteria" ("login sai → 401 + rate-limit (tái dùng code rate-limit meeting-server)")
- **Flaw:** Three compounding weaknesses in the interim auth that guards the entire public surface:
  1. **Single shared credential.** One `ACCESS_PASSWORD` for all users. There is no per-user identity, so audit trails, the `jobs.user` column, and any "who uploaded this" story are fiction until SSO. If the password leaks once (it will be pasted in chats/wikis), the whole app is open.
  2. **Weak, reused limiter.** The plan reuses `createRateLimiter({capacity:20, refillPerSec:1})` (`rate-limit.mjs:2`, default 20 burst / 1 per second sustained). Against a single password that is 20 guesses instantly + 3,600/hour/IP sustained — and if `TRUST_PROXY=true` behind nginx, the bucket key is the client-supplied `X-Forwarded-For` first hop, so a distributed guesser rotates IPs for effectively unlimited attempts (the code even warns about this: `index.mjs:63-71`). No lockout, no exponential backoff, no CAPTCHA.
  3. **No fail-fast on empty secret.** `validateConfig` requires only `LK_API_KEY`, `LK_API_SECRET`, `HOST_SECRET` (`config.mjs:25-31`). The plan adds `ACCESS_PASSWORD` and a JWT signing secret but never says to extend `validateConfig`. If `ACCESS_PASSWORD` is unset, a `safeEq(input, "")` comparison authenticates an empty password, or a `""` HS256 signing secret makes tokens forgeable. Deploy-time omission → silent full bypass.
- **Failure scenario:** Operator deploys, forgets to set `ACCESS_PASSWORD` in the env file (it isn't in `.env.example`, so it's easy to miss). Server boots fine (validateConfig doesn't check it). `POST /auth/login` with `{password:""}` returns a valid JWT. Attacker has full app access with zero credentials. Alternatively: password is set but short; attacker rotates `X-Forwarded-For` and brute-forces it in minutes.
- **Evidence:** `server/meeting-server/lib/rate-limit.mjs:2` (`capacity=20, refillPerSec=1`); `server/meeting-server/index.mjs:63-71` (XFF-spoof warning), `:101`,`:117` (limiter applied only to `/token/*`); `server/meeting-server/lib/config.mjs:25-31` (validateConfig omits any app-auth secret); `server/meeting-server/.env.example` (no `ACCESS_PASSWORD`, no JWT secret). Plan `phase-02` step 3 + Success Criteria.
- **Suggested fix:** Extend `validateConfig` to hard-fail if `ACCESS_PASSWORD` or the JWT signing secret is empty/short (fail-fast, matching the existing `HOST_SECRET` pattern). Add both to `.env.example` with `please-change` placeholders. Use a *dedicated, stricter* limiter for `/auth/login` (small burst, per-account exponential backoff + temporary lockout), keyed on socket IP unless behind a verified single trusted proxy. Document that the interim single-password mode is not multi-user and the `jobs.user` column is a placeholder until SSO.

---

## Finding 7: Webhook-driven bot spawn adds a process-control surface, but `verifyWebhook` checks no expiry/nonce and `/webhook` has no rate limit — replayable spawn trigger

- **Severity:** Medium
- **Location:** Phase 4, step 4 (`bot-manager.mjs`: webhook `room_started` → spawn bot; `room_finished`/idle → kill; crash → respawn ≤3) + step 2 ("webhook handler đã có verifyWebhook — nối bot-manager")
- **Flaw:** Phase 4 wires LiveKit webhooks to **spawn and kill OS processes**. The existing `verifyWebhook` validates the HMAC signature and a `sha256` body digest but performs **no `exp`/`nbf`/timestamp/nonce check** (`index.mjs:189-215`) — its own comment claims "chống replay body khác," which only prevents *body* tampering, not *replay of the same body*. `/webhook` is also **not rate-limited** (the limiter is applied only to `/token/guest` and `/token/host`, `index.mjs:101,117`). Today, replaying a `participant_left` webhook is harmless (idempotent lock release). Once the same endpoint drives `spawn/kill`, replay becomes a process-control primitive. Also note: the plan asserts `room_started`/`room_finished` handling is reuse ("webhook handler đã có verifyWebhook"), but the current handler processes **only** `participant_left` (`index.mjs:169`); the new events require both new server code and a LiveKit server webhook-config change (which events are emitted) that no phase covers.
- **Failure scenario:** Anyone who can read one valid webhook request off the wire/logs (or a co-located service that sees loopback traffic) replays a captured `room_started` payload repeatedly. `bot-manager` dedups to 1 caption room so it can't fork-bomb, but replaying `room_finished` for the active room kills the live caption bot on demand (caption DoS for a legitimate meeting), and rapid `room_started`/`room_finished` alternation drives spawn/kill churn (repeated `livekit-rtc` process starts, each loading state) with no rate limit to damp it.
- **Evidence:** `server/meeting-server/index.mjs:189-215` (`verifyWebhook` — signature + `sha256` body only, no `exp`/`nbf`/nonce), `:160-177` (only `participant_left` handled), `:101`/`:117` (limiter on token routes only, `/webhook` uncovered); `plans/.../phase-04` step 2 + step 4.
- **Suggested fix:** In `verifyWebhook`, enforce the webhook JWT's `exp` and reject stale timestamps (LiveKit webhook tokens carry `exp`); add a short-window nonce/`jti` cache to reject replays. Rate-limit `/webhook`. Make `bot-manager` state transitions idempotent and debounced (ignore `room_finished` for a room that already has no bot; cap spawn frequency). Add a phase step for the LiveKit server webhook config (event list + endpoint), since `room_started`/`room_finished` handling is net-new, not reuse.

---

## Verification Results (fact-check: file paths, symbols, endpoints, config keys)

Legend: VERIFIED (exists as claimed) · FAILED (claim contradicted by code) · UNVERIFIED (external/not in repo).

### Phase 1 — Benchmark ASR Gate
| Claim | Result | Evidence |
|---|---|---|
| `sidecar/transcriber/stream_asr_spike.py` exists | VERIFIED | file present, 10516 bytes |
| `sidecar/transcriber/main.py` batch protocol | VERIFIED | `main.py:107-136` |
| `model_paths.py` small/medium = `quocphu/PhoWhisper-ct2-FasterWhisper` | VERIFIED | `model_paths.py:11-12` |
| `main.py --cpu-threads 2` (step 5) | **FAILED** | `main.py:139-147` no argparse; `main.py:67` hardcodes `os.cpu_count()` |
| `stream_asr_spike.py --vad-cut` (step 6) | **FAILED (flag name)** | flag is `--strategy vad-cut`, `stream_asr_spike.py:49`; no bare `--vad-cut` |
| `stream_asr_spike.py --cpu-threads`, `--realtime` | VERIFIED | `stream_asr_spike.py:48,59` |
| `rtf_mean` / `aggregate_rtf_wall` / `latency_valid` fields | VERIFIED | `stream_asr_spike.py:181-190` |
| spike results `spike-streaming-asr-results.md` | VERIFIED | file present under plan 260626 reports |
| `requirements.txt` (no silero/torch) | VERIFIED | `requirements.txt` (webrtcvad, no torch) |

### Phase 2 — Batch Transcribe Core
| Claim | Result | Evidence |
|---|---|---|
| meeting-server `index.mjs` = 236 lines | VERIFIED | `wc -l` = 236 |
| Reuse "transcribe cmd, `transcribe_msg` events" unmodified | **FAILED** | protocol events are `ready/download/model_ready/progress/done/error` (`main.py:7-8`); no `transcribe_msg` symbol anywhere |
| `--cpu-threads 2` on `main.py` | **FAILED** | `main.py:67` `os.cpu_count()`; no flag |
| PyAV `av==17` decodes webm/opus, no ffmpeg CLI | VERIFIED (plausible) | `requirements.txt:5` `av==17.1.0`; faster-whisper decodes via libav |
| `fmt_ts` srt formatter in sidecar | VERIFIED | `main.py:39` (Python-side; FE cannot reuse it directly — minor) |
| reuse rate-limit code | VERIFIED | `server/meeting-server/lib/rate-limit.mjs` |
| tests "node:test, theo pattern test meeting-server" | **FAILED** | existing tests use a custom runner, not node:test — `run-tests.mjs:58` (`const test = (name,fn)=>TESTS.push`) |
| `node:sqlite` / `better-sqlite3` | VERIFIED (net-new) | none in repo; plan acknowledges |
| `git mv server/meeting-server server/onescribe-server` target | VERIFIED (dir exists to move) | `server/meeting-server/` present |
| systemd unit adds `CPUQuota=200%`, `MemoryMax=2500M` | VERIFIED (net-new) | existing `meeting-server.service` has neither |
| path-traversal fix "server-generate filename" | UNVERIFIED (design intent) | not yet in code |

### Phase 3 — Live Caption Recording
| Claim | Result | Evidence |
|---|---|---|
| `stream_server.py`, `stream_session.py` exist | VERIFIED | both present |
| protocol `stream_start/stream_pcm(base64)/stream_stop` | **FAILED (symbol)** | command is `stream_end`, not `stream_stop` — `stream_server.py:12-19,178` |
| desktop G6 `src/composables/use-audio-tap-probe.ts` | VERIFIED | file present |
| desktop `use-live-caption.ts` | VERIFIED | `src/composables/use-live-caption.ts` present |
| `packages/meeting-shared/src/caption-feed.ts` reusable for WS feed | VERIFIED (exists) / caveat | present, but realtime path is data-channel-only (`caption-feed.ts:61`) — see Finding 4 |
| lib `ws` net-new dep | VERIFIED (net-new) | not in `apps/web` or server deps |
| global cap ≤2 enforced in sidecar | FAILED (not in sidecar) | `stream_server.py` allows arbitrary tracks; cap must live in Node registry (design note) |
| `/proc/meminfo` RAM guard before acquire (risk) | UNVERIFIED (design intent) | net-new |

### Phase 4 — Meeting Web Caption Bot
| Claim | Result | Evidence |
|---|---|---|
| `verifyWebhook` already exists | VERIFIED (partial) | `index.mjs:189`; but only `participant_left` handled (`:169`), not `room_started/finished` |
| `server/dev-token/generate-token.mjs` | VERIFIED | present; roles host/guest only (`:20`), no bot |
| `apps/web/src/MeetingRoomPage.vue` | VERIFIED | present |
| `packages/meeting-shared/src/meeting-client.ts`, `use-livekit-viewer.ts` | VERIFIED | both present |
| token `role=bot` reuses `signToken` | FAILED (needs change) | `jwt.mjs:22` grants `canPublishData` only for `role==="host"`; bot role gets no write path |
| caption store write "route đã có" for bot | FAILED (needs change) | `index.mjs:147` requires `role==="host"` to POST captions |
| `livekit-rtc` Python runs on box | UNVERIFIED (external) | plan gates via mini-spike — appropriate |
| plan 260626 `phase-03-host-transcribe-relay.md` reference | VERIFIED | file exists under 260626 |

### Phase 5 — SSO Retention Hardening
| Claim | Result | Evidence |
|---|---|---|
| rate-limit + `TRUST_PROXY` code exists to reuse | VERIFIED | `rate-limit.mjs`; `config.mjs:16`; used `index.mjs:66` (plan says "in meeting.mjs" — that module is created in Phase 2 by move) |
| VictoriaMetrics/vmagent on box | UNVERIFIED (external) | brainstorm context only |
| `auth-onedash.mjs`, `metrics.mjs`, `retention.mjs` net-new | VERIFIED (net-new) | not present |
| OneDash auth contract | UNVERIFIED (external) | explicitly pending per plan |
| infra uses nginx | FAILED (mismatch) | repo infra ships **Caddy** (`infra/caddy/Caddyfile`) + no `infra/nginx/`; plan targets nginx vhost — box may use nginx (brainstorm), but no repo artifact exists to reuse |

**Sampled:** 40 claims across 5 phases. FAILED: 9 (5 are load-bearing: `--cpu-threads`, `transcribe_msg`, bot role write path ×2, `stream_stop`). UNVERIFIED (external, acceptable): 6.

---

## Summary of severities
- **Critical (1):** #1 public media-decode RCE surface with secret-laden child env.
- **High (5):** #2 IDOR/no object authz, #3 WS token in URL, #4 caption C7 regression + host injection, #5 phantom `--cpu-threads` guardrail / CPU DoS, #6 shared password + weak limiter + no fail-fast.
- **Medium (1):** #7 replayable/unthrottled webhook driving process spawn.

## Cross-cutting recommendation
The plan's strongest safety asset is the already-hardened meeting-server (C6/C7/C8, XFF-aware rate limiting, fail-fast config). Three findings (#4, #5, #6, #7) are regressions or gaps *against that existing baseline* — the plan reuses the code but not its invariants. Before Phase 2 starts: (1) extend `validateConfig` for the new secrets, (2) make ownership filtering and env-scrubbed spawns non-negotiable data-access invariants, and (3) resolve the "reuse unmodified vs. `--cpu-threads 2`" contradiction explicitly, because the 10-test regression gate is void the moment the sidecar is edited.
