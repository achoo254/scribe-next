# Red-Team Review: Scope & Complexity Critic
**Plan:** 260626-1418-realtime-meeting-live-transcribe
**Reviewer role:** Scope & Complexity Critic (YAGNI enforcer)
**Date:** 2026-06-26

---

## Finding 1: "Bot join Meet/Zoom" alternative never evaluated — cheapest path dismissed without evidence
**Severity:** Critical

**Location:** `plan.md` § "Out of scope" — no mention; `plan.md` §Overview — states conferencing = commodity without costing alternatives.

**Flaw:** The plan's stated USP is "live-transcribe tiếng Việt + privacy." This is achievable today by building a **headless bot** that joins an existing Google Meet or Zoom call (via browser automation / Recall.ai-style capture), captures the audio stream, and pipes it into the existing `sidecar/transcriber/main.py` — which already handles the entire batch transcribe pipeline (`main.py:86–115`). No SFU, no TURN, no Redis, no web client, no monorepo refactor. Estimated delta from current codebase: ~1 phase of work vs 5 phases here.

The plan's out-of-scope list (`plan.md:73–78`) drops MCU, failover, media storage — but never surfaces the bot-join alternative as a considered-and-rejected option. The validation session asked 4 questions (`plan.md:91–95`), none of which were "should we build the SFU at all?"

**Failure scenario:** Team spends 24–37 days building a Zoom-clone nobody uses when they could validate the transcribe-of-meetings use-case in 3–5 days by attaching to an existing call.

**Evidence:**
- `sidecar/transcriber/main.py:86` — `transcribe()` already accepts any audio file; pipeline is complete.
- `plan.md:71–78` — alternative approaches list does NOT include bot-join or screen-capture-of-existing-call.
- `README.md:9` — "Ghi tiếng hệ thống (WASAPI loopback)" already implemented — loopback of a Meet tab IS the bot alternative on Windows.

**Suggested fix:** Before committing to 5 phases, add a Phase 0 spike (1–2 days): use existing WASAPI loopback (`README.md:9`) to capture audio of a Meet/Zoom browser tab and transcribe it with the existing sidecar. If that satisfies the core use case, kill Phases 2–5 entirely.

---

## Finding 2: `BatchedInferencePipeline` is incompatible with streaming — critical architectural assumption is wrong
**Severity:** Critical

**Location:** `phase-01-spike-streaming-asr.md:39–45`; `sidecar/transcriber/main.py:75–80`

**Flaw:** Phase 1 proposes using `whisper_streaming` (UFAL) with faster-whisper as backend for streaming ASR. However, `main.py:75–80` already uses `BatchedInferencePipeline` — which is a **batched** pipeline incompatible with the `whisper_streaming` library's expected `faster_whisper.WhisperModel` interface. `whisper_streaming` wraps `WhisperModel` directly, not `BatchedInferencePipeline`. Switching to streaming mode means **throwing away the `BatchedInferencePipeline` optimization** that gives "~2x faster on CPU" for the existing batch Mode C path, OR maintaining two separate model instances simultaneously (doubling RAM: ~1 GB for small, up to ~6 GB for two mediums).

The plan says "extend sidecar — do NOT break Mode C" (`phase-03:44`), but doesn't account for the fact that Mode C's warm model (`BatchedInferencePipeline`) and the streaming spike's required `WhisperModel` are **different Python objects**. Running both warm simultaneously on a machine with 8–16 GB RAM during a meeting is a real risk.

**Failure scenario:** Phase 1 spike discovers that streaming requires its own `WhisperModel` instance separate from the batch pipeline. During a meeting (streaming active), user triggers a Mode C transcribe → system tries to hold two models in RAM → OOM on 8 GB machines, or forced reload that kills streaming latency.

**Evidence:**
- `sidecar/transcriber/main.py:75–80` — `BatchedInferencePipeline` is the current warm model.
- `sidecar/transcriber/requirements.txt:3` — `faster-whisper==1.2.1` pinned; `whisper-streaming` not in requirements at all (zero presence in entire repo: grep confirms).
- `phase-01:57` — "thêm `whisper-streaming`" into venv — implies a new dependency not yet validated against pinned `faster-whisper==1.2.1`.
- grep for `whisper_streaming|whisper.streaming` in `sidecar/` returns 0 matches in any `.py` file.

