# Deployment Guide

> Hướng dẫn triển khai hạ tầng OneScribe meeting. Stack: **LiveKit SFU** (media) + **meeting-server**
> (token/caption/TURN, Phase 5) + **coturn** (TURN) + **web client** (Phase 4) + reverse-proxy TLS.
> Tất cả **NATIVE (KHÔNG Docker)**, điều phối bằng **systemd**. Local-first trên WSL Ubuntu trước, rồi remote.
> **KHÔNG commit secret** vào repo. Credential SSH + key/secret thật giữ ngoài repo (file ENV nội bộ / vault).

> **Dev nhanh (local):** `bash infra/run-local.sh` chạy LiveKit dev + meeting-server cùng lúc.

## 1. LiveKit SFU — Staging .65

Dựng LiveKit (SFU media) cho tính năng Họp, expose qua **wss** dùng nginx + wildcard cert sẵn có. Native (KHÔNG Docker).

### 1.1. Server facts
| | |
|---|---|
| Host | `103.72.98.65` · SSH port `24700` · user `root` (credential trong file ENV nội bộ — KHÔNG ở repo) |
| OS | CentOS Stream 9 · 4 core · ~7.5GB RAM (server **dùng chung** nhiều dịch vụ — chỉ thao tác additive) |
| Có sẵn | docker, node v22, **nginx (80/443)**, git |
| firewalld | **tắt** (host không chặn cổng); UDP WebRTC do **cloud security-group** quyết định |
| Domain | `*.tunnel.inetdev.io.vn` → `.65` (wildcard DNS + wildcard Let's Encrypt cert sẵn) |

### 1.2. Cài + cấu hình LiveKit
```bash
# 1. Cài binary (native)
curl -sSL https://get.livekit.io | bash      # → /usr/local/bin/livekit-server

# 2. Sinh key/secret ngẫu nhiên + config (KHÔNG dùng dev devkey/secret cho server công khai)
mkdir -p /etc/livekit
LKID="API$(openssl rand -hex 6)"; LKSIG="$(openssl rand -hex 24)"
cat > /etc/livekit/livekit.yaml <<EOF
port: 7880
bind_addresses: ["127.0.0.1"]      # signaling chỉ localhost → nginx proxy
rtc:
  tcp_port: 7881                    # fallback TCP (mở công khai)
  port_range_start: 50000
  port_range_end: 50100             # UDP WebRTC media
  use_external_ip: true             # advertise public IP cho ICE
keys:
  ${LKID}: ${LKSIG}
EOF
chmod 600 /etc/livekit/livekit.yaml   # secret chỉ ở đây, mode 600

# 3. systemd
cat > /etc/systemd/system/livekit.service <<'EOF'
[Unit]
Description=LiveKit SFU (OneScribe meeting staging)
After=network.target
[Service]
ExecStart=/usr/local/bin/livekit-server --config /etc/livekit/livekit.yaml
Restart=on-failure
LimitNOFILE=65535
[Install]
WantedBy=multi-user.target
EOF
systemctl daemon-reload && systemctl enable --now livekit
```

### 1.3. nginx wss reverse-proxy
Thêm **file riêng** (exact server_name thắng wildcard `*.tunnel`, không đụng vhost khác). `/etc/nginx/conf.d/livekit.conf`:
```nginx
server {
    listen 443 ssl;
    listen [::]:443 ssl;
    server_name livekit.tunnel.inetdev.io.vn;
    ssl_certificate     /etc/letsencrypt/live/tunnel.inetdev.io.vn/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/tunnel.inetdev.io.vn/privkey.pem;
    location / {
        proxy_pass http://127.0.0.1:7880;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_read_timeout 86400s;
        proxy_send_timeout 86400s;
    }
}
```
```bash
nginx -t && systemctl reload nginx      # CHỈ reload khi -t pass (graceful, không drop vhost khác)
```
> Lưu ý: CentOS có thể bật SELinux chặn nginx→localhost (`setsebool -P httpd_can_network_connect 1`). Trên .65 SELinux **Disabled** nên không cần. Nếu test ngay sau `reload` gặp 502 → đợi vài giây (worker cũ drain) rồi thử lại.

### 1.4. Verify
```bash
curl -s https://livekit.tunnel.inetdev.io.vn/                # → OK (200)
curl -s -o /dev/null -w '%{http_code}' .../rtc/validate      # → 401 (LiveKit trả lời)
```
WebRTC media đi **trực tiếp** tới `.65` qua UDP `50000-50100` (+ TCP `7881` fallback) — KHÔNG qua nginx. Nếu signaling OK mà không thấy/nghe nhau → mở UDP `50000-50100` ở **cloud security-group**.

### 1.5. Sinh token test
Đọc key/secret từ yaml trên server (KHÔNG in secret), mint JWT:
```bash
node -e 'const c=require("crypto"),f=require("fs");const y=f.readFileSync("/etc/livekit/livekit.yaml","utf8");const m=y.match(/keys:\s*\n\s*(\S+):\s*(\S+)/);const b=o=>Buffer.from(JSON.stringify(o)).toString("base64url");const n=Math.floor(Date.now()/1e3);const d=b({alg:"HS256",typ:"JWT"})+"."+b({iss:m[1],sub:"tester",name:"Tester",nbf:n-10,exp:n+21600,video:{room:"demo",roomJoin:true,canPublish:true,canSubscribe:true,canPublishData:true}});console.log(d+"."+c.createHmac("sha256",m[2]).update(d).digest("base64url"))'
```
Đổi `sub`/`name` cho mỗi participant. Token hết hạn 6h.

### 1.6. Test từ app
App desktop → tab **"Họp"** → URL `wss://livekit.tunnel.inetdev.io.vn` → dán token → Vào phòng. (CSP app đã cho phép `wss:`.)

### 1.7. Gỡ bỏ (revert)
```bash
systemctl disable --now livekit
rm -f /etc/nginx/conf.d/livekit.conf /etc/livekit/livekit.yaml \
      /etc/systemd/system/livekit.service /usr/local/bin/livekit-server
systemctl daemon-reload; nginx -t && systemctl reload nginx
```

### Caveats
- Server **dùng chung** (Mongo/PG/nginx/nhiều service) + RAM căng (~1.4GB free); LiveKit idle nhẹ (~50MB) nhưng để ý khi họp đông.
- SSH có **fail2ban** — mở quá nhiều phiên liên tiếp → bị chặn tạm (`banner: Not allowed at this time`), tự hết sau ~10-30 phút.
- Dev devkey/secret + WSL local: xem `infra/livekit/` (Phase 2 local). Auth thật theo role + TURN ephemeral = Phase 5.

## 2. meeting-server (token + caption store + TURN credential)

Node zero-dep. Cấp JWT LiveKit theo role (host/guest), lưu caption text TTL 1 ngày, sinh TURN credential ephemeral.

```bash
# 1. Copy repo lên server (vd /opt/onescribe). Điền env (KHÔNG commit).
sudo mkdir -p /etc/onescribe
sudo cp server/meeting-server/.env.example /etc/onescribe/meeting-server.env
sudo chmod 600 /etc/onescribe/meeting-server.env
# Sửa /etc/onescribe/meeting-server.env: LK_API_KEY/LK_API_SECRET KHỚP livekit.yaml; đặt HOST_SECRET
# ngẫu nhiên dài; LIVEKIT_URL=wss://livekit.tunnel...; TURN_SECRET khớp coturn; ALLOW_ORIGINS=domain web.

# 2. systemd (sửa WorkingDirectory trong unit cho khớp nơi clone)
sudo cp infra/systemd/meeting-server.service /etc/systemd/system/
sudo systemctl daemon-reload && sudo systemctl enable --now meeting-server
curl -s localhost:8787/healthz     # → {"ok":true}
```

- **Auth:** `/token/guest` gán cứng role=guest (client KHÔNG xin được host). `/token/host` cần `HOST_SECRET` + khóa 1-host/phòng. `/captions/:room` cần JWT hợp lệ + room khớp; chỉ host ghi.
- **Reverse-proxy:** đặt sau nginx/Caddy TLS (vd `api.tunnel.inetdev.io.vn` → `127.0.0.1:8787`). getUserMedia web cần HTTPS.
- **Webhook (tùy chọn):** trỏ LiveKit webhook → `POST /webhook` để nhả host-lock khi host rời phòng.

## 3. coturn (TURN — qua NAT/firewall doanh nghiệp)

```bash
sudo apt install coturn
sudo cp infra/coturn/turnserver.conf /etc/turnserver.conf   # sửa static-auth-secret = TURN_SECRET; realm/external-ip
sudo systemctl enable --now coturn
```
Mở UDP dải relay (`49160-49200`) + `3478` ở cloud security-group. TURN credential do meeting-server sinh (ephemeral, hết hạn ~5').

## 4. Web client (apps/web)

```bash
# Build (từ repo root; dùng node_modules root — xem apps/web/vite.config.ts)
npx vite build --config apps/web/vite.config.ts     # → apps/web/dist
# Serve tĩnh qua Caddy/nginx trên domain HTTPS (getUserMedia bắt buộc HTTPS).
# Cấu hình VITE_MEETING_SERVER=https://api.tunnel... lúc build (hoặc để mặc định localhost cho dev).
```
Xem `infra/caddy/Caddyfile` (mẫu domain `meet.` cho web + `api.` cho meeting-server + `livekit.` cho wss).

## Open questions
- Cloud security-group có mở sẵn UDP `50000-50100` (LiveKit) + `49160-49200`/`3478` (coturn) cho server không?
- meeting-server caption store hiện in-memory (mất khi restart) → cân nhắc Redis khi cần bền/multi-instance.
