# Deploy Guide — Scribe Web (prod .65)

Hướng dẫn build + deploy **Scribe** (web + backend dùng chung với app iPhone) (BE axum musl + FE React) lên box prod `.65`, bằng **deploy làm tay** (CI GitHub Actions chỉ kiểm, không deploy). Mục đích: AI Agent / dev có full context deploy mà không cần dò lại hạ tầng.

## Tên trên prod

Prod chạy đúng theo hai file mẫu trong `deploy/` (`scribe-be.service`, `nginx-scribe.conf`):

| Biến trong `.env.production` | Giá trị trên prod |
|---|---|
| `DEPLOY_DOMAIN` | `scribe.hoandat.com` (Cloudflare → nginx) |
| `DEPLOY_SERVICE` | `scribe-be` — systemd unit, cũng là tên file binary trong `DEPLOY_APP_DIR` |
| `DEPLOY_USER` | `scribe` — user hệ thống chạy BE |
| `DEPLOY_APP_DIR` | `/opt/scribe-be` (binary, `data/`, các tệp bí mật — xem §Tệp bí mật trên server) |
| `DEPLOY_WEB_ROOT` | `/var/www/scribe/dist` |
| `DEPLOY_NGINX_CONF` | `/etc/nginx/conf.d/scribe.conf` |

Database: `scribe` trên mongod chung `27017`, user Mongo riêng `scribe` (chỉ `readWrite` + `dbAdmin` trên db `scribe`). `.env.production` ở gốc repo (không có trong git, mỗi máy deploy tự giữ) chứa các biến `DEPLOY_*` trên cùng biến build của BE (xem bên dưới). Các lệnh dưới đây giả định đã nạp: `set -a; . ./.env.production; set +a`.

> **Đổi tên và đăng nhập ngày 2026-10-16 (đã xong):** bản cũ `onescribe-be` / `/opt/onescribe-be` / db `onescribe` / domain `scribe.inetdev.io.vn` đã xoá hẳn (service, user, thư mục, db, vhost, DNS); đăng nhập Firebase đã bỏ, thay bằng hoandat-auth. Dữ liệu trước đổi tên chỉ còn ở bản dump `/root/onescribe-backup-20261016-124301/`.

## Môi trường

Scribe web có đúng **hai** môi trường (chốt 2026-10-15; khái niệm staging đã bỏ):

| Môi trường | Ở đâu | Dùng để |
|---|---|---|
| **Dev local** | Máy dev: BE `127.0.0.1:8787` + `cd web && pnpm run dev` (`:5173`, proxy `/api` → BE) | Phát triển và kiểm chứng trước khi đẩy prod |
| **Prod** | Box `.65`, `https://scribe.hoandat.com` | Người dùng thật — mọi deploy hay đổi cấu hình ở đây là thay đổi prod |

Không dựng thêm môi trường xem trước (tunnel, preview) trên `.65`.

### Chạy dev local

- **Mongo dev:** database dev trên mongod lab **176** (`192.168.122.176:27017`, cần VPN lab), user dev chỉ có `readWrite` trên đúng database đó. 176 là máy khác với prod nên không đụng dữ liệu thật. Tạo ngày 2026-10-15; đã kiểm cách ly: user này không thấy database nào khác.
- **Biến BE dev** nằm trong `.env.development` ở gốc repo (không có trong git): `MONGO_URI`, `PORT`, `DB_NAME`, `SESSION_KEY_PATH`, `DATA_DIR`, `PUBLIC_URL`, `OIDC_ISSUER`, `OIDC_WEB_CLIENT_ID`, `OIDC_WEB_CLIENT_SECRET_PATH`, `OPENAI_KEY_PATH`, `TRANSCRIBE_EMAILS` (các đường dẫn tệp trỏ vào thư mục dev trong home, vì thư mục prod không ghi được trên macOS). BE đọc biến **lúc build** (`option_env!`), nên nạp env rồi mới build/chạy:

  ```bash
  set -a; . ./.env.development; set +a
  cargo run -p scribe-be             # → 127.0.0.1:8787, kiểm: curl 127.0.0.1:8787/api/health
  cd web && pnpm run dev              # → http://localhost:5173
  ```
