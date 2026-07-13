# Deploy Guide — OneScribe Web (staging .65)

Hướng dẫn build + deploy **OneScribe Web** (BE axum musl + FE Vue) lên staging box `.65` tại `scribe.inetdev.io.vn`, qua **GitLab CI/CD** (runner sẵn có) + fallback deploy thủ công. Mục đích: AI Agent / dev có full context deploy mà không cần dò lại hạ tầng.

> Deploy **installer desktop** (Windows/macOS) là tài liệu khác: [deployment-guide.md](./deployment-guide.md). File này CHỈ cho bản Web.

## Server

| Server | IP | SSH port | Vai trò | Ghi chú |
|---|---|---|---|---|
| **Staging single-box `.65`** | `103.72.98.65` | `24700` | `scribe.inetdev.io.vn` — BE axum `:8787` + FE static + MongoDB **co-located** | Box SHARED (~15 vhost + MongoDB, chỉ ~3Gi RAM free). Cùng box với cloud-panel staging. |

**Credentials SSH (root@.65):** nằm trong `all-in-one.json` (biến `ALL_IN_ONE_JSON` — xem `cloud-panel-application/docs/server-scripts/README.md` để biết path per-OS), nhóm **`"SERVER STAGING .65"`**:
- `SSH_KEY_SERVER_STAGING_CONTENT` — **nội dung PEM private key inline** (`-----BEGIN ...`). Auth thực tế = key-based. Cách dùng: ghi content ra file tạm + `chmod 600` → `ssh -i <tmp.pem> -p 24700 root@103.72.98.65`.
- `SSH_SERVER_STAGING` — lệnh `sshpass` (password có thể không auth được vì server key-only; ưu tiên key trên).

## Deploy topology

```
Cloudflare (DNS proxied, lo TLS edge)
        │  scribe.inetdev.io.vn
        ▼
   nginx :80  (/etc/nginx/conf.d/onescribe.conf — SHARED, chỉ thêm 1 server block)
        ├─ /api/  → proxy 127.0.0.1:8787  (BE axum, systemd onescribe-be)
        └─ /      → /var/www/onescribe/dist  (FE Vue SPA, try_files → index.html)
                          │
   BE ─ /opt/onescribe-be/onescribe-be  (static musl, user `onescribe`, chmod 700)
        ├─ /opt/onescribe-be/.session-key   (tự sinh runtime)
        └─ /opt/onescribe-be/data/{userId}/ (upload/recording/output)
   MongoDB co-located 127.0.0.1:27017 (db `onescribe`)
```

| Thành phần | Path trên .65 | Nguồn (repo) |
|---|---|---|
| BE binary | `/opt/onescribe-be/onescribe-be` | `scripts/build-musl.sh` → `target/x86_64-unknown-linux-musl/release/onescribe-be` |
| systemd unit | `/etc/systemd/system/onescribe-be.service` | `deploy/onescribe-be.service` |
| nginx vhost | `/etc/nginx/conf.d/onescribe.conf` | `deploy/nginx-onescribe.conf` |
| FE static | `/var/www/onescribe/dist` | `web/` → `npm run build` → `web/dist` |

## ⚠️ Ràng buộc then chốt — secret bake **compile-time**

`server/onescribe-be/src/config.rs` đọc config bằng **`option_env!`** (compile-time, KHÔNG phải runtime) + `build.rs` bake vào binary qua `cargo:rustc-env`. Hệ quả:

- **Binary KHÔNG đọc env lúc chạy** → KHÔNG dùng systemd `EnvironmentFile` để cấp `MONGO_URI`.
- **`MONGO_URI` (+ `PORT`, `ALLOWED_DOMAINS`, `ALLOWED_EMAILS`, `SESSION_KEY_PATH`, `DATA_DIR`) phải có mặt LÚC BUILD** → CI runner build binary *kèm secret* rồi mới rsync sang `.65`.
- Biến nào không set lúc build → dùng fallback trong `config.rs` (`PORT=8787`, `MONGO_URI=mongodb://127.0.0.1:27017`, allowlist rỗng = mở, `SESSION_KEY_PATH=/opt/onescribe-be/.session-key`, `DATA_DIR=/opt/onescribe-be/data`).
- **Firebase project ID** (`onescribe-inet`) là hằng công khai trong `config.rs`, KHÔNG phải secret.

> Đổi `MONGO_URI`/allowlist = **build lại binary** rồi redeploy; sửa env trên server vô tác dụng.

## CI/CD

Pipeline: `.gitlab-ci.yml` (root) + template `.gitlab/ci/templates/ssh-deploy.yml`. Project `dattqh/one-scribe` (id `331`).

### Stages & jobs

