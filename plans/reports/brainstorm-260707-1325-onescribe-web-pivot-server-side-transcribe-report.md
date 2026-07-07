# Brainstorm Report — OneScribe Web: pivot desktop offline → web app server-side

**Date:** 2026-07-07 · **Skill:** /brainstorm (no flags) · **Status:** APPROVED (user duyệt Approach A)
**Target deploy:** server staging .65 (103.72.98.65 — 4 vCPU Broadwell, 7.5GB RAM, 60GB disk, ~15 service đang chạy)

## 1. Problem statement (problem-first inversion)

**Solution đề xuất ban đầu:** "chạy app trên server linux (web app online) thay mô hình offline".
**Vấn đề thật bên dưới:** nỗi đau **phân phối cross-platform** — desktop app phải build/ký/cài riêng Windows/macOS, iPad không khả thi (sidecar Python), user tải model 490MB–1.5GB, hiệu năng phụ thuộc máy user. Evidence: **medium** (user trực tiếp trải nghiệm; Windows-first MVP vừa xong, macOS vừa thêm, iPad vừa hỏi).

**3 frames đã cân:**
- A "web hóa toàn bộ" — va giới hạn browser (WASAPI/ddagrab không tồn tại) + box yếu cho live-ASR.
- B "nỗi đau ở ASR không phải ở app" — transcribe service server-side, client mỏng.
- C "mở rộng nền meeting sẵn có" — meeting-server + apps/web + LiveKit đã deploy trên .65.
→ Chốt: B+C hội tụ, có điều chỉnh kỳ vọng parity (xem §3).

## 2. Hiện trạng đã khảo sát

**Codebase:** Tauri 2 + Vue 3; sidecar Python faster-whisper/PhoWhisper CT2 (stream vad-cut + batch); `server/meeting-server` (token/caption/TURN, 10 test); `apps/web` Vue thuần; `packages/meeting-shared`; `infra/` livekit/coturn/caddy/systemd. Plan 260626 meeting Phase 0–5 code done (host-desktop-transcribe).

**Server .65 (SSH verify 2026-07-07):** load 0.15–0.24 (CPU rảnh); RAM 4.3/7.5GB dùng, **3.2GB available, swap chớm dùng**; disk 35GB trống; `livekit.service` (OneScribe staging) ĐÃ chạy port 7880; nginx route `*.inetdev.io.vn`; app Node chạy PM2, infra systemd; OneDash backend = bundle 1 file, **không lộ contract auth**.

## 3. Quyết định đã chốt (user, session này)

| # | Quyết định | Ghi chú |
|---|---|---|
| 1 | **Web thay thế desktop**; desktop → maintenance (tag release cuối, chỉ fix bug) | Đảo Decision #4 plan meeting (host-transcribe) — có chủ đích |
| 2 | Scope hướng **full parity**, chấp nhận giới hạn vật lý browser (ma trận §4) | |
| 3 | **Model cố định, không cho chọn: small = live, medium = final/batch** | Giải mâu thuẫn "medium × realtime × không đè box" |
| 4 | Loopback web = **Chrome/Edge-only** (share system audio); Safari/Firefox báo không hỗ trợ | |
| 5 | **Audio ephemeral** — xóa ngay sau transcript; transcript có retention | Giữ một phần định vị privacy cũ |
| 6 | Auth = **OneDash/CloudPanel SSO**, nhưng pluggable (JWT nội bộ trước, cắm SSO khi có contract) | Contract chưa có — không chặn MVP |
| 7 | **Meeting GỘP vào MVP** (user override khuyến nghị "phase sau") | Sub-project 3, gate riêng, trễ thì tách |

## 4. Ma trận parity desktop → web

| Tính năng | Web | Ghi chú |
|---|---|---|
| Transcribe file (Mode C) | ✅ 100% | upload → queue → medium batch |
| Split file (Mode B) | ✅ 100% | ffmpeg server-side |
| Ghi mic | ✅ 100% | MediaRecorder |
| Ghi màn hình | ✅ ~90% | getDisplayMedia |
| Loopback hệ thống | ⚠️ ~60% | Chrome/Edge share-system-audio only |
| Live caption khi ghi | ⚠️ | small realtime + medium final (2-pass) |
| Meeting + caption | ⚠️ | bot server-side small ≤2 stream |