- **Đăng nhập ở dev:** qua hoandat-auth với client confidential riêng `scribe-web-dev` (IdP bật bằng `SCRIBE_WEB_DEV_URL` + `SCRIBE_WEB_DEV_CLIENT_SECRET` trong `.env` của hoandat-auth). Redirect loopback phải khớp **đúng cổng**, nên `PUBLIC_URL` trong `.env.development` phải bằng origin đã khai ở IdP. Secret dev nằm trong tệp `OIDC_WEB_CLIENT_SECRET_PATH` trên máy dev (600).

## Server

| Server | IP | SSH port | Vai trò | Ghi chú |
|---|---|---|---|---|
| **Prod single-box `.65`** | `103.72.98.65` | `24700` | BE axum `:8787` + FE static + MongoDB **co-located** | Box SHARED (~15 vhost + MongoDB, chỉ ~3Gi RAM free). |

**Credentials SSH (root@.65):** trong tệp credential lab (biến môi trường `ALL_IN_ONE_STAGING_DEV`), entry `name = "SSH_SERVER_STAGING"`, trường `private_key` (PEM inline). Cách dùng: ghi ra file tạm + `chmod 600` → `ssh -i <tmp.pem> -p 24700 root@103.72.98.65`:

```bash
KEY=$(mktemp); chmod 600 "$KEY"
jq -r '.servers[]|select(.name=="SSH_SERVER_STAGING")|.private_key' "$ALL_IN_ONE_STAGING_DEV" > "$KEY"
```

## Deploy topology

```
Cloudflare (DNS proxied, SSL "Full")
        │  scribe.hoandat.com (HTTPS tới origin)
        ▼
   nginx :443 ssl  (/etc/nginx/conf.d/scribe.conf — SHARED, chỉ thêm server block của Scribe;
        │           chứng chỉ tự ký chung /etc/pki/nginx/server.{crt,key}; :80 → 301 https)
        ├─ /api/  → proxy 127.0.0.1:8787  (BE axum, systemd scribe-be)
        └─ /      → /var/www/scribe/dist  (FE React SPA, try_files → index.html)
                          │
   BE ─ /opt/scribe-be/scribe-be  (static musl, user scribe, chmod 700)
        ├─ tệp bí mật trong /opt/scribe-be (§Tệp bí mật trên server)
        └─ /opt/scribe-be/data/{userId}/   (upload/recording/output)
   MongoDB chung 27017 (database scribe)        IdP: https://auth.hoandat.com (hoandat-auth, cùng máy)
```

| Thành phần | Path trên .65 | Nguồn (repo) |
|---|---|---|
| BE binary | `/opt/scribe-be/scribe-be` | `cargo zigbuild` (xem §Deploy) → `x86_64-unknown-linux-musl/release/scribe-be` trong thư mục target |
| systemd unit | `/etc/systemd/system/scribe-be.service` | `deploy/scribe-be.service` |
| nginx vhost | `/etc/nginx/conf.d/scribe.conf` | `deploy/nginx-scribe.conf` |
| FE static | `/var/www/scribe/dist` | `web/` → `pnpm run build` → `web/dist` |

### Tệp bí mật trên server

Tất cả trong `/opt/scribe-be`, chủ `scribe`, quyền 600, ghi bằng stdin (không qua argv), không bao giờ in. Đường dẫn là biến build; nội dung đọc lúc khởi động, nên đổi tệp xong phải restart.

| Tệp | Biến build | Ghi chú |
|---|---|---|
| `.session-key` | `SESSION_KEY_PATH` | BE tự sinh nếu chưa có; khoá mã hoá refresh token IdP của thiết bị |
| `.oidc-web-secret` | `OIDC_WEB_CLIENT_SECRET_PATH` | Secret của client `scribe-web`; phía IdP là `SCRIBE_WEB_CLIENT_SECRET` trong `/var/www/hoandat-auth/.env` — đổi thì đổi cả hai |
| `.openai-key` | `OPENAI_KEY_PATH` | Key OpenAI của server (§Deploy) |
| `.notify-bot-token` | `NOTIFY_BOT_TOKEN_PATH` | Token bot báo động (§Deploy) |

## ⚠️ Ràng buộc then chốt — cấu hình bake **compile-time**

`server/scribe-be/src/config.rs` đọc config bằng **`option_env!`** (compile-time, KHÔNG phải runtime) + `build.rs` bake vào binary. Hệ quả:

