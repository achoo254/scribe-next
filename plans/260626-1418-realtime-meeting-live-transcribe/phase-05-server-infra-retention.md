---
phase: 5
title: "Server Infra & Retention"
status: in-progress
priority: P2
effort: "5-7d"
dependencies: [2, 3, 4]
---

# Phase 5: Server Infra & Retention

> **Tiến độ (cook, verified trên macOS):** Code DONE (gộp token-service + caption-store thành 1 process `server/meeting-server`, KISS — plan cho phép gộp).
> - `index.mjs` + `lib/{jwt,turn,host-lock,caption-store,rate-limit,config}.mjs` (Node zero-dep). **8 test pass** (HTTP thật): **C6** role gán server + host-lock 409, **C7** guest không publishData, **C8** caption store JWT + room khớp + chỉ host ghi, TURN ephemeral, rate-limit, verify token tamper.
> - **Quyết định caption store (plan yêu cầu chốt trước):** **in-memory** (MVP single-instance local-first) — interface store-agnostic, drop-in Redis khi cần bền qua restart/multi-instance. Đánh đổi: late-joiner-qua-restart mất (chấp nhận MVP).
> - Client wiring: web guest + desktop host dùng token-service; host `pushCaption` → store (late-joiner `fetchHistory` seed).
> - Infra configs: `infra/coturn/turnserver.conf` (ephemeral), `infra/caddy/Caddyfile` (TLS + wss + web), `infra/systemd/meeting-server.service`, `infra/run-local.sh`. Docs: `system-architecture.md` (mới) + `deployment-guide.md` (§2-4).
> **CHƯA verify (cần remote thật, KHÔNG thay được bằng local):** TURN qua NAT/4G/firewall doanh nghiệp thật, egress/giờ thực đo → chi phí, HTTPS thật cho getUserMedia web.

## Overview

Hóa cứng hạ tầng để chạy thật ngoài LAN: **auth/room-token** (chặn người lạ, phân vai host vs guest), **TURN/coturn** (qua NAT/firewall), **retention transcript 1 ngày** trên server (text-only, purge khi hết hạn), và **đo băng thông egress** thật ở quy mô mục tiêu để ra chi phí vận hành.

> 🖥️ **Local-first + NATIVE (đã chốt, KHÔNG Docker):** cài toàn bộ stack **native trên WSL Ubuntu** — LiveKit (binary `get.livekit.io`), coturn/redis/caddy (`apt`), token service (process) — chạy thông end-to-end local TRƯỚC (host desktop + web client), điều phối bằng **systemd units** (hoặc tmux/script lúc dev). Chỉ lên remote/production **sau khi local chạy ổn**. WSL Ubuntu sẵn sàng + có sudo để cài gói (credential giữ ngoài repo).

## Requirements

- **Functional:**
  - Token server cấp JWT LiveKit: role **host** (1/phòng, được phép — nhưng transcribe là việc client) vs **guest** (consumer, không host).
  - TURN relay cho participant sau NAT/firewall chặt.
  - Caption text lưu tạm phía server, **TTL 1 ngày → purge tự động**; người vào trễ đọc được caption từ đầu (trong hạn).
  - Server **KHÔNG** lưu media (chỉ relay) — xác nhận bằng cấu hình + kiểm tra.
- **Non-functional:** TLS/HTTPS (bắt buộc cho getUserMedia web); ổn định khi server restart (media mất là chấp nhận; text trong TTL nên bền hợp lý — chọn cơ chế ở dưới).

## Architecture

```
[Token service nhỏ (Node/Rust)] ── cấp JWT LiveKit theo room + role (host|guest)
        │
[LiveKit server] ── SFU relay (KHÔNG record) + TURN tích hợp / coturn riêng
        │
[Caption store] ── text-only, TTL 1 ngày:
     Lựa chọn A: Redis với EXPIRE 86400 (đơn giản, bền qua restart)
     Lựa chọn B: in-memory map + TTL (mất khi restart — chấp nhận nếu chỉ phục vụ trong-họp)
        │
[Reverse proxy + TLS] (Caddy/Nginx) ── domain HTTPS cho web client + wss cho LiveKit
```