## 5. Approaches đã cân

- **A (CHỐT): 1 server Node + ASR worker Python, tái dùng tối đa.** Mở rộng `server/meeting-server` → onescribe-server (auth pluggable, upload, SQLite job queue, WS PCM↔caption, meeting giữ nguyên). ASR: batch = spawn `main.py` per-job (medium, 2 threads, nice, load-on-demand + idle-unload); live = `main.py --stream` persistent (small, ≤2 stream, protocol stdio y hệt đường Rust→sidecar). Meeting-bot = livekit-rtc Python join room, active-speaker gating, feed stream engine. FE = mở rộng `apps/web`, port views. ~70% tái dùng; phần mới thật: upload/queue, WS bridge, meeting-bot, port UI.
- B: Python FastAPI ASR service riêng — boundary sạch hơn nhưng +1 HTTP surface, auth trùng, PCM serialize 2 lần. Loại.
- C: 2 server Node tách — isolation hơn nhưng RAM là tài nguyên hiếm nhất, phần nặng là Python. Loại (KISS).

## 6. Ngân sách tài nguyên .65 + guardrails

Node ~150MB thường trú · medium batch ~1.5GB khi có job · small stream ~600MB khi live → **peak ~2.2GB / 3.2GB available — vừa khít**.
Guardrails BẮT BUỘC: systemd `MemoryMax` + `CPUQuota=200%` cho nhóm ASR; queue batch concurrency=1; live+meeting chung cap ≤2 stream ASR; batch nice 10.
**Khuyến nghị mạnh (không blocker):** resize VM lên 12–16GB RAM.

## 7. Build order MVP

| # | Sub-project | Tier | Gate |
|---|---|---|---|
| 0 | Benchmark RTF small/medium trên CHÍNH box .65 (`stream_asr_spike.py` sẵn có) | S | NO-GO nếu medium batch RTF >~3 |
| 1 | Nền server + batch transcribe + FE cơ bản + JWT + deploy vhost staging | M | E2E upload→transcript |
| 2 | Live caption (WS PCM→small) + screen/system-audio + medium final pass | M | latency ≈ spike Phase 1 |
| 3 | Meeting web + caption bot livekit-rtc | L | **rủi ro cao nhất, chưa spike** — gate sau #0 |
| 4 | SSO OneDash + retention purge + metrics (VictoriaMetrics sẵn có trên box) | M | |

## 8. Rủi ro & mitigation

1. RAM 3.2GB vừa khít → guardrails + đề xuất resize.
2. Meeting-bot chưa từng spike (livekit-rtc + PhoWhisper trên Broadwell) → sub-project 0 đo trước, gate trước #3.
3. SSO OneDash không có contract → auth pluggable.
4. Plan 260626 Phase 3 (host relay) thành legacy → cập nhật status plan cũ, tránh cook tiếp nhánh chết.
5. Loopback web Chrome/Edge-only → disclosure UI rõ ràng.

## 9. Success metrics

- Upload file 60' tiếng Việt → transcript medium xong, box không OOM, service khác không degrade (load/RAM đo trước-sau).
- Live caption mic: caption small ≤4s sau dứt câu, final medium tự chạy sau khi dừng.
- Meeting ≤8 người trên web + caption đúng người nói (small, ≤2 stream).
- Audio bị xóa sau transcript (kiểm disk); transcript purge đúng retention.
- Không secret trong repo; auth chặn anonymous.

## 10. Next steps

1. `/ck:plan` từ report này (plan dir mới, plan meeting cũ đánh dấu superseded phần Phase 3/4).
2. Sub-project 0 (benchmark box .65) làm NGAY đầu plan.
3. User: hỏi contract auth OneDash + khả năng resize VM.

## Unresolved questions

- Contract auth OneDash (endpoint verify/login? OIDC?) — chờ user cung cấp.
- VM .65 resize RAM được không? (ảnh hưởng độ an toàn, không ảnh hưởng thiết kế)
- Domain chính thức cho web app (`onescribe.inetdev.io.vn`?).
- Số phận chi tiết plan 260626: Phase 3 host-relay legacy hoàn toàn hay giữ làm fallback offline?