- **Binary KHÔNG đọc env lúc chạy** → KHÔNG dùng systemd `EnvironmentFile`.
- **Danh sách biến là `server/scribe-be/build.rs`** (biến nào không có ở đó thì không vào binary); mặc định và ý nghĩa ở `server/scribe-be/src/config.rs`.
- **Bắt buộc lúc build:** `DB_NAME`, `PUBLIC_URL`, `SESSION_KEY_PATH`, `DATA_DIR`, `OIDC_WEB_CLIENT_SECRET_PATH`. Thiếu biến nào thì binary **dừng ngay khi khởi động** và log tên biến thiếu (để không bao giờ chạy với database hay thư mục sai).
- **Tuỳ chọn:** `MONGO_URI` (mặc định `mongodb://127.0.0.1:27017`), `PORT` (`8787`), `ALLOWED_DOMAINS`, `ALLOWED_EMAILS`, `TRANSCRIBE_EMAILS`, `TRANSCRIBE_OPEN`, các biến OIDC, `OPENAI_KEY_PATH`, các biến hạn mức chép lời và báo động (bảng dưới).

> Đổi bất kỳ biến nào = **build lại binary** rồi redeploy; sửa env trên server vô tác dụng.

### Biến build của BE

| Biến | Giá trị / nguồn |
|---|---|
| `MONGO_URI` | Mongo URI prod (user `scribe`, db `scribe`) — chỉ có trong `.env.production` |
| `PORT` | `8787` (khớp systemd + nginx proxy) |
| `DB_NAME` | `scribe` |
| `PUBLIC_URL` | `https://scribe.hoandat.com` — origin trình duyệt thấy: dựng redirect OIDC `/api/auth/callback`; `https://` thì cookie là `__Host-scribe` + `Secure` |
| `SESSION_KEY_PATH`, `DATA_DIR` | `/opt/scribe-be/.session-key`, `/opt/scribe-be/data` |
| `OIDC_WEB_CLIENT_SECRET_PATH` | `/opt/scribe-be/.oidc-web-secret` |
| `OIDC_ISSUER`, `OIDC_WEB_CLIENT_ID`, `OIDC_MOBILE_CLIENT_ID`, `OIDC_MOBILE_REDIRECT_URI` | Prod dùng mặc định: `https://auth.hoandat.com`, `scribe-web`, `scribe-mobile`, `com.hoandat.scribe:/auth/callback` |
| `OPENAI_KEY_PATH` | `/opt/scribe-be/.openai-key` |
| `TRANSCRIBE_EMAILS` | Email được dùng "Chép lời" (cách nhau dấu phẩy) khi chưa bật `TRANSCRIBE_OPEN` |
| `TRANSCRIBE_OPEN` | `1`/`true` = mọi user đăng nhập được chép lời trong hạn mức gói. **Bật trên prod từ 2026-10-16 14:19** |
| `TRANSCRIBE_FREE_MINUTES`, `TRANSCRIBE_PLUS_MINUTES`, `TRANSCRIBE_FREE_POOL_USD`, `TRANSCRIBE_MONTHLY_CAP_USD`, `TRANSCRIBE_FREE_DAILY_MINUTES`, `TRANSCRIBE_FREE_DELAY_HOURS` | Hạn mức chép lời (§Deploy); prod dùng mặc định |
| `NOTIFY_BOT_API`, `NOTIFY_BOT_TOKEN_PATH`, `NOTIFY_CHAT_ID` | Báo động qua bot (§Deploy) |

- `ALLOWED_DOMAINS` / `ALLOWED_EMAILS`: **KHÔNG set** → fallback rỗng = mở cho mọi tài khoản hoandat-auth có email đã xác minh (quyết định hiện tại). Muốn siết → thêm `ALLOWED_DOMAINS=<domain>` rồi build lại.
- `TRANSCRIBE_EMAILS`: rỗng (và chưa bật `TRANSCRIBE_OPEN`) nghĩa là **không ai** được chép lời (ngược với allowlist đăng nhập).

### Đăng nhập hoandat-auth

Hai client trên IdP `https://auth.hoandat.com` (chỉ tenant chính):