- **Retention 1 ngày = text-only**: dung hòa với quyết định "server không lưu media" — chỉ nới cho text KB-scale, KHÔNG lưu video/audio. Ghi rõ trong `system-architecture.md`.
- **Late-joiner history:** server giữ caption final trong TTL → client mới join fetch lịch sử (1 lần) rồi nghe realtime tiếp.
- **Host role:** token đánh dấu host; nhưng theo quyết định #4, transcribe vẫn do client host làm — server chỉ relay caption host publish.

## Related Code Files

- Create: `server/token-service/` (cấp JWT LiveKit theo room+role) — Node hoặc Rust nhỏ gọn.
- Create: `server/caption-store/` (Redis client + API push/fetch caption theo room, TTL 86400) hoặc tích hợp vào token-service.
- Create: `infra/coturn/turnserver.conf` + `infra/systemd/*.service` (units cho livekit/coturn/redis/token-service) hoặc `infra/run-local.sh` (dev) — KHÔNG docker-compose.
- Create: `infra/caddy/Caddyfile` (TLS + domain + wss upstream LiveKit) — chạy caddy native (apt).
- Modify: `docs/deployment-guide.md`, `docs/system-architecture.md` (kiến trúc + xác nhận no-media-storage + retention policy).
- Modify: client (desktop + web) — gọi token-service thay dev-token; fetch caption history khi join.

## Implementation Steps

0. **Local-first NATIVE trên WSL Ubuntu (KHÔNG Docker):** cài `livekit-server` (`curl -sSL https://get.livekit.io | bash`), `sudo apt install coturn redis-server caddy`, chạy token service (process); viết **systemd units** (hoặc script/tmux lúc dev) để bật cả stack. Chạy thông end-to-end local (host desktop Windows + ≥1 web client) — vào phòng, caption, retention, token role. Đây là bước phải xanh TRƯỚC khi đụng remote. Các bước 1–8 dưới làm/kiểm ngay trên stack local này.
1. Token service: endpoint `POST /token {room, name, role}` → JWT LiveKit với grant phù hợp (host: 1/phòng + cờ; guest: canPublish, không host). Chống tạo phòng/giả host trái phép.
2. coturn: cấu hình `turnserver.conf` (TURN over UDP/TCP/TLS), tích hợp credential vào LiveKit/clients; test participant sau NAT chặt (mạng 4G + firewall doanh nghiệp).
3. Caption store: chọn Redis (khuyến nghị — bền qua restart, EXPIRE đơn giản); API `push(room, caption)` + `fetchHistory(room)`; TTL 86400s; job/cơ chế purge tự động (Redis EXPIRE lo sẵn).
4. Client: host push caption final lên store song song publishData; client join gọi `fetchHistory` → render lịch sử → nghe realtime tiếp.
5. Reverse proxy + TLS (Caddy auto-HTTPS): domain cho web client + `wss` cho LiveKit; xác minh getUserMedia chạy trên HTTPS thật.
6. Xác nhận server KHÔNG lưu media: review cấu hình LiveKit (không bật egress/recording), kiểm tra disk server không phình theo thời lượng họp.
7. **Đo băng thông egress thật**: chạy phòng ở quy mô mục tiêu **≤ 8 người** (MVP đã chốt), đo egress/giờ → ngoại suy chi phí/tháng → đưa vào `deployment-guide.md` (quyết định self-host VM vs managed).
8. Hardening cơ bản: rate-limit token endpoint, validate room name, secret qua env (không commit).

## Success Criteria

- [ ] Người sau NAT/firewall doanh nghiệp + mạng 4G join được phòng nhờ TURN.
- [ ] Token phân vai đúng: guest không thể host; mỗi phòng tối đa 1 host.
- [ ] Caption text giữ đúng 1 ngày rồi purge; người vào trễ đọc được lịch sử trong hạn.
- [ ] Xác nhận server KHÔNG lưu media (disk không phình theo thời lượng họp) — bằng chứng cấu hình + đo thực tế.
- [ ] Web client chạy trên HTTPS thật, getUserMedia OK.
- [ ] Có số liệu egress/giờ thực đo + ước tính chi phí/tháng ở quy mô mục tiêu trong `deployment-guide.md`.
- [ ] Secret/credential không lộ trong repo; token endpoint có rate-limit.