| Job | Stage | Chạy khi | Việc |
|---|---|---|---|
| `be:build` | build | mọi nhánh + MR | Build static musl binary trong image `rust-musl-cross` (bake `MONGO_URI`/`PORT`). Artifact = binary. |
| `fe:build` | build | mọi nhánh + MR | `cd web && npm ci && npm run build` (vue-tsc + vite). Artifact = `web/dist`. |
| `deploy:staging` | deploy | **chỉ `main`** | rsync binary + dist sang `.65`, thay binary bằng **atomic rename** rồi `systemctl restart onescribe-be`. |

Build chạy mọi nhánh/MR để bắt lỗi sớm; deploy chỉ `main` (tránh nhánh phụ đẩy lên staging shared).

### CI/CD Variables (Settings → CI/CD → Variables)

| Key | Giá trị / nguồn | Protected | Masked | Dùng ở |
|---|---|---|---|---|
| `SSH_PRIVATE_KEY` | `SSH_KEY_SERVER_STAGING_CONTENT` từ `all-in-one.json` (PEM inline) | ✅ | ❌ (multiline) | deploy |
| `SERVER_IP` | `103.72.98.65` | ✅ | ❌ | deploy |
| `SERVER_PORT` | `24700` | ✅ | ❌ | deploy |
| `SSH_USER` | `root` | ✅ | ❌ | deploy |
| `MONGO_URI` | Mongo URI staging trên `.65` (co-located; thường `mongodb://127.0.0.1:27017`) | ✅ | ✅ | be:build (bake) |
| `PORT` | `8787` (khớp systemd + nginx proxy) | ✅ | ❌ | be:build (bake) |

- `ALLOWED_DOMAINS` / `ALLOWED_EMAILS`: **KHÔNG set** → fallback rỗng = mở cho mọi email Google verified (quyết định staging). Muốn siết → thêm biến `ALLOWED_DOMAINS=inet.vn` rồi build lại.
- **Protected** = chỉ inject trên protected branch/tag. `main` phải là **protected branch** (mặc định GitLab) để job `main` nhận được biến. Nhánh phụ không nhận protected var → build dùng fallback (vô hại vì không deploy).

### SSH source-IP allowlist (BẮT BUỘC)

`.65` chặn SSH port `24700` theo iptables chain **`CP_SSH_ACL`** (allowlist source IP). **IP egress runner chạy `deploy:staging` PHẢI nằm trong allowlist**, nếu không `ssh-keyscan` trong template treo tới timeout rồi fail (template in rõ egress IP để whitelist).

- Runner online: **`103.57.222.245`** (id=15, đang dùng) · `202.92.5.105` (id=5, Shared-Runner đang paused).
- Thêm runner mới / CI fail ở keyscan → SSH vào `.65` thêm rule (idempotent) rồi persist:
  ```bash
  iptables -C CP_SSH_ACL -s <RUNNER_IP>/32 -j ACCEPT 2>/dev/null || \
    iptables -I CP_SSH_ACL 1 -s <RUNNER_IP>/32 -j ACCEPT
  service iptables save   # → /etc/sysconfig/iptables
  ```
- Chain này là hardening ops có chủ đích — KHÔNG gỡ; chỉ thêm IP runner khi cần.

### Image registry

Dùng Harbor pull-through mirror `harbor.000nethost.com/docker-hub/...` cho image Docker Hub (né rate-limit). BE build dùng `ghcr.io/rust-cross/rust-musl-cross:x86_64-musl` — nếu runner không reach ghcr.io → mirror image này vào Harbor rồi đổi `image:` trong `be:build`.

## Setup 1 lần trên `.65` (bootstrap — trước lần deploy CI đầu)

CI **giả định** đã có sẵn: user `onescribe`, systemd unit, nginx vhost, MongoDB, thư mục. Làm 1 lần (SSH root@.65):

```bash
# 1) User + thư mục (session key + data ghi được; ProtectSystem=full chừa /opt)
useradd --system --home /opt/onescribe-be --shell /usr/sbin/nologin onescribe || true
mkdir -p /opt/onescribe-be/data /var/www/onescribe/dist
chown -R onescribe:onescribe /opt/onescribe-be

# 2) systemd unit (từ deploy/onescribe-be.service)
#    scp deploy/onescribe-be.service → /etc/systemd/system/onescribe-be.service
systemctl daemon-reload
# (enable; --now sẽ fail tới khi có binary — deploy CI đầu tiên sẽ đưa binary + restart)
systemctl enable onescribe-be

# 3) nginx vhost (từ deploy/nginx-onescribe.conf → /etc/nginx/conf.d/onescribe.conf)
#    KIỂM TRA SSL mode Cloudflare cho zone: Flexible → block listen 80 đủ;
#    Full/Full(strict) → thêm block listen 443 ssl + origin cert (xem comment trong file).
nginx -t && systemctl reload nginx

# 4) MongoDB: đảm bảo mongod chạy 127.0.0.1:27017; nếu bật auth → tạo user/db `onescribe`
#    và cập nhật MONGO_URI (CI variable) tương ứng.
```