- **Web — `scribe-web` (confidential):** BE là BFF, chạy Code + PKCE phía server (`/api/auth/login` → IdP → `/api/auth/callback`), không token IdP nào tới JavaScript. Secret ở `OIDC_WEB_CLIENT_SECRET_PATH` trên server và `SCRIBE_WEB_CLIENT_SECRET` trong `/var/www/hoandat-auth/.env`. IdP gọi back-channel logout tới `/api/auth/backchannel-logout`.
- **iPhone — `scribe-mobile` (public):** xem [ios-app.md](./ios-app.md); BE đổi code ở `/api/auth/native/exchange`, làm mới ở `/api/auth/native/refresh`.
- **Hồ sơ lấy từ `userinfo`:** với code flow, ID token của hoandat-auth chỉ có `sub`; email, `email_verified`, tên lấy từ userinfo endpoint và `sub` của userinfo phải khớp ID token (`server/scribe-be/src/oidc.rs`). Email chưa xác minh thì từ chối.
- **User cũ thời Firebase** được ghép theo email đã xác minh ở lần đăng nhập IdP đầu tiên (gắn `idpSub` vào user có `firebaseUid`), sau đó khoá theo `idpSub` (`server/scribe-be/src/models/user.rs`).
- **Phiên** lưu trong Mongo, thu hồi được (`sessions`, `devices`, `auth_states`, `logout_jtis` — `server/scribe-be/src/models/session.rs`); cookie `__Host-scribe` trên https.

### FE build env

FE đọc **lúc build** từ `web/.env` (không có trong git, mẫu ở `web/.env.example`): chỉ `VITE_NATIVE_API_ORIGIN=https://scribe.hoandat.com` (origin backend mà app iPhone gọi, `web/src/lib/api.ts`; trang web dùng cùng origin nên không cần). `pnpm run ios:device` cũng nhúng giá trị này vào Info.plist của app. Thiếu biến thì `vite build` dừng và báo tên biến (`web/vite.config.ts`).

## CI và repo