**Suggested fix:** Phase 1 spike must explicitly test the two-model-in-memory scenario. If RAM is the bottleneck, the architecture must choose: either streaming replaces batch (breaking Mode C warm), or use VAD-cut-utterance with the existing `BatchedInferencePipeline` (simpler, avoids the two-instance problem, possibly sufficient latency). Plan currently presents `whisper_streaming` as the default path without validating this conflict.

---

## Finding 3: Monorepo refactor of Tauri app (Phase 4) is high-risk scope with zero current evidence of feasibility
**Severity:** High

**Location:** `phase-04-web-client-vue.md:33,47`, `plan.md:68`, `plan.md:99`

**Flaw:** Phase 4 Step 1 mandates moving the entire Tauri app into `apps/desktop/` as part of an npm workspaces monorepo. The current `tauri.conf.json:8–9` has `frontendDist: "../dist"` and `devUrl: "http://localhost:1420"` with `beforeBuildCommand: "npm run build"` — all relative to the current flat structure. The Tauri sidecar resolution in `transcribe.rs:108–116` (`find_script()`) walks ancestor directories looking for `sidecar/transcriber/main.py`. Moving `src-tauri/` one level deeper into `apps/desktop/src-tauri/` changes every relative path: `tauri.conf.json`, `find_script()`, all capability files, `Cargo.toml`'s `[[bin]]` references.

The plan acknowledges "có thể đụng cấu hình" (`phase-04:66`) but marks it as a risk to "giảm thiểu" rather than as a blocking prerequisite to verify. The validation session confirmed monorepo as "chốt" without a feasibility spike.

**Failure scenario:** Step 1 of Phase 4 breaks the working Tauri app. Recovering takes longer than building the web client on a separate repo. The risk note says "làm trên nhánh riêng để revert" — but the plan also says "KHÔNG tự tạo branch" per CLAUDE.md git rules.

**Evidence:**
- `src-tauri/tauri.conf.json:8–9` — `frontendDist: "../dist"` path becomes invalid if `src-tauri` moves.
- `src-tauri/src/transcribe.rs:109–116` — `find_script()` walks from `current_exe()` ancestors; path breaks when binary location changes.
- `package.json:1` — flat `"name": "one-scribe"`, no workspaces declared anywhere in repo.
- `src-tauri/Cargo.toml` — no workspace declaration; changing to workspace member requires coordinated `Cargo.toml` edits at root.

**Suggested fix:** Either (a) skip monorepo entirely for MVP — ship web client as a separate standalone Vite project, share code by copy+adjust (acceptable for MVP ≤8 person scope), or (b) add a Phase 0 monorepo feasibility spike BEFORE committing. DRY at this scale does not justify the refactor risk.

---

## Finding 4: Audio tap path (WebView2 JS → PCM → Rust IPC → Python) has no precedent in this codebase and no spike planned before Phase 3
**Severity:** High

**Location:** `phase-03-host-transcribe-relay.md:33–46`, `phase-01:line note about audio tap`

**Flaw:** Phase 3's core data path is: WebRTC remote audio track → `MediaStreamTrackProcessor`/`AudioWorklet` in WebView2 JS → Tauri IPC command → `transcribe.rs` → sidecar stdin PCM. This is a completely novel IPC path not present anywhere in the codebase. The current IPC for transcribe (`commands.rs:140–154`) passes **file paths** only — the sidecar reads audio from disk, not from a streaming byte buffer over stdin. Phase 3 requires extending `transcribe.rs` to accept continuous PCM byte streams per-track over Tauri commands, which means async streaming IPC, backpressure management, and chunked binary transfer through Tauri's message-passing system.

Phase 3 says "chốt đường đi cụ thể dựa trên kết quả spike Phase 1" (`phase-03:45`) — but Phase 1 spike (`phase-01:51–62`) only measures ASR latency from **file WAV / stdin PCM**, NOT the full JS→Rust→Python path. The hardest unknown — whether WebView2's `MediaStreamTrackProcessor` can pump PCM to Rust at 16kHz continuously without buffer overflow or latency spikes — is not tested in Phase 1.