> Chỉ THÊM server block nginx cho OneScribe — KHÔNG sửa 14 vhost khác trên box shared. `nginx -t` trước mọi reload.

## Deploy thủ công (fallback khi CI hỏng / hotfix)

Cần Docker (build musl) + SSH key `.65`.

```bash
# 1) Build BE musl (secret truyền qua ENV, KHÔNG hardcode)
MONGO_URI="mongodb://127.0.0.1:27017" PORT=8787 ./scripts/build-musl.sh
#    → target/x86_64-unknown-linux-musl/release/onescribe-be

# 2) Build FE
cd web && npm ci && npm run build && cd ..   # → web/dist

# 3) Ghi SSH key ra file tạm (từ all-in-one.json) rồi rsync
KEY=$(mktemp); chmod 600 "$KEY"
#   (lấy SSH_KEY_SERVER_STAGING_CONTENT ghi vào $KEY)
rsync -avz -e "ssh -i $KEY -p 24700" target/x86_64-unknown-linux-musl/release/onescribe-be root@103.72.98.65:/tmp/onescribe-be.new
rsync -avz --delete -e "ssh -i $KEY -p 24700" web/dist/ root@103.72.98.65:/tmp/onescribe-fe/
ssh -i "$KEY" -p 24700 root@103.72.98.65 '
  set -e
  install -o onescribe -g onescribe -m 700 /tmp/onescribe-be.new /opt/onescribe-be/onescribe-be.new
  mv -f /opt/onescribe-be/onescribe-be.new /opt/onescribe-be/onescribe-be
  rm -f /tmp/onescribe-be.new
  rsync -a --delete /tmp/onescribe-fe/ /var/www/onescribe/dist/
  rm -rf /tmp/onescribe-fe
  systemctl restart onescribe-be && sleep 2 && systemctl is-active onescribe-be'
rm -f "$KEY"
```

> **Thay binary đang chạy:** dùng `mv` (atomic rename) — tránh `ETXTBSY` khi ghi đè executable đang chạy. rsync mặc định (không `--inplace`) cũng an toàn.

## Verify sau deploy

```bash
# Trên .65:
systemctl is-active onescribe-be          # → active
journalctl -u onescribe-be -n 30 --no-pager
curl -s http://127.0.0.1:8787/api/health  # → liveness OK

# Từ ngoài:
curl -sI https://scribe.inetdev.io.vn/            # 200, FE index
curl -s  https://scribe.inetdev.io.vn/api/health  # liveness
```

- [ ] `systemctl is-active onescribe-be` = active, không restart-loop (`systemctl status` restart count).
- [ ] `/api/health` trả OK (BE connect Mongo thành công — nếu `MONGO_URI` sai → BE fail sớm, xem journal).
- [ ] FE load, đăng nhập Google được (allowlist mở → mọi email verified).
- [ ] Ghi/cắt file → upload + phát lại (Range) hoạt động.

## Trạng thái đã xác minh (pipeline 6892 + dò `.65`, 2026-07-13)

- **Bootstrap `.65`: XONG** — user `onescribe`, systemd unit active, nginx vhost, mongod đều có sẵn. CI deploy = redeploy (idempotent).
- **Cloudflare: OK** — `https://scribe.inetdev.io.vn/api/health` → `{"status":"ok"}` với nginx block `listen 80` hiện tại (CF lo TLS edge). index.html serve `DYNAMIC` (CF không cache), asset hashed cache 4h.
- **Runner reach `ghcr.io`: OK** — `be:build` pull `rust-musl-cross` thành công (263s).
- **MongoDB `.65`:** auth ENABLED; `MONGO_URI` dùng cred `onescribe` (db + authSource = `onescribe`). **DB dùng chung cho team theo chủ đích** → cổng `27017` mở ra ngoài + mật khẩu chia sẻ là **cố ý**, KHÔNG firewall về localhost / KHÔNG rotate (team cần kết nối trực tiếp).

## Ghi chú vận hành

- **Mọi push lên `main` đều rebuild musl (~4.5') + redeploy** (chưa có `changes:` filter). Push chỉ đổi docs/config → dùng `git push -o ci.skip` để bỏ qua. Có thể thêm rule `changes: [web/**, server/**, crates/**, .gitlab-ci.yml]` nếu muốn tự lọc.
- Feature **preview/level meter khi ghi** chỉ hiện lúc đang ghi (`v-if="rec.recording.value"` trong `RecordView.vue`) — reload trang idle không thấy; phải bấm Bắt đầu ghi.