- **Repo duy nhất là GitHub** (`origin` = `achoo254/scribe-next`, private). GitLab đã bỏ hoàn toàn ngày 2026-10-16.
- **CI** `.github/workflows/ci.yml` chạy mỗi lần push `main` và mỗi pull request, **chỉ kiểm, không deploy**: job `web` (`ppnpm install --frozen-lockfile`, `lint`, `typecheck`, `check:tokens`, `test`) và job `rust` (`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, có ffmpeg cho test cắt file thật).

### SSH source-IP allowlist

`.65` chặn SSH port `24700` theo iptables chain **`CP_SSH_ACL`** (allowlist source IP). Máy deploy phải có IP nằm trong allowlist. Chain này là hardening ops có chủ đích — KHÔNG gỡ; chỉ thêm IP khi cần:

```bash
iptables -C CP_SSH_ACL -s <IP>/32 -j ACCEPT 2>/dev/null || \
  iptables -I CP_SSH_ACL 1 -s <IP>/32 -j ACCEPT
service iptables save   # → /etc/sysconfig/iptables
```

## Setup 1 lần trên `.65` (bootstrap — trước lần deploy đầu)

Deploy **giả định** đã có sẵn: user, systemd unit, nginx vhost, MongoDB, thư mục. Làm 1 lần (SSH root@.65, đã có các biến `DEPLOY_*` trong shell):

```bash
# 1) User + thư mục (session key + data ghi được; ProtectSystem=full chừa /opt)
useradd --system --home "$DEPLOY_APP_DIR" --shell /usr/sbin/nologin "$DEPLOY_USER" || true
mkdir -p "$DEPLOY_APP_DIR/data" "$DEPLOY_WEB_ROOT"
chown -R "$DEPLOY_USER:$DEPLOY_USER" "$DEPLOY_APP_DIR"

# 2) systemd unit: từ mẫu deploy/scribe-be.service (thay tên/thư mục cho khớp DEPLOY_*)
#    → /etc/systemd/system/$DEPLOY_SERVICE.service
systemctl daemon-reload
systemctl enable "$DEPLOY_SERVICE"   # --now sẽ fail tới khi lần deploy đầu đưa binary lên

# 3) nginx vhost: deploy/nginx-scribe.conf → $DEPLOY_NGINX_CONF
#    (443 ssl với chứng chỉ chung /etc/pki/nginx/server.{crt,key}, zone Cloudflare SSL "Full"; 80 → 301)
nginx -t && systemctl reload nginx

# 4) MongoDB: mongod chung 27017 có auth → tạo user riêng chỉ có readWrite + dbAdmin trên $DB_NAME
#    (cần tài khoản quản trị mongo_admin_* của entry SSH_SERVER_STAGING, mật khẩu qua stdin),
#    rồi cập nhật MONGO_URI trong .env.production.

# 5) Tệp bí mật (§Tệp bí mật trên server): ghi bằng stdin, chown $DEPLOY_USER, chmod 600.
#    Client scribe-web phải có cùng secret trong /var/www/hoandat-auth/.env (SCRIBE_WEB_CLIENT_SECRET).
```

> Chỉ THÊM server block nginx cho Scribe — KHÔNG sửa 14 vhost khác trên box shared. `nginx -t` trước mọi reload.

## Deploy (làm tay)

> **Restart BE khi đang chép lời:** không mất lượt (key nằm ở server): lượt đang chạy được xếp lại và chạy từ đầu, nhưng audio gửi lại bị OpenAI tính tiền lần nữa; lượt đã bị cắt ngang một lần thì dừng hẳn. Vẫn nên chờ hàng đợi rỗng trước khi restart: đếm `db.transcripts.countDocuments({status:{$in:["queued","running"]}})` qua `scripts/prod-mongosh.sh` của skill `scribe-ops-prod`.
>
> **Key OpenAI của server** (model `gpt-4o-mini-transcribe`): tệp `OPENAI_KEY_PATH` (`/opt/scribe-be/.openai-key`), chủ `$DEPLOY_USER`, quyền 600, ghi bằng stdin, không bao giờ in. Đọc lúc khởi động; thiếu hoặc sai dạng thì chép lời tắt (journal: "không đọc được tệp key OpenAI"). OpenAI báo key hỏng/hết credit thì BE ngắt chép lời tới lần khởi động sau (audit `transcript.key_invalid`). Đổi key: ghi đè tệp rồi restart.
>
> **Gói và sổ phút** (chạy trên `.65` bằng user của service, cấu hình nhúng trong binary):
> `sudo -u $DEPLOY_USER $DEPLOY_APP_DIR/$DEPLOY_SERVICE admin set-plan <email> <free|plus> [--until YYYY-MM-DD]`;
> `… admin usage [--email <email>] [--month YYYY-MM] [--recompute [--apply]]` (đối soát sổ từ `transcribe_runs`). Hạn mức: biến build `TRANSCRIBE_FREE_MINUTES` (120), `TRANSCRIBE_PLUS_MINUTES` (3000), `TRANSCRIBE_FREE_POOL_USD` (10), `TRANSCRIBE_MONTHLY_CAP_USD` (30); `TRANSCRIBE_OPEN=1` mở chép lời cho mọi user đăng nhập (bật từ 16/10). Phút được giữ chỗ khi xếp hàng rồi quyết toán theo số giây thật sự gửi OpenAI. Mảnh hoàn toàn im lặng (đỉnh < −30 dB, đo bằng `volumedetect` trong lần nén) không gửi và không tính phút (`server/scribe-be/src/transcribe/mod.rs`).
>
> **Chống lạm dụng gói free** (chỉ áp cho gói `free`): mỗi tài khoản `TRANSCRIBE_FREE_DAILY_MINUTES` (30) phút/ngày (ngày theo giờ Việt Nam, sổ `d:<userId>:<ngày>` trong `transcribe_usage`; không giới hạn theo IP vì người dùng 4G chung IP nhà mạng), tài khoản mới chờ `TRANSCRIBE_FREE_DELAY_HOURS` (24) giờ từ `createdAt`. Lỗi trả `DAILY_LIMIT`, `ACCOUNT_TOO_NEW` kèm `retryAt`; lượt dài hơn giới hạn ngày trả `TOO_LONG_FOR_FREE` (không có `retryAt`). Worker không gửi audio của tài khoản đã bị khoá (lượt đang xếp hàng đóng, hoàn đủ phút). Khoá tài khoản: `… admin block <email> [--reason <lý do>]` (chặn đăng nhập và chép lời, xoá phiên; server đang chạy còn nhớ phiên tối đa 30 giây) / `… admin unblock <email>`.
>
> **Báo động qua bot chat.hoandat.com** (`@bot_scribe_bot`, nhắn vào chat riêng của chủ sản phẩm): biến build `NOTIFY_CHAT_ID` (chat id của chủ sản phẩm, lấy từ `GET /bot/v1/updates` sau khi bấm "Bắt đầu" với bot) + `NOTIFY_BOT_TOKEN_PATH` (`/opt/scribe-be/.notify-bot-token`, chủ `$DEPLOY_USER`, 600, ghi bằng stdin); `NOTIFY_BOT_API` mặc định `https://chat.hoandat.com/bot/v1`. Báo khi quỹ free qua 50/80/100%, trần tổng qua 80/100% (mỗi mốc một lần mỗi tháng), một tài khoản chạm giới hạn ngày lần thứ 3 trong ngày, key OpenAI hỏng. Thiếu cấu hình thì chỉ log ("không gửi được báo động"). Thử: `… admin notify-test`.
>
> **Đổi dữ liệu chép lời (`transcripts`):** chạy `scripts/transcripts-audit.js` (chỉ đọc) trước, và `mongodump --db "$DB_NAME" --collection transcripts` vào thư mục sao lưu ngay trước deploy. Khôi phục khi dữ liệu hỏng: `mongorestore --drop --nsInclude "$DB_NAME.transcripts" <thư mục sao lưu>`.

Cần `cargo-zigbuild` (build chéo musl trên macOS: `brew install zig`, `rustup target add x86_64-unknown-linux-musl`, `cargo install cargo-zigbuild --locked`), cộng SSH key `.65`.

> Build backend bằng `cargo zigbuild` (script Docker `scripts/build-musl.sh` cũ đã bỏ ngày 2026-10-16: còn đòi biến Firebase và không chuyển biến OIDC/hạn mức vào container).

```bash
set -a; . ./.env.production; set +a

# 1) Build BE musl (cấu hình lấy từ .env.production, KHÔNG hardcode)
cargo zigbuild --release --target x86_64-unknown-linux-musl -p scribe-be
#    → target/x86_64-unknown-linux-musl/release/scribe-be

# 2) Build FE
cd web && ppnpm install --frozen-lockfile && pnpm run build && cd ..   # → web/dist

# 3) Đẩy lên .65 (KEY = file tạm chứa private key, xem mục Server)
rsync -avz -e "ssh -i $KEY -p 24700" target/x86_64-unknown-linux-musl/release/scribe-be root@103.72.98.65:/tmp/scribe-be.new
rsync -avz --delete -e "ssh -i $KEY -p 24700" web/dist/ root@103.72.98.65:/tmp/scribe-fe/
ssh -i "$KEY" -p 24700 root@103.72.98.65 "
  set -e
  install -o $DEPLOY_USER -g $DEPLOY_USER -m 700 /tmp/scribe-be.new $DEPLOY_APP_DIR/$DEPLOY_SERVICE.new
  mv -f $DEPLOY_APP_DIR/$DEPLOY_SERVICE.new $DEPLOY_APP_DIR/$DEPLOY_SERVICE
  rm -f /tmp/scribe-be.new
  rsync -a --delete /tmp/scribe-fe/ $DEPLOY_WEB_ROOT/
  rm -rf /tmp/scribe-fe
  systemctl restart $DEPLOY_SERVICE && sleep 2 && systemctl is-active $DEPLOY_SERVICE"
rm -f "$KEY"
```

Skill SSH lab không có rsync: đẩy bản build FE bằng `tar -C web/dist -czf - . | lab-ssh.sh 65 "tar -xzf - -C /tmp/scribe-fe"` (cách skill `scribe-ops-prod` làm: §6 của skill). Sao lưu trước mỗi lần thay bản vào `/root/scribe-backup-<YYYYMMDD-HHMMSS>/` (binary `scribe-be.prev`, `dist/`, dump Mongo khi cần). Quay lại bản trước: `install` binary `.prev` từ thư mục sao lưu thành `$DEPLOY_APP_DIR/$DEPLOY_SERVICE.new`, `mv -f` đè lên binary rồi `systemctl restart $DEPLOY_SERVICE`.

> **Thay binary đang chạy:** dùng `mv` (atomic rename) — tránh `ETXTBSY` khi ghi đè executable đang chạy. rsync mặc định (không `--inplace`) cũng an toàn.

## Verify sau deploy

```bash
# Trên .65:
systemctl is-active "$DEPLOY_SERVICE"          # → active
journalctl -u "$DEPLOY_SERVICE" -n 30 --no-pager
curl -s http://127.0.0.1:8787/api/health       # → liveness OK

# Từ ngoài:
curl -sI "https://$DEPLOY_DOMAIN/"             # 200, FE index
curl -s  "https://$DEPLOY_DOMAIN/api/health"   # liveness
```

- [ ] Service active, không restart-loop (`systemctl status` restart count).
- [ ] `/api/health` trả OK. Thiếu biến build bắt buộc → BE dừng ngay, journal ghi "thiếu biến build bắt buộc: …".
- [ ] FE load, đăng nhập qua hoandat-auth được (`/api/auth/login` → 303 sang `auth.hoandat.com` với `client_id=scribe-web`; allowlist mở → mọi email đã xác minh).
- [ ] `/api/auth/native/config` trả client `scribe-mobile` (app iPhone đăng nhập được).
- [ ] Ghi/cắt file → upload + phát lại (Range) hoạt động.

## Trạng thái đã xác minh (2026-10-16, sau đổi tên + đăng nhập)

- **Bootstrap `.65`: XONG** — user `scribe`, unit `scribe-be`, vhost `/etc/nginx/conf.d/scribe.conf`, db `scribe` + user Mongo `scribe` đều có sẵn. Deploy = redeploy (idempotent).
- **Cloudflare:** `scribe.hoandat.com` proxied, SSL "Full" (CF↔origin là HTTPS, chứng chỉ tự ký chung của box); `http://` → 301 `https://`. Domain cũ `scribe.inetdev.io.vn` đã bỏ (vhost và DNS đã xoá, không 301).
- **IdP:** `scribe-web` và `scribe-mobile` bật trên hoandat-auth; `SCRIBE_WEB_CLIENT_SECRET` nằm trong `/var/www/hoandat-auth/.env`.
- **MongoDB `.65`:** auth ENABLED; `MONGO_URI` dùng user riêng `scribe` (database + authSource = `scribe`). **mongod dùng chung cho team theo chủ đích** → cổng `27017` mở ra ngoài + mật khẩu chia sẻ là **cố ý**, KHÔNG firewall về localhost / KHÔNG rotate (team cần kết nối trực tiếp).
- **Sao lưu:** `/root/scribe-backup-<ts>/` (mới nhất `20261016-132747`, trước khi đổi đăng nhập) và `/root/onescribe-backup-20261016-124301/` (dump db `onescribe` trước đổi tên). Không có cron backup.

## Log và audit

BE ghi hai lớp (thêm 2026-10-15):

- **Access log** ra journal (`target=access`): mỗi request `/api` một dòng `method`, `path`, `status`, `ms`, `ip` (từ `X-Real-IP` do nginx đặt), `user` (id Mongo, `-` khi chưa đăng nhập). Chunk upload/ghi thành công ở mức debug (khi ghi âm ≈1 request/giây); `/api/health` không log. Xem: `journalctl -u "$DEPLOY_SERVICE" -o cat | grep "access: request"`.
- **`audit_events`** trong Mongo (database `$DB_NAME`): một doc cho mỗi hành vi `auth.login`, `auth.login_denied`, `auth.logout`, `auth.link` (ghép user cũ theo email), `auth.backchannel_logout`, `auth.device_revoked`, `recording.start`, `recording.finish`, `upload.start`, `upload.finish`, `job.create`, `job.done`, `job.failed`, `job.download`, `file.play`, `file.download`, `file.delete`, `recordings.archive`, `transcript.create`, `transcript.rerun`, `transcript.done`, `transcript.failed`, `transcript.rename`, `transcript.edit`, `transcript.delete`, `transcript.blocked`, `transcript.cap_50`, `transcript.cap_80`, `transcript.cap_reached`, `transcript.key_invalid`, `admin.set_plan`, `admin.usage_recompute`, `admin.block`, `admin.unblock` — kèm `at`, `userId`, `ip` và chi tiết (fileId, tên, kích thước…; sự kiện transcript chỉ ghi khoá phiên, id file và số câu/độ dài tên, không ghi tiêu đề hay câu). TTL 180 ngày trên `at`; index `{userId, at}` để tra theo user. Ghi nền: Mongo lỗi chỉ log warn, không làm hỏng request.

## Ghi chú vận hành

- **Preview/level meter khi ghi** chỉ hiện lúc đang ghi (`LiveMonitor` trong `web/src/pages/record/record-page.tsx`) — reload trang idle không thấy; phải bấm Bắt đầu ghi.