**Failure scenario:** Phase 3 discovers that Tauri's command/event IPC introduces 200–500ms overhead per chunk due to JSON serialization/deserialization of binary PCM data (the current protocol is line-delimited JSON, `main.py:46`). Throughput becomes the bottleneck, not ASR speed. Entire host-transcribe architecture requires redesign at Phase 3, after Phase 2 (5–8d) has already been built.

**Evidence:**
- `src-tauri/src/transcribe.rs:183–189` — `send()` writes line-delimited JSON to stdin; no binary/streaming path exists.
- `sidecar/transcriber/main.py:125–148` — reads line-by-line from `sys.stdin`; no streaming byte input protocol.
- `src/composables/` — 4 composables exist, none touch `MediaStream`, `AudioWorklet`, or `AudioContext` (grep confirms zero matches).
- `phase-01:51–54` — spike only creates `stream_asr_spike.py` reading PCM from "stdin/WAV files", not from Tauri IPC.

**Suggested fix:** Extend Phase 1 spike to include a proof-of-concept of the full JS→Tauri→Python PCM pipe. Without this, Phase 3 is not de-risked and the "RISK GATE" label on Phase 1 is incomplete.

---

## Finding 5: Self-host LiveKit + coturn + Redis + Caddy for MVP ≤8 people — gold-plating ops burden
**Severity:** High

**Location:** `phase-05-server-infra-retention.md` entire file; `phase-02:41` (`docker-compose.yml`)

**Flaw:** Phase 5 mandates running 4 separate services: LiveKit SFU, coturn TURN server, Redis, and Caddy reverse proxy — all self-hosted. For an MVP of ≤8 people, this is the operational equivalent of building a data center for a neighborhood party. LiveKit Cloud (managed) offers a free tier with 50 concurrent participants and handles TURN, TLS, and scaling automatically. The plan's own risk note acknowledges "Redis là thêm 1 dependency vận hành" (`phase-05:78`) and "chi phí băng thông egress là hóa đơn thật" (`phase-05:75`) but does not present LiveKit Cloud as an alternative.

The plan's architectural decision table (`plan.md:25`) lists "LiveKit self-host (Apache 2.0)" as "đã chốt" with no recorded rationale for rejecting managed. The validation session (`plan.md:91–95`) never asked this question.

**Failure scenario:** Team spends 5–7 days (Phase 5 estimate) on infra setup and discovers that egress costs for video are prohibitive self-hosted, or that coturn configuration is broken for enterprise firewalls. By then, LiveKit Cloud would have handled it for free at MVP scale. Alternatively, server maintenance becomes an ongoing distraction for a desktop app team with no DevOps background (Cargo.toml and requirements.txt show no server-side dependencies).

**Evidence:**
- `plan.md:25-26` — Decision #1 states self-host with no cost/ops trade-off documented.
- `phase-05:47` — creates `server/token-service/`, `server/caption-store/`, `docker/coturn/`, `docker/proxy/` — 4 new directories, zero of which exist in repo.
- `package.json` — no server-side deps; `Cargo.toml` — no server deps; entire repo is a desktop app.
- LiveKit Cloud free tier: 50 participants, 100GB/month egress — covers MVP ≤8 people with zero ops cost.

**Suggested fix:** Replace Phase 5 with LiveKit Cloud for MVP. Token service reduces to a 20-line serverless function (Vercel/Cloudflare Worker). TURN is managed. TLS is managed. Redis caption store can be a Cloudflare KV or Upstash with TTL. Total infra phase effort: 1–2 days vs 5–7 days, with zero ongoing ops burden.

---

## Finding 6: Phase 4 Web Client is out-of-scope for desktop-app MVP — pure gold-plating
**Severity:** High

**Location:** `phase-04-web-client-vue.md` entire file; `plan.md:67` (Phase 4 scope "chốt")