## Risk Assessment

- **Test local (WSL) KHÔNG thay được test remote cho NAT/TURN/egress/HTTPS thật.** Local chỉ validate wiring/auth/config/retention. Phải còn 1 vòng remote để kiểm: TURN qua NAT/4G/firewall doanh nghiệp thật (bước 2), egress/giờ thật (bước 7), HTTPS thật cho getUserMedia web. Đừng tuyên bố "xong" chỉ vì local xanh.
- **Chi phí băng thông egress** là hóa đơn thật, có thể lớn ở quy mô đông. Giảm thiểu: đo sớm (bước 7), bật simulcast/giới hạn độ phân giải mặc định, cân nhắc managed nếu ops quá nặng.
- **TURN cấu hình sai** → một số user không kết nối được. Giảm thiểu: test đa dạng mạng (4G, văn phòng, VPN) sớm.
- **"Retention 1 ngày" mâu thuẫn cảm nhận với "server không lưu gì"** → cần truyền thông rõ: chỉ lưu **text** (KB), KHÔNG media. Ghi vào docs + privacy note.
- **Redis là thêm 1 dependency vận hành**. Giảm thiểu: nếu muốn tối giản, dùng in-memory TTL (chấp nhận mất caption khi restart server) — chốt theo nhu cầu late-joiner. **Lưu ý:** nếu chọn in-memory thì success criterion "late-joiner đọc lịch sử" KHÔNG đạt khi server restart — phải chốt Redis-vs-in-memory TRƯỚC khi bắt đầu phase, không defer.

## Red Team Hardening (Session 1)

Self-host (user giữ Decision #1) → tự chịu trách nhiệm bảo mật các điểm sau (red-team Critical):

- **[C6] Server GÁN role, KHÔNG nhận `role` từ client.** Endpoint `POST /token {room, name, role}` hiện cho client tự khai role → ai cũng xin được `role=host`. Fix: **2 luồng tách biệt** — (a) host-token chỉ cấp qua flow CÓ XÁC THỰC (pre-shared secret / signed invite link từ người tạo phòng); (b) guest endpoint mở nhưng **server hard-code `role=guest`**, bỏ qua field client. Enforce **1 host/phòng** bằng **distributed lock** (`SET room:{id}:host NX EX`), request thứ 2 trả `409`; nhả lock qua LiveKit webhook `ParticipantLeft` của host.
- **[C8] Caption store API PHẢI auth.** `push`/`fetchHistory` hiện chỉ dùng `room` name làm key, không auth → đoán/brute-force room name = đọc trộm hoặc bơm transcript giả. Fix: yêu cầu **LiveKit JWT hợp lệ** ở `Authorization` header, verify `room` claim khớp trước khi serve/accept. **Room ID là UUID ngẫu nhiên opaque**, KHÔNG dùng slug người-đọc-được (Phase 2 bước 6 sửa: tách "tên hiển thị" vs "room id").
- **[TURN] Credential ephemeral, KHÔNG tĩnh.** coturn dùng `use-auth-secret` (HMAC time-limited); token-service sinh TURN credential hết hạn ~5 phút cùng lúc cấp LiveKit JWT. Credential tĩnh lộ → TURN bị lạm dụng làm open proxy (tốn băng thông + attribution risk).
- **[Race late-joiner]** Host `await push(store)` TRƯỚC `publishData`, hoặc cho server nhận caption qua LiveKit webhook (single source of truth) — tránh người vào trễ fetch history thiếu câu vừa publish.

### Success Criteria bổ sung
- [ ] Client KHÔNG tự xin được `role=host` (server gán role; host qua flow xác thực); 2 request host đồng thời → chỉ 1 thắng (lock).
- [ ] Gọi caption store không kèm JWT hợp lệ (sai room) → bị từ chối; room id là UUID.
- [ ] TURN credential ephemeral (hết hạn), không tĩnh.