**Flaw:** The stated MVP target is ≤8 people using the app. Every non-host participant in Phase 4 is described as a "consumer thuần — xem video + đọc caption" (`phase-04:14`). This consumer role could be filled by another desktop app user (install OneScribe, join as non-host). The web client is a quality-of-life feature for participants who don't want to install the desktop app — valuable eventually, but not required to prove the core value proposition (Vietnamese live transcription in a meeting).

Building a separate web client in Phase 4 requires: monorepo refactor (high risk, Finding 3), a new Vite app, ViUI npm package alignment, Safari/iOS WebRTC testing, HTTPS/TLS (Phase 5 dependency), and a token endpoint. That is 5–8 days of scope that does not prove whether people want the live-transcription feature itself.

**Failure scenario:** Team ships a web client for a meeting feature that nobody uses yet (no users validated), while the core transcription quality (Phase 1 GO/NO-GO) might not even pass. If Phase 1 is NO-GO, Phases 2–5 are wasted.

**Evidence:**
- `plan.md:67` — Phase 4 marked P2 (lower priority) but included in MVP scope.
- `phase-04:33` — "Chấp nhận chi phí refactor" — team already knows it has a cost.
- README.md — product is described as a desktop app throughout; zero mention of web access as a goal.
- `package.json` — no web server, no router, no HTTP framework — web client would require building an entirely separate application from scratch.

**Suggested fix:** Defer Phase 4 entirely until after Phase 1–3 prove the core value. Replace web participant for MVP with "install desktop app, join as non-host." Add Phase 4 to roadmap as post-MVP.

---

## Finding 7: Effort estimate (24–37 days) understates actual complexity — critical phases have uncapped unknowns
**Severity:** High

**Location:** Phase effort fields: P1=3–5d, P2=5–8d, P3=6–9d, P4=5–8d, P5=5–7d; `plan.md:55–60` (risk list)

**Flaw:** The estimates treat each phase as independent. Phase 3 (6–9d) assumes Phase 1 spike produced a working streaming ASR implementation AND that the JS→Rust PCM pipe works. If either assumption fails and requires redesign, Phase 3 expands to 15–20d. Phase 4 (5–8d) assumes the monorepo refactor in Step 1 "just works" — it's actually an unbounded refactor touching `tauri.conf.json`, `Cargo.toml`, `find_script()`, all capability files, and npm scripts. Phase 5 (5–7d) for coturn alone is notoriously tricky for enterprise NAT traversal.

More critically, the plan has no contingency estimate or explicit NO-GO exit criteria beyond Phase 1. If Phase 1 is GO, the plan commits to all 4 remaining phases with no re-evaluation gate.

**Evidence:**
- Phase dependencies: P3 depends on P1 AND P2; P4 depends on P2 AND P3; P5 depends on P2, P3, AND P4 — sequential bottleneck where any slip cascades.
- `phase-03:80` — "Tap PCM từ remote WebRTC track là phần khó/chưa có tiền lệ" — the hardest risk is explicitly labeled but given no separate time budget.
- `phase-04:66` — monorepo refactor risk given "làm từng bước" mitigation but no time allocation.
- Total: 24–37 days best case. Realistic with 1 major slip: 40–60 days.

**Suggested fix:** After Phase 1 GO, add a re-evaluation gate before committing Phase 2 budget. Define explicit time boxes per risky step (e.g., "if PCM IPC not working in 2 days, pivot to server-side ASR"). Build a minimum-cut MVP: P1 → P2 → P3 (desktop-only, no web client, LiveKit Cloud) = 14–22 days, proving the core value before P4/P5.

---

## Finding 8: `whisper_streaming` dependency not validated against `faster-whisper==1.2.1` pin
**Severity:** Medium

**Location:** `phase-01-spike-streaming-asr.md:57`; `sidecar/transcriber/requirements.txt:3`

**Flaw:** `requirements.txt` pins `faster-whisper==1.2.1`. The `whisper_streaming` library from UFAL has its own `faster-whisper` version requirements that may conflict. Additionally, `whisper_streaming` is not on PyPI under a stable name — the UFAL repo (`ufal/whisper_streaming`) is installed via git URL or as a local package. The plan says "thêm `whisper-streaming`" without specifying the install mechanism or validating it against the pinned `faster-whisper`.

**Failure scenario:** `pip install whisper-streaming` installs a different package (name collision); or the UFAL version forces `faster-whisper` to upgrade, breaking the existing PyInstaller build (`build_pyinstaller.ps1`) which was pinned specifically for stability.

**Evidence:**
- `sidecar/transcriber/requirements.txt:3` — `faster-whisper==1.2.1` hard-pinned.
- `phase-01:57` — "thêm `whisper-streaming` (hoặc tích hợp thủ công LocalAgreement)" — "hoặc tích hợp thủ công" implicitly acknowledges the package may not be directly installable.
- Zero evidence of `whisper_streaming` anywhere in repo (grep returns 0 matches in sidecar/).

**Suggested fix:** Phase 1 spike must explicitly resolve the install mechanism (git+URL vs manual copy), verify no version conflict with `faster-whisper==1.2.1`, and update `requirements.txt` or document why a separate venv is needed for the spike.

---

## Finding 9: Token service in Phase 5 has no auth — anyone can create a host token
**Severity:** Medium

**Location:** `phase-05:54` — `POST /token {room, name, role}` endpoint spec

**Flaw:** The token service endpoint as specified accepts `role: "host"` from the client request body with no authentication. Any person who can reach the token service endpoint can claim host role. The plan's hardening note (`phase-05:62`) says "rate-limit token endpoint, validate room name, secret qua env" but does not specify how the server verifies that the caller is allowed to request the host role. If this is an open endpoint with only rate-limiting, an attacker hitting the endpoint once can obtain a host token and take over any room.

**Evidence:**
- `phase-05:54` — endpoint spec: `POST /token {room, name, role}` — no authentication parameter specified.
- `phase-05:62` — mitigations: rate-limit + validate room name + env secrets. No mention of caller identity verification before granting `role=host`.
- `plan.md:31` — Decision #5: "Host cố định 1 người/phòng" — enforcement mechanism not defined at server level.

**Suggested fix:** Token service must verify host role requests against a pre-shared secret or signed invite link. Simplest: host creates room with a server-generated host-token (server picks role, client cannot specify `role=host` directly). Guest tokens are open. Document this in the phase.

---

## Summary

| # | Finding | Severity | Cut/Fix |
|---|---------|----------|---------|
| 1 | Bot-join alternative never evaluated — entire plan may be overscoped | Critical | Add Phase 0 spike with WASAPI loopback on existing sidecar |
| 2 | `BatchedInferencePipeline` incompatible with `whisper_streaming` — dual-model RAM conflict | Critical | Validate in Phase 1 before any architecture commits |
| 3 | Monorepo refactor of Tauri app is unbounded risk with zero feasibility evidence | High | Defer or spike separately; separate web app is safer MVP cut |
| 4 | JS→Rust→Python PCM pipe not tested in Phase 1 spike — Phase 3's hardest risk is unmitigated | High | Extend Phase 1 to include IPC throughput proof-of-concept |
| 5 | Self-host LiveKit+coturn+Redis+Caddy — gold-plating ops burden for ≤8 person MVP | High | Use LiveKit Cloud free tier for MVP |
| 6 | Web Client (Phase 4) is out-of-scope for MVP — doesn't prove core value | High | Defer Phase 4 post-MVP |
| 7 | Effort estimates (24–37d) hide cascading slip risk with no re-evaluation gate after Phase 1 | High | Add GO/NO-GO gate post-Phase 1; define minimum-cut MVP |
| 8 | `whisper_streaming` install/version not validated against pinned `faster-whisper==1.2.1` | Medium | Resolve in Phase 1 spike before committing architecture |
| 9 | Token service grants `role=host` based on client-supplied request body — no caller auth | Medium | Server must assign host role, not accept it from client |

**Minimum viable cut that still proves value:** Phase 0 loopback spike (1–2d) → Phase 1 streaming ASR gate (3–5d) → Phase 2 LiveKit Cloud media (4–6d, no self-host infra) → Phase 3 host transcribe desktop-only (6–9d). Total: ~14–22d. Defer web client, monorepo, self-hosted infra.
