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

Database: `scribe` trên mongod chung `27017`, user Mongo riêng `scribe` (chỉ `readWrite` + `dbAdmin` trên db `scribe`). `.env.production` chứa các biến `DEPLOY_*` trên cùng biến build của BE (xem bên dưới). Các lệnh dưới đây giả định đã nạp: `set -a; . "$(scripts/env-dir.sh)/.env.production"; set +a`.

> **Env nằm trên iCloud, không trong repo** (quy tắc bắt buộc, từ 23/10/2026). Env của Scribe nằm trên iCloud (`…/ENV production/one-scribe/`: `.env.production`, `.env.development`, `web/.env`), không bao giờ để trong repo; máy Mac và máy Windows đọc chung bản đó. `scripts/env-dir.sh` in ra thư mục này (đặt `SCRIBE_ENV_DIR` nếu để chỗ khác); Vite tìm cùng chỗ (`web/vite.config.ts`). Trên Mac, tệp iCloud bị dọn khỏi đĩa thì đọc qua SSH báo `Resource deadlock avoided`: chạy `brctl download <tệp>` rồi đọc lại.

> **Đổi tên và đăng nhập ngày 2026-10-16 (đã xong):** bản cũ `onescribe-be` / `/opt/onescribe-be` / db `onescribe` / domain `scribe.inetdev.io.vn` đã xoá hẳn (service, user, thư mục, db, vhost, DNS); đăng nhập Firebase đã bỏ, thay bằng auth-next. Dữ liệu trước đổi tên chỉ còn ở bản dump `/root/onescribe-backup-20261016-124301/`.

## Môi trường

Scribe web có đúng **hai** môi trường (chốt 2026-10-15; khái niệm staging đã bỏ):

| Môi trường | Ở đâu | Dùng để |
|---|---|---|
| **Dev local** | Máy dev: BE `127.0.0.1:8787` + `cd web && pnpm run dev` (`:5173`, proxy `/api` → BE) | Phát triển và kiểm chứng trước khi đẩy prod |
| **Prod** | Box `.65`, `https://scribe.hoandat.com` | Người dùng thật — mọi deploy hay đổi cấu hình ở đây là thay đổi prod |

Không dựng thêm môi trường xem trước (tunnel, preview) trên `.65`.

### Chạy dev local

- **Mongo dev:** database dev trên mongod lab **176** (`192.168.122.176:27017`, cần VPN lab), user dev chỉ có `readWrite` trên đúng database đó. 176 là máy khác với prod nên không đụng dữ liệu thật. Tạo ngày 2026-10-15; đã kiểm cách ly: user này không thấy database nào khác.
- **Biến BE dev** nằm trong `.env.development` ở thư mục env trên iCloud (`scripts/env-dir.sh`): `MONGO_URI`, `PORT`, `DB_NAME`, `SESSION_KEY_PATH`, `DATA_DIR`, `PUBLIC_URL`, `OIDC_ISSUER`, `OIDC_WEB_CLIENT_ID`, `OIDC_WEB_CLIENT_SECRET_PATH`, `OPENAI_KEY_PATH`, `ELEVENLABS_KEY_PATH` (máy dev không có key ElevenLabs thì đặt `TRANSCRIBE_PROVIDER=openai` để vẫn chép lời được), `TRANSCRIBE_EMAILS` (các đường dẫn tệp trỏ vào thư mục dev trong home, vì thư mục prod không ghi được trên macOS). BE đọc biến **lúc build** (`option_env!`), nên nạp env rồi mới build/chạy:

  ```bash
  set -a; . "$(scripts/env-dir.sh)/.env.development"; set +a
  cargo run -p scribe-be             # → 127.0.0.1:8787, kiểm: curl 127.0.0.1:8787/api/health
  cd web && pnpm run dev              # → http://localhost:5173
  ```
- **Đăng nhập ở dev:** qua auth-next với client confidential riêng `scribe-web-dev` (IdP bật bằng `SCRIBE_WEB_DEV_URL` + `SCRIBE_WEB_DEV_CLIENT_SECRET` trong `.env` của auth-next). Redirect loopback phải khớp **đúng cổng**, nên `PUBLIC_URL` trong `.env.development` phải bằng origin đã khai ở IdP. Secret dev nằm trong tệp `OIDC_WEB_CLIENT_SECRET_PATH` trên máy dev (600).

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
   MongoDB chung 27017 (database scribe)        IdP: https://auth.hoandat.com (auth-next, pm2 `hoandat-auth`, cùng máy)
```

| Thành phần | Path trên .65 | Nguồn (repo) |
|---|---|---|
| BE binary | `/opt/scribe-be/scribe-be` | `cargo zigbuild` (xem §Deploy) → `x86_64-unknown-linux-musl/release/scribe-be` trong thư mục target |
| systemd unit | `/etc/systemd/system/scribe-be.service` | `deploy/scribe-be.service` |
| nginx vhost | `/etc/nginx/conf.d/scribe.conf` | `deploy/nginx-scribe.conf` |
| nginx header bảo mật | `/etc/nginx/scribe/security-headers.inc` | `deploy/nginx-scribe-security-headers.inc` (§Header bảo mật) |
| nginx phần chung (`/api/`, web tĩnh, `/assets/`) | `/etc/nginx/scribe/locations.inc` | `deploy/nginx-scribe-locations.inc` |
| nginx `scribe.<domain>` của tenant sổ | `/etc/nginx/conf.d/scribe-tenants.conf` | `deploy/nginx-scribe-tenants.conf` (§nginx cho domain tenant) |
| FE static | `/var/www/scribe/dist` | `web/` → `pnpm run build` → `web/dist` |

### Tệp bí mật trên server

Tất cả trong `/opt/scribe-be`, chủ `scribe`, quyền 600, ghi bằng stdin (không qua argv), không bao giờ in. Đường dẫn là biến build; nội dung đọc lúc khởi động, nên đổi tệp xong phải restart.

| Tệp | Biến build | Ghi chú |
|---|---|---|
| `.session-key` | `SESSION_KEY_PATH` | BE tự sinh nếu chưa có; khoá mã hoá refresh token IdP của thiết bị |
| `.oidc-web-secret` | `OIDC_WEB_CLIENT_SECRET_PATH` | Secret của client `scribe-web`; phía IdP là `SCRIBE_WEB_CLIENT_SECRET` trong `/var/www/hoandat-auth/.env` — đổi thì đổi cả hai |
| `.openai-key` | `OPENAI_KEY_PATH` | Key OpenAI của server: tóm tắt, hỏi đáp, dịch; chép lời khi `TRANSCRIBE_PROVIDER=openai`, chép trực tiếp khi `REALTIME_PROVIDER=openai` (§Deploy) |
| `.elevenlabs-key` | `ELEVENLABS_KEY_PATH` | Key ElevenLabs của server: chép lời tệp và phiên iPhone (§Deploy) |
| `.user-keys-secret` | `USER_KEYS_SECRET_PATH` | 32 byte ngẫu nhiên mã hoá "Key riêng" (key API người dùng tự nhập), §Key riêng. Thiếu tệp hoặc quyền khác 600 ⟹ tính năng tắt, giao diện báo |
| `.gemini-key`, `.anthropic-key` | `GEMINI_KEY_PATH`, `ANTHROPIC_KEY_PATH` | Chỉ cần khi `LLM_PROVIDER` là `gemini` / `anthropic` (model chữ của server) |
| `.notify-bot-token` | `NOTIFY_BOT_TOKEN_PATH` | Token bot `@bot_scribe_bot`: báo động, liên kết chat và gửi biên bản (§Deploy) |
| `.scribe-web-client-key` | `SCRIBE_WEB_CLIENT_KEY_PATH` | Khoá app `scribe-web` cho tenant sổ (§Đăng nhập auth-next): sinh ở IdP bằng `node --env-file=.env dist/app-client-key.mjs scribe-web <file>` (mode 600, trong `/var/www/hoandat-auth`), chép sang đây; Scribe tự tính secret `scribe-web` của từng tenant = `base64url(HMAC-SHA256(khoá, tenantId))`. Đổi `APP_CLIENT_SECRET_KEY` ở IdP thì sinh lại tệp này. Thiếu hoặc sai dạng ⟹ chế độ tenant tắt, tenant chính không đổi |
| `tenant-sealing-keys/` (thư mục 700) | `TENANT_SEALING_KEYS_DIR` | Private key RSA-3072 niêm phong key AI của tenant, mỗi tệp `<vân tay>.pem` (600), §Key AI của tenant. Thiếu hoặc không có khoá dùng được ⟹ mọi tenant sổ "chưa bật", tenant chính không đổi |
| `.idp-internal-secret` | `IDP_INTERNAL_API_SECRET_PATH` | Mật khẩu Basic của tài khoản `scribe` ở API nội bộ của auth-next (một dòng, ≥ 32 ký tự); phía IdP là mục `scribe:<secret>` của `INTERNAL_API_CREDENTIALS` trong `/var/www/hoandat-auth/.env` — đổi thì đổi cả hai. Thiếu hoặc sai dạng ⟹ dùng tên `IDP_DISPLAY_NAME` |

## ⚠️ Ràng buộc then chốt — cấu hình bake **compile-time**

`server/scribe-be/src/config.rs` đọc config bằng **`option_env!`** (compile-time, KHÔNG phải runtime) + `build.rs` bake vào binary. Hệ quả:

- **Binary KHÔNG đọc env lúc chạy** → KHÔNG dùng systemd `EnvironmentFile`.
- **Danh sách biến là `server/scribe-be/build.rs`** (biến nào không có ở đó thì không vào binary); mặc định và ý nghĩa ở `server/scribe-be/src/config.rs`.
- **Bắt buộc lúc build:** `DB_NAME`, `PUBLIC_URL`, `SESSION_KEY_PATH`, `DATA_DIR`, `OIDC_WEB_CLIENT_SECRET_PATH`. Thiếu biến nào thì binary **dừng ngay khi khởi động** và log tên biến thiếu (để không bao giờ chạy với database hay thư mục sai).
- **Tuỳ chọn:** `MONGO_URI` (mặc định `mongodb://127.0.0.1:27017`), `PORT` (`8787`), `ALLOWED_DOMAINS`, `ALLOWED_EMAILS`, `TRANSCRIBE_EMAILS`, `TRANSCRIBE_OPEN`, các biến OIDC, `OPENAI_KEY_PATH`, `TRANSCRIBE_PROVIDER`, `REALTIME_PROVIDER`, `ELEVENLABS_KEY_PATH`, `ELEVENLABS_MONTHLY_HOURS`, các biến hạn mức chép lời, báo động, tóm tắt, hỏi đáp và liên kết chat (bảng dưới).

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
| `TRANSCRIBE_PROVIDER` | `elevenlabs` (mặc định, không cần đặt) hoặc `openai` (luồng cũ, giữ làm dự phòng một bản phát hành). Chỉ áp cho chép lời tệp/phiên |
| `REALTIME_PROVIDER` | Chép trực tiếp trên iPhone: `elevenlabs` (mặc định từ 19/10/2026, Scribe v2 Realtime 0,39 USD/giờ, token dùng một lần cấp bằng key `ELEVENLABS_KEY_PATH`) hoặc `openai` (OpenAI Realtime `gpt-4o-mini-transcribe`, giữ làm dự phòng một bản phát hành). App iPhone bản 17/10 chỉ nói được với OpenAI: cài app mới trước hoặc cùng lúc deploy backend mặc định, hoặc build tạm với `REALTIME_PROVIDER=openai`. Qua ElevenLabs, chép trực tiếp tính vào sổ giờ `ELEVENLABS_MONTHLY_HOURS` theo giờ Scribe v2 tương đương (1 giờ realtime ≈ 1,77 giờ trên sổ) và vào trần USD theo giá realtime |
| `ELEVENLABS_KEY_PATH` | `/opt/scribe-be/.elevenlabs-key`. Đọc khi `TRANSCRIBE_PROVIDER` hoặc `REALTIME_PROVIDER` là `elevenlabs`. Thiếu tệp thì chép lời (journal: "thiếu key chép lời của nhà cung cấp") và chép trực tiếp bằng ElevenLabs tắt |
| `ELEVENLABS_MONTHLY_HOURS` | Trần giờ audio ElevenLabs mỗi tháng (giờ VN), mặc định `25` (gói Starter có khoảng 27 giờ Scribe v2). Độc lập với trần USD |
| `TRANSCRIBE_EMAILS` | Email được dùng "Chép lời" (cách nhau dấu phẩy) khi chưa bật `TRANSCRIBE_OPEN` |
| `REALTIME_EMAILS` | Email được dùng "Chép trực tiếp" trên iPhone (cách nhau dấu phẩy). Rỗng = **không ai**, không theo `TRANSCRIBE_OPEN`. Phút tính chung hạn mức chép lời. **Trước lần build prod có tính năng này, chủ sản phẩm thêm email của mình vào `.env.production`** (chưa thêm thì app nhận 403, ghi âm vẫn chạy) |
| `UNVERIFIED_EMAIL_SUBS` | Tài khoản auth-next (`sub`, ví dụ `acc_…`, cách nhau dấu phẩy) được đăng nhập dù email chưa xác minh. Chỉ dùng cho tài khoản duyệt App Store (`app_review`, ADR 0010 của auth-next): theo `sub` vì email chưa xác minh thì ai cũng khai được. Rỗng = mọi email phải được Google/GitHub/Apple xác minh. Gỡ khi tắt đăng nhập duyệt |
| `TRANSCRIBE_OPEN` | `1`/`true` = mọi user đăng nhập được chép lời trong hạn mức gói. **Bật trên prod từ 2026-10-16 14:19** |
| `TRANSCRIBE_FREE_MINUTES`, `TRANSCRIBE_PLUS_MINUTES`, `TRANSCRIBE_FREE_POOL_USD`, `TRANSCRIBE_MONTHLY_CAP_USD`, `TRANSCRIBE_FREE_DAILY_MINUTES`, `TRANSCRIBE_FREE_DELAY_HOURS` | Hạn mức chép lời (§Deploy); prod dùng mặc định |
| `NOTIFY_BOT_API`, `NOTIFY_BOT_TOKEN_PATH`, `NOTIFY_CHAT_ID` | Báo động qua bot (§Deploy) |
| `LLM_PROVIDER` | Nhà cung cấp model chữ của server cho biên bản, hỏi đáp, dịch: `openai` (mặc định), `gemini` hoặc `anthropic`. Key đọc từ `OPENAI_KEY_PATH` / `GEMINI_KEY_PATH` / `ANTHROPIC_KEY_PATH`. Prod chưa đổi (OpenAI) |
| `LLM_MODEL` | Model chữ của server; mặc định theo nhà cung cấp: `gpt-4.1-mini`, `gemini-3.8-flash`, `claude-haiku-5-5`. Giá token để quy ra trần chi phí lấy từ bảng `MODELS` ở `server/scribe-be/src/summary/llm.rs` (kèm link giá, kiểm 19/10/2026); model không có trong bảng thì **không chạy**: BE dùng model mặc định của nhà cung cấp và ghi lỗi lúc khởi động ("LLM_MODEL không có trong bảng giá"), muốn dùng model khác thì thêm nó cùng giá vào bảng. Gemini chạy ở mức suy nghĩ thấp nhất và phần giữ chỗ cộng sẵn phần suy nghĩ tối đa. Giá Gemini Flash tăng gấp đôi từ 1/1/2027: cập nhật bảng trước ngày đó |
| `SUMMARY_MODEL` | Tên cũ của `LLM_MODEL`, chỉ áp khi `LLM_PROVIDER=openai` và chưa đặt `LLM_MODEL` |
| `GEMINI_KEY_PATH`, `ANTHROPIC_KEY_PATH` | `/opt/scribe-be/.gemini-key`, `/opt/scribe-be/.anthropic-key`; chỉ đọc khi `LLM_PROVIDER` tương ứng |
| `USER_KEYS_SECRET_PATH` | `/opt/scribe-be/.user-keys-secret` (§Key riêng). Không đặt = tính năng Key riêng tắt |
| `SUMMARY_FREE_DAILY`, `SUMMARY_PAID_DAILY`, `SUMMARY_FREE_MONTHLY_MINUTES`, `SUMMARY_PAID_MONTHLY_MINUTES` | Hạn mức tóm tắt (§Deploy); prod dùng mặc định |
| `ASK_FREE_DAILY`, `ASK_PAID_DAILY` | Số lượt hỏi đáp mỗi ngày, mặc định 30 / 200 (§Deploy); prod dùng mặc định. Hỏi đáp dùng model `SUMMARY_MODEL` và trần chi phí tháng của tóm tắt, không có biến trần riêng |
| `TRANSLATE_FREE_DAILY`, `TRANSLATE_PAID_DAILY` | Số lượt dịch mỗi ngày, mặc định 3 / 20 (§Deploy); prod dùng mặc định. Dịch dùng model `SUMMARY_MODEL` và trần chi phí tháng của tóm tắt, không có biến trần riêng |
| `CHAT_LINK_POLL` | `1`/`true` = chạy task long-poll của bot để liên kết chat. **Chỉ bật trên prod**, không bao giờ ở dev (§Deploy) |
| `CHAT_BOT_USERNAME` | Tên bot trong deep link; prod để trống, BE hỏi `/bot/v1/me` |
| `IDP_INTERNAL_API_URL` | API nội bộ của auth-next trên cùng máy, ví dụ `http://127.0.0.1:4501`. Cùng `IDP_INTERNAL_API_SECRET_PATH` (`/opt/scribe-be/.idp-internal-secret`): lúc khởi động rồi mỗi giờ BE gọi `GET {url}/internal/tenants/lookup?host=<host của PUBLIC_URL>` (Basic `scribe`, quá giờ 5 giây; lỗi thì thử lại sau 5 phút) để lấy tên hiển thị của tenant (`TENANT_DISPLAY_NAME` của IdP) và trả công khai ở `GET /api/idp`; giao diện gọi tài khoản đăng nhập bằng tên này ("Đăng nhập bằng …", "Tài khoản …", trang pháp lý). Lỗi thì giữ tên đúng gần nhất, journal ghi cảnh báo (không chứa bí mật). Request tới host `scribe.<domain>` khác cũng tra tenant qua route này (cache 60 giây; [ADR 0001](./decisions/0001-tenants-by-domain.md)). Thiếu một trong hai biến = tắt: tên mặc định, chỉ phục vụ tenant chính |
| `IDP_DISPLAY_NAME` | Tên dùng tới khi hỏi được IdP (hoặc khi tính năng tắt), mặc định `Auth Next` (mặc định của auth-next); web cũng hiện `Auth Next` tới khi `/api/idp` trả lời |
| `IDP_LOOPBACK_URL` | Listener công khai của auth-next trên loopback, prod `http://127.0.0.1:4500` (pm2 `hoandat-auth`). Token, userinfo, thu hồi của tenant sổ gửi tới đây với `Host: auth.<domain>`, `X-Forwarded-Proto: https` (không qua DNS của domain tenant). Cùng `SCRIBE_WEB_CLIENT_KEY_PATH` và hai biến API nội bộ ở trên: đủ cả ⟹ đăng nhập được ở tenant sổ (chế độ tenant); thiếu một ⟹ journal `chế độ tenant tắt (thiếu …)`, mọi `scribe.<domain>` khác hiện "Scribe chưa bật", tenant chính không đổi |
| `SCRIBE_WEB_CLIENT_KEY_PATH` | `/opt/scribe-be/.scribe-web-client-key` (§Tệp bí mật trên server) |
| `TENANT_SEALING_KEYS_DIR` | `/opt/scribe-be/tenant-sealing-keys` (§Key AI của tenant). Cần thêm API nội bộ; thiếu ⟹ journal `key AI của tenant tắt (…)`, mọi tenant sổ hiện "Scribe chưa bật". Đủ ⟹ journal `key AI của tenant bật` kèm vân tay các khoá |

- `ALLOWED_DOMAINS` / `ALLOWED_EMAILS`: **KHÔNG set** → fallback rỗng = mở cho mọi tài khoản auth-next có email đã xác minh (quyết định hiện tại). Muốn siết → thêm `ALLOWED_DOMAINS=<domain>` rồi build lại.
- `TRANSCRIBE_EMAILS`: rỗng (và chưa bật `TRANSCRIBE_OPEN`) nghĩa là **không ai** được chép lời (ngược với allowlist đăng nhập).
- Mốc gửi nguyên bản ghi cho hỏi đáp **không phải biến build**: hằng trong `full_context_chars` (`server/scribe-be/src/search/ask.rs`), gói free dưới 30.000 ký tự, gói trả phí dưới 120.000 ký tự; dài hơn thì chỉ gửi các câu liên quan. Đổi mốc là sửa code; lý do chọn mốc ghi ngay trên hàm.

### Đăng nhập auth-next

Mỗi `scribe.<domain>` đăng nhập bằng IdP của chính domain đó (`https://auth.<domain>`); tenant chính `hoandat.com` như dưới đây.

**Tenant sổ** ([ADR 0001](./decisions/0001-tenants-by-domain.md)): một client `scribe-web` (confidential, back-channel logout, không app di động, không dev) do auth-next dựng khi bật `SCRIBE_TENANTS` và chủ domain đã nhập key chép lời. Discovery và JWKS đọc qua API nội bộ (`/internal/tenants/<id>/openid-configuration`, `/jwks`, kiểm `issuer`), token/userinfo qua `IDP_LOOPBACK_URL`; secret tính từ khoá app `SCRIBE_WEB_CLIENT_KEY_PATH`. Redirect `https://scribe.<domain>/api/auth/callback`, back-channel `/api/auth/backchannel-logout` (chỉ xoá phiên của tenant đó), sau đăng xuất về `https://scribe.<domain>/`. `/api/auth/native/*` ở tenant sổ trả 404. Bật thật sự còn cần khoá niêm phong key AI phía Scribe (§Key AI của tenant); chưa có thì mọi tenant sổ hiện "Scribe chưa bật".

Hai client trên IdP `https://auth.hoandat.com` (tenant chính):

- **Web — `scribe-web` (confidential):** BE là BFF, chạy Code + PKCE phía server (`/api/auth/login` → IdP → `/api/auth/callback`), không token IdP nào tới JavaScript. Secret ở `OIDC_WEB_CLIENT_SECRET_PATH` trên server và `SCRIBE_WEB_CLIENT_SECRET` trong `/var/www/hoandat-auth/.env`. IdP gọi back-channel logout tới `/api/auth/backchannel-logout`.
- **iPhone — `scribe-mobile` (public):** xem [ios-app.md](./ios-app.md); BE đổi code ở `/api/auth/native/exchange`, làm mới ở `/api/auth/native/refresh`.
- **Hồ sơ lấy từ `userinfo`:** với code flow, ID token của auth-next chỉ có `sub`; email, `email_verified`, tên lấy từ userinfo endpoint và `sub` của userinfo phải khớp ID token (`server/scribe-be/src/oidc.rs`). Email chưa xác minh thì từ chối.
- **User cũ thời Firebase** được ghép theo email đã xác minh ở lần đăng nhập IdP đầu tiên (gắn `idpSub` vào user có `firebaseUid`), sau đó khoá theo `idpSub` (`server/scribe-be/src/models/user.rs`).
- **Phiên** lưu trong Mongo, thu hồi được (`sessions`, `devices`, `auth_states`, `logout_jtis` — `server/scribe-be/src/models/session.rs`); cookie `__Host-scribe` trên https.

### FE build env

FE đọc **lúc build** từ `web/.env` trong thư mục env trên iCloud (mẫu ở `web/.env.example`): chỉ `VITE_NATIVE_API_ORIGIN=https://scribe.hoandat.com` (origin backend mà app iPhone gọi, `web/src/lib/api.ts`; trang web dùng cùng origin nên không cần). `pnpm run ios:device` cũng nhúng giá trị này vào Info.plist của app. Thiếu biến thì `vite build` dừng và báo tên biến (`web/vite.config.ts`).

Tuỳ chọn: `VITE_SUPPORT_EMAIL` — email hỗ trợ ở mục "Liên hệ" của `/privacy` và `/terms` (App Store cần một cách liên hệ thật); bỏ trống thì trang ghi "người vận hành Scribe qua tài khoản hoandat". Có giá trị mà không phải email thì build dừng.

Tuỳ chọn: `VITE_IDP_ORIGIN` — origin auth-next cho link "Tài khoản hoandat" và "Ứng dụng" ở account menu và footer; bỏ trống thì dùng `https://auth.hoandat.com` (`web/src/lib/idp.ts`). Có giá trị mà không phải origin http(s) thì build dừng (`web/vite.config.ts`).

## CI và repo

- **Repo duy nhất là GitHub** (`origin` = `achoo254/scribe-next`, private). GitLab đã bỏ hoàn toàn ngày 2026-10-16.
- **CI** `.github/workflows/ci.yml` chạy mỗi lần push `main` và mỗi pull request, **chỉ kiểm, không deploy**: job `web` (`pnpm install --frozen-lockfile`, `lint` gồm `check:brand` và `check:tokens`, `typecheck`, `test`, `vite build`, `check:dist`), job `e2e` (Playwright chromium + webkit, cache trình duyệt theo phiên bản: `test:e2e --ignore-snapshots` vì ảnh mốc chỉ có bản macOS, rồi `vite build` + `test:e2e:csp`, §Header bảo mật) và job `rust` (`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, có ffmpeg cho test cắt file thật). CI build với `VITE_NATIVE_API_ORIGIN=https://scribe.hoandat.com` khai trong workflow.

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
> **Key OpenAI của server**: tệp `OPENAI_KEY_PATH` (`/opt/scribe-be/.openai-key`), chủ `$DEPLOY_USER`, quyền 600, ghi bằng stdin, không bao giờ in. Đọc lúc khởi động; dùng cho tóm tắt, hỏi đáp, dịch, chép lời khi `TRANSCRIBE_PROVIDER=openai` và chép trực tiếp khi `REALTIME_PROVIDER=openai`. Thiếu hoặc sai dạng thì các tính năng đó tắt (journal: "không đọc được tệp key OpenAI"). Key hỏng ở lượt chép lời bằng OpenAI thì BE ngắt chép lời tới lần khởi động sau (audit `transcript.key_invalid`). Đổi key: ghi đè tệp rồi restart.
>
> **Key ElevenLabs của server** (chép lời mặc định, Scribe v2, từ 17/10/2026): tệp `ELEVENLABS_KEY_PATH` (`/opt/scribe-be/.elevenlabs-key`), chủ `$DEPLOY_USER`, quyền 600. Cài bằng stdin, không qua argv, không in, ví dụ dán key vào lệnh `read -rs` rồi:
> `printf '%s' "$KEY" | ssh <server> "sudo install -m 600 -o $DEPLOY_USER -g $DEPLOY_USER /dev/stdin /opt/scribe-be/.elevenlabs-key"`, sau đó `unset KEY` và restart BE. Key cần quyền Speech to Text; dùng thương mại phải là gói trả phí (gói Free không cho dùng thương mại).
> Một tệp (bản ghi trình duyệt, tệp tải lên) = một request; **phiên iPhone = một request cho cả phiên**: ffmpeg ghép các đoạn theo thứ tự rồi chia câu về lại từng đoạn, nhãn người nói nhất quán cả phiên. Audio gửi đi là Opus mono 16 kHz 32 kbps (khoảng 14 MB mỗi giờ); tệp hay phiên hoàn toàn im lặng (đỉnh < −30 dB) không gửi, không tính phút. Đoạn tải lên trong lúc lượt đang hoãn (phiên còn đoạn đang tải) được thêm vào cùng lượt; đoạn tải lên muộn hơn (sau khi phiên đã chép) làm lần bấm chép tiếp theo **chép lại cả phiên** và tính phút cả phiên lần nữa (web hỏi trước nếu mất câu đã sửa hay tên người nói). ElevenLabs từ chối chắc chắn thì hoàn cả phút người dùng, trần USD và giờ ElevenLabs; chỉ hết thời gian chờ hay mất kết nối sau khi gửi mới giữ nguyên (có thể đã bị tính). Deploy khi hàng đợi chép lời rỗng. ElevenLabs báo key hỏng (401/403) ⟹ ngắt chép lời tới lần khởi động sau, audit `transcript.key_invalid`, báo bot; tóm tắt không bị ảnh hưởng. ElevenLabs báo hết credit tháng (402 `insufficient_credits`, hoặc 401 `quota_exceeded`) ⟹ **không** coi là key hỏng: đánh dấu tháng, dừng gửi tới đầu tháng sau, audit `transcript.provider_quota`, báo bot một lần; nạp thêm credit rồi `… admin elevenlabs-quota --clear` để mở lại ngay. Trần giờ riêng `ELEVENLABS_MONTHLY_HOURS` (sổ `el:<tháng>` trong `transcribe_usage`) giữ chỗ trước khi gửi và quyết toán theo `audio_duration_secs` ElevenLabs trả; hết trần thì người dùng thấy "Đã dùng hết giờ chép lời của tháng này, mở lại ngày 1/MM" (403 `PROVIDER_HOURS`). Xem sổ: `… admin usage` (mục `elevenlabs`) hoặc `… admin elevenlabs-quota [--month YYYY-MM]`.
>
> **Key riêng** (key API người dùng tự nhập: ElevenLabs, OpenAI, Gemini, Claude; từ 19/10/2026). Key lưu trong collection `user_keys`, mã hoá AES-256-GCM bằng bí mật 32 byte ở tệp `USER_KEYS_SECRET_PATH`. Tạo tệp **một lần**, trên server, bí mật không bao giờ rời máy, không in ra:
> `ssh -i "$KEY" -p 24700 root@103.72.98.65 'test -e /opt/scribe-be/.user-keys-secret || head -c 32 /dev/urandom | install -m 600 -o scribe -g scribe /dev/stdin /opt/scribe-be/.user-keys-secret'`
> (đăng nhập root như các bước deploy — §Server; lệnh trong nháy đơn chạy hết trên server nên không phụ thuộc biến của máy mình hay sudo; `test -e` để không bao giờ ghi đè bí mật đang dùng; `scribe` là user của service; BE từ chối tệp không đúng 32 byte hoặc quyền rộng hơn 600). Thêm `USER_KEYS_SECRET_PATH=/opt/scribe-be/.user-keys-secret` vào `.env.production`, build và restart; journal có "Key riêng bật".
> **Sao lưu:** sao lưu tệp này riêng với bản sao lưu Mongo (không để chung một chỗ). Mất hoặc đổi tệp ⟹ mọi key đã lưu không giải mã được nữa (trạng thái "unreadable"), người dùng phải nhập lại; không có cách khôi phục. Đừng tạo lại tệp khi tệp cũ còn.
> Việc chạy bằng key riêng không tính phút, lượt ngày, quỹ free, trần tổng hay sổ giờ ElevenLabs (sổ chi tiết `transcribe_runs`, `summary_runs`/`ask_runs`/`translate_runs` có `byok`, `admin usage --recompute` bỏ qua; số giây chép bằng key riêng ở sổ `b:<userId>:<tháng>` chỉ để người dùng xem). Key riêng lỗi lúc dùng chỉ đánh dấu key đó (audit `user_key.failed`), không ngắt chép lời hay tóm tắt của server. Audit `user_key.save|delete|rejected|prefs` không có key.
>
> **Key AI của tenant** ([ADR 0001](./decisions/0001-tenants-by-domain.md) §6, auth-next ADR 0012). Chủ domain nhập key AI ở `https://auth.<domain>/admin`; IdP niêm phong key bằng **public key** của Scribe (RSA-OAEP SHA-256 bọc khoá AES-256-GCM) và chỉ trả bản niêm phong cho tài khoản `scribe` qua `GET /internal/tenants/<id>/scribe-keys`. **Private key** chỉ nằm ở Scribe, trong `TENANT_SEALING_KEYS_DIR` (thư mục 700, mỗi tệp `<vân tay>.pem` PKCS#8, 600; vân tay = 16 ký tự hex đầu SHA-256 của SPKI DER, cùng cách IdP tính). Sinh khoá **một lần**, trên server (đăng nhập root như §Server; lệnh chạy hết trên server, không in khoá):
> ```bash
> ssh -i "$KEY" -p 24700 root@103.72.98.65 'set -eu; umask 077
>   D=/opt/scribe-be/tenant-sealing-keys; install -d -m 700 -o scribe -g scribe "$D"
>   T=$(mktemp); openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:3072 -out "$T"
>   R=$(openssl pkey -in "$T" -pubout -outform DER | sha256sum | cut -c1-16)
>   install -m 600 -o scribe -g scribe "$T" "$D/$R.pem"
>   openssl pkey -in "$T" -pubout -out /var/www/hoandat-auth/secrets/scribe-sealing.pub.pem
>   chmod 644 /var/www/hoandat-auth/secrets/scribe-sealing.pub.pem; shred -u "$T"; echo "recipient $R"'
> ```
> Thêm `TENANT_SEALING_KEYS_DIR=/opt/scribe-be/tenant-sealing-keys` vào `.env.production`, build và restart Scribe; journal có `key AI của tenant bật` với đúng vân tay. Phía IdP: `SCRIBE_SEALING_PUBLIC_KEY_PATH=/var/www/hoandat-auth/secrets/scribe-sealing.pub.pem` cùng `SCRIBE_TENANTS=1` trong `/var/www/hoandat-auth/.env`, restart IdP (IdP từ chối private key, khoá dưới 3072 bit; xem `auth-next/docs/deployment-guide.md`). Public key không bí mật.
> **Sao lưu:** chép tệp `<vân tay>.pem` (qua `ssh … 'cat /opt/scribe-be/tenant-sealing-keys/<vân tay>.pem'`, không in ra màn hình) vào iCloud `ENV production/one-scribe/server/`, tách khỏi bản sao lưu Mongo. Mất private key ⟹ mọi key AI đã niêm phong cho nó không mở được: Scribe **không** báo `unreadable` về IdP (`/admin` không tự hiện gì), chỉ log lỗi và báo bot người vận hành một lần mỗi vân tay; người vận hành khôi phục tệp từ bản sao lưu, không thì báo từng chủ domain nhập lại key.
> **Xoay khoá theo kế hoạch:** sinh khoá mới vào cùng thư mục (lệnh trên, nhưng ghi public key ra tệp mới `/var/www/hoandat-auth/scribe-sealing-public-<vân tay>.pem` thay vì đè tệp đang dùng), restart Scribe (journal liệt kê cả hai vân tay); trỏ `SCRIBE_SEALING_PUBLIC_KEY_PATH` sang public key mới, restart IdP. Key cũ vẫn mở bằng khoá cũ (chọn theo `recipient`); chủ domain lưu lại key thì doc mang vân tay mới. Chỉ gỡ khoá cũ khi không còn doc nào mang vân tay cũ (đếm, chỉ đọc, trong DB của IdP: `db.tenant_secrets.countDocuments({ _id: /:scribe-/, "sealed.recipient": "<vân tay cũ>" })`). **Khi lộ khoá:** coi mọi key AI niêm phong cho khoá đó là đã lộ. Trước khi gỡ, lấy danh sách tenant bị ảnh hưởng (chỉ đọc, DB của IdP: `db.tenant_secrets.find({ _id: /:scribe-/, "sealed.recipient": "<vân tay lộ>" }, { _id: 1 })`); gỡ ngay tệp khoá đó và restart Scribe. Scribe không báo `unreadable` cho bản niêm phong mà nó không có khoá, nên `/admin` **không** tự hiện "cần nhập lại": người vận hành phải tự báo từng chủ domain trong danh sách (qua kênh khác ngoài `/admin`) thu hồi key ở nhà cung cấp và nhập key mới. Trong lúc đó lượt dùng key cũ báo "Key … của domain không dùng được" và bot báo người vận hành một lần mỗi vân tay.
> Chọn key mỗi việc: key riêng của người dùng → key của chủ domain → key server (**chỉ tenant chính**); tenant sổ không có key cho việc đó ⟹ 403 `TENANT_KEY_MISSING`. Lượt bằng key chủ domain không tính phút, lượt ngày, quỹ free, trần tổng hay sổ giờ ElevenLabs (sổ chi tiết có `byok` + `keySource: "tenant"`, `transcribe_runs` có thêm `tenantId`); hạn mức dung lượng gói free vẫn áp. Bản niêm phong cache 60 giây (quá hạn thì dùng bản cũ ngay và hỏi lại IdP nền; IdP lỗi thì giữ bản cũ tối đa 10 phút), bản rõ chỉ mở lúc dùng và không bao giờ vào log. Key chủ domain lỗi (401/403, hết credit, có khoá mà không mở được) ⟹ Scribe báo IdP `POST …/scribe-keys/<vendor>/status` kèm `recipient` và `tag` (thử lại nền 3 lần nếu IdP chưa nhận; vẫn không nhận thì journal lỗi kèm tenant và nhà cung cấp, lần lỗi sau báo lại), audit `tenant_key.failed`, người dùng thấy "Key … của domain không dùng được — chủ domain sửa ở trang quản trị" (403 `TENANT_KEY_FAILED` với model chữ); không bao giờ ngắt chép lời hay tóm tắt của server và không báo bot. Ngoại lệ duy nhất có bot: bản niêm phong cho khoá Scribe không có (ở trên). Quay lại: gỡ `TENANT_SEALING_KEYS_DIR` khỏi `.env.production`, build, restart ⟹ mọi tenant sổ về "chưa bật"; lượt đang xếp hàng bằng key chủ domain dừng với câu trên, không tốn tiền server.
>
> **Gói và sổ phút** (chạy trên `.65` bằng user của service, cấu hình nhúng trong binary):
> `sudo -u $DEPLOY_USER $DEPLOY_APP_DIR/$DEPLOY_SERVICE admin set-plan <email> <free|plus> [--until YYYY-MM-DD]`;
> `… admin shorten-free-retention [--apply]`: tệp của user không có gói plus hiệu lực về `expiresAt = min(expiresAt, createdAt + 7 ngày)`. Không `--apply` thì chỉ in số tệp, số user và số tệp sẽ bị sweep xoá ở vòng kế tiếp (tối đa một giờ); sao lưu trước khi `--apply` (audit `admin.shorten_free_retention`).
> `… admin usage [--email <email>] [--month YYYY-MM] [--recompute [--apply]]` (đối soát sổ từ `transcribe_runs`). Hạn mức: biến build `TRANSCRIBE_FREE_MINUTES` (120), `TRANSCRIBE_PLUS_MINUTES` (3000), `TRANSCRIBE_FREE_POOL_USD` (10), `TRANSCRIBE_MONTHLY_CAP_USD` (30); `TRANSCRIBE_OPEN=1` mở chép lời cho mọi user đăng nhập (bật từ 16/10). Phút được giữ chỗ khi xếp hàng rồi quyết toán theo số giây thật sự gửi nhà cung cấp (ElevenLabs: theo `audio_duration_secs`). Trần USD đổi ra giây theo giá của nhà cung cấp (ElevenLabs 0,22 USD/giờ, OpenAI 0,003 USD/phút), nên với ElevenLabs trần $30 ≈ 8.182 phút, quỹ free $10 ≈ 2.727 phút. Mảnh hoàn toàn im lặng (đỉnh < −30 dB, đo bằng `volumedetect` trong lần nén) không gửi và không tính phút (`server/scribe-be/src/transcribe/mod.rs`).
>
> **Dung lượng theo gói** (hằng trong `server/scribe-be/src/config.rs`, không phải biến build): free 200 MB tổng, mỗi tệp tối đa 200 MB, tệp mới tự xoá sau 7 ngày; plus 5 GB, 2 GB, 14 ngày. Hạn tệp tính lúc tạo theo gói của chủ file. User đang vượt 200 MB giữ tệp, chỉ bị chặn ghi và tải mới (413). `DELETE /api/files` ("Xoá tất cả") xoá mọi bản ghi và tệp tải lên của user cùng bản chép lời, biên bản, dấu mốc và câu trong index.

> **Chống lạm dụng gói free** (chỉ áp cho gói `free`): mỗi tài khoản `TRANSCRIBE_FREE_DAILY_MINUTES` (30) phút/ngày (ngày theo giờ Việt Nam, sổ `d:<userId>:<ngày>` trong `transcribe_usage`; không giới hạn theo IP vì người dùng 4G chung IP nhà mạng), thời gian chờ của tài khoản mới `TRANSCRIBE_FREE_DELAY_HOURS` mặc định 0 (không chờ; chủ dự án bỏ ngày 19/10/2026, prod chạy `0` từ 08:33 hôm đó), đặt số giờ > 0 thì tài khoản mới phải chờ chừng đó từ `createdAt`. Lỗi trả `DAILY_LIMIT`, `ACCOUNT_TOO_NEW` (chỉ khi thời gian chờ > 0) kèm `retryAt`; lượt dài hơn giới hạn ngày trả `TOO_LONG_FOR_FREE` (không có `retryAt`). Worker không gửi audio của tài khoản đã bị khoá (lượt đang xếp hàng đóng, hoàn đủ phút). Khoá tài khoản: `… admin block <email> [--reason <lý do>]` (chặn đăng nhập và chép lời, xoá phiên; server đang chạy còn nhớ phiên tối đa 30 giây) / `… admin unblock <email>`.
>
> **Báo động qua bot chat.hoandat.com** (`@bot_scribe_bot`, nhắn vào chat riêng của chủ sản phẩm): biến build `NOTIFY_CHAT_ID` (chat id của chủ sản phẩm, lấy từ `GET /bot/v1/updates` sau khi bấm "Bắt đầu" với bot) + `NOTIFY_BOT_TOKEN_PATH` (`/opt/scribe-be/.notify-bot-token`, chủ `$DEPLOY_USER`, 600, ghi bằng stdin); `NOTIFY_BOT_API` mặc định `https://chat.hoandat.com/bot/v1`. Báo khi quỹ free qua 50/80/100%, trần tổng qua 80/100% (mỗi mốc một lần mỗi tháng), một tài khoản chạm giới hạn ngày lần thứ 3 trong ngày, key OpenAI hỏng. Thiếu cấu hình thì chỉ log ("không gửi được báo động"). Thử: `… admin notify-test`.
>
> **Biên bản (tóm tắt) và liên kết chat** (gói "Sau cuộc họp", lên prod 16/10/2026 22:31): biên bản chỉ viết khi người dùng bấm "Tóm tắt", dùng key OpenAI của server. Mỗi tài khoản có số lượt mỗi ngày giờ VN `SUMMARY_FREE_DAILY` (10) / `SUMMARY_PAID_DAILY` (50) và trần chi phí tháng `SUMMARY_FREE_MONTHLY_MINUTES` (60) / `SUMMARY_PAID_MONTHLY_MINUTES` (600) tính bằng phút chép lời tương đương (sổ `summary_usage`). Chi phí token quy ra giây chép lời và **cộng vào trần tổng tháng** (`TRANSCRIBE_MONTHLY_CAP_USD`), không vào sổ phút của user hay quỹ free, nên cảnh báo 80/100% và chặn ở 100% tính cả tóm tắt. Key hỏng ở lượt tóm tắt chỉ **tạm ngưng tóm tắt** (cờ riêng, audit `summary.key_invalid`, báo qua bot một lần) tới lần khởi động sau; chép lời vẫn chạy. `admin usage --recompute` cộng cả `summary_runs.costS` vào trần tổng, và `--apply` từ chối khi còn lượt tóm tắt đang chạy (chờ vài phút rồi chạy lại). `admin block` gỡ luôn liên kết chat của tài khoản.
>
> Liên kết chat dùng chung token bot ở `NOTIFY_BOT_TOKEN_PATH`, không thêm endpoint public: BE long-poll `GET {NOTIFY_BOT_API}/updates`. Mỗi bot chỉ có một poll, poll mới làm poll cũ nhận 409, mà dev và prod dùng chung token: **chỉ prod build với `CHAT_LINK_POLL=1`**; backend dev bật cờ này sẽ lấy mất `/start` và nút xác nhận của người dùng prod. Tắt cờ thì không tạo được liên kết mới (`POST /api/chat/link` trả 503 `CHAT_LINK_DISABLED`, web ẩn mục liên kết với người chưa liên kết); liên kết đã có vẫn gửi biên bản được.
>
> **Tìm kiếm và hỏi đáp** (gói "Kho bản ghi", lên prod 17/10/2026 06:23): tìm kiếm không gọi model và không tính tiền. Hỏi đáp dùng key OpenAI của server và model `SUMMARY_MODEL`; mỗi tài khoản có số lượt mỗi ngày giờ VN `ASK_FREE_DAILY` (30) / `ASK_PAID_DAILY` (200) (sổ `ask:<userId>:<ngày>` trong `summary_usage`), còn trần chi phí tháng của tài khoản, trần tổng tháng và cờ tạm ngưng khi key hỏng thì **dùng chung với tóm tắt**. `admin usage --recompute` cộng cả `ask_runs.costS`; `--apply` từ chối khi còn lượt hỏi đáp đang mở. Lần khởi động đầu tiên của bản có index tìm kiếm dựng index cho mọi phiên đã chép xong rồi ghi doc `search.backfill` vào `kv`; có doc đó thì không chạy lại. Phiên dựng lỗi do retention đối soát mỗi giờ dựng lại. Muốn dựng lại cả kho: xoá doc `search.backfill` rồi restart.
>
> **Dịch lời chép** (chưa deploy): dịch cả phiên khi người dùng bấm "Dịch", dùng key OpenAI của server và model `SUMMARY_MODEL`. Mỗi tài khoản có số lượt mỗi ngày giờ VN `TRANSLATE_FREE_DAILY` (3) / `TRANSLATE_PAID_DAILY` (20) (sổ `tr:<userId>:<ngày>` trong `summary_usage`); trần chi phí tháng của tài khoản, trần tổng tháng và cờ tạm ngưng khi key hỏng **dùng chung với tóm tắt và hỏi đáp**. Một lượt gọi model theo từng lô 100 câu trong task nền, giữ chỗ một lần và quyết toán một lần (`translate_runs`); lượt dài có thể chạy hơn 10 phút nên `admin usage --apply` coi lượt dịch mở dưới 60 phút là còn chạy. Bản dịch lưu ở collection mới `transcript_translations` (index unique `{userId, key, lang}` tạo lúc khởi động, lỗi thì dừng khởi động). Khởi động lại giữa lượt thì lượt đó báo "bị ngắt", người dùng bấm dịch tiếp; phần giữ chỗ của lượt dở không được quyết toán, giống tóm tắt (đối soát bằng `admin usage --recompute`).
>
> **Gắn tenant cho dữ liệu cũ** (bản đầu tiên có đa tenant, [ADR 0001](./decisions/0001-tenants-by-domain.md)): lúc khởi động, trước khi nhận request, BE gắn `tenantId: "primary"` cho mọi doc chưa có ở `users`, `sessions`, `devices`, `transcripts`, `jobs` (chỉ thêm trường), xoá `auth_states`/`logout_jtis` chưa có `tenantId` (lượt đăng nhập đang dở phải bắt đầu lại), rồi tạo index unique `users.(tenantId, idpSub)` và bỏ `idpSub_unique`. Journal ghi `đã gắn tenantId cho dữ liệu cũ` kèm số doc; còn sót doc chưa gắn hoặc Mongo lỗi ⟹ journal `migration tenant: …` và BE dừng (không phục vụ). Chạy lại được, lần sau không đổi gì (mỗi lần khởi động chỉ đọc mỗi collection một lượt). `mongodump --db "$DB_NAME"` vào thư mục sao lưu ngay trước lần deploy này. Quay lại: nếu đã cài `scribe-tenants.conf` thì gỡ nó trước (§nginx cho domain tenant), rồi chạy binary trước (bỏ qua `tenantId`, tạo lại `idpSub_unique`); không cần khôi phục dump trừ khi dữ liệu hỏng. Lệnh `admin` theo email tìm trong tenant chính (`--tenant <id>` cho tenant khác) và chỉ thấy user đã gắn `tenantId`: chạy sau khi service bản mới đã khởi động một lần.
>
> **Đổi dữ liệu chép lời (`transcripts`):** chạy `scripts/transcripts-audit.js` (chỉ đọc) trước, và `mongodump --db "$DB_NAME" --collection transcripts` vào thư mục sao lưu ngay trước deploy. Khôi phục khi dữ liệu hỏng: `mongorestore --drop --nsInclude "$DB_NAME.transcripts" <thư mục sao lưu>`.

Cần `cargo-zigbuild` (build chéo musl trên macOS: `brew install zig`, `rustup target add x86_64-unknown-linux-musl`, `cargo install cargo-zigbuild --locked`), cộng SSH key `.65`.

> Build backend bằng `cargo zigbuild` (script Docker `scripts/build-musl.sh` cũ đã bỏ ngày 2026-10-16: còn đòi biến Firebase và không chuyển biến OIDC/hạn mức vào container).

```bash
set -a; . "$(scripts/env-dir.sh)/.env.production"; set +a

# 1) Build BE musl (cấu hình lấy từ .env.production, KHÔNG hardcode)
cargo zigbuild --release --target x86_64-unknown-linux-musl -p scribe-be
#    → target/x86_64-unknown-linux-musl/release/scribe-be

# 2) Build FE
cd web && pnpm install --frozen-lockfile && pnpm run build && cd ..   # → web/dist

# 3) Đẩy lên .65 (KEY = file tạm chứa private key, xem mục Server)
rsync -avz -e "ssh -i $KEY -p 24700" target/x86_64-unknown-linux-musl/release/scribe-be root@103.72.98.65:/tmp/scribe-be.new
rsync -avz --delete -e "ssh -i $KEY -p 24700" web/dist/ root@103.72.98.65:/tmp/scribe-fe/
ssh -i "$KEY" -p 24700 root@103.72.98.65 "
  set -e
  install -o $DEPLOY_USER -g $DEPLOY_USER -m 700 /tmp/scribe-be.new $DEPLOY_APP_DIR/$DEPLOY_SERVICE.new
  mv -f $DEPLOY_APP_DIR/$DEPLOY_SERVICE.new $DEPLOY_APP_DIR/$DEPLOY_SERVICE
  rm -f /tmp/scribe-be.new
  rsync -a /tmp/scribe-fe/assets/ $DEPLOY_WEB_ROOT/assets/
  rsync -a --delete --exclude /assets/ /tmp/scribe-fe/ $DEPLOY_WEB_ROOT/
  rm -rf /tmp/scribe-fe
  systemctl restart $DEPLOY_SERVICE && sleep 2 && systemctl is-active $DEPLOY_SERVICE"
rm -f "$KEY"
```

Skill SSH lab không có rsync: đẩy bản build FE bằng `tar -C web/dist -czf - . | lab-ssh.sh 65 "tar -xzf - -C /tmp/scribe-fe"` (cách skill `scribe-ops-prod` làm: §6 của skill). Sao lưu trước mỗi lần thay bản vào `/root/scribe-backup-<YYYYMMDD-HHMMSS>/` (binary `scribe-be.prev`, `dist/`, dump Mongo khi cần). Quay lại bản trước: `install` binary `.prev` từ thư mục sao lưu thành `$DEPLOY_APP_DIR/$DEPLOY_SERVICE.new`, `mv -f` đè lên binary rồi `systemctl restart $DEPLOY_SERVICE`.

> **Deploy web:** chỉ deploy khi không còn phiên ghi hay phiên tải lên đang mở và hàng đợi chép lời rỗng. Phiên mở là doc còn trong `recording_sessions` / `upload_sessions` (finalize thì doc bị xoá, `server/scribe-be/src/recordings.rs`, `upload.rs`; phiên bỏ dở do `retention.rs` dọn); hàng đợi đếm như ở đầu §Deploy. Giữ `assets/` của bản trước để tab đang mở bản cũ không bị 404 khi tải tệp trong `assets/`: rsync `assets/` không `--delete` và chạy trước, nên `index.html` mới không trỏ tới tệp chưa có. Tệp cũ trong `assets/` dọn ở lần deploy sau.
>
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
- [ ] FE load, đăng nhập qua auth-next được (`/api/auth/login` → 303 sang `auth.hoandat.com` với `client_id=scribe-web`; allowlist mở → mọi email đã xác minh).
- [ ] `/api/auth/native/config` trả client `scribe-mobile` (app iPhone đăng nhập được).
- [ ] Ghi/cắt file → upload + phát lại (Range) hoạt động.
- [ ] Index tìm kiếm: lần khởi động đầu của bản có index, journal có dòng `đã dựng index tìm kiếm cho dữ liệu cũ` kèm `sessions`, `sentences`, `failed`; `db.kv.findOne({_id: "search.backfill"})` có `doneAt`. Các lần khởi động sau không có dòng này (đã có cờ). `failed` lớn hơn 0 thì xem dòng `dựng index lỗi` cùng lúc; retention dựng lại các phiên đó ở lượt quét kế.

## Trạng thái đã xác minh (2026-10-16, sau đổi tên + đăng nhập)

- **Bootstrap `.65`: XONG** — user `scribe`, unit `scribe-be`, vhost `/etc/nginx/conf.d/scribe.conf`, db `scribe` + user Mongo `scribe` đều có sẵn. Deploy = redeploy (idempotent).
- **Cloudflare:** `scribe.hoandat.com` proxied, SSL "Full" (CF↔origin là HTTPS, chứng chỉ tự ký chung của box); `http://` → 301 `https://`. Domain cũ `scribe.inetdev.io.vn` đã bỏ (vhost và DNS đã xoá, không 301).
- **IdP:** `scribe-web` và `scribe-mobile` bật trên auth-next; `SCRIBE_WEB_CLIENT_SECRET` nằm trong `/var/www/hoandat-auth/.env`.
- **MongoDB `.65`:** auth ENABLED; `MONGO_URI` dùng user riêng `scribe` (database + authSource = `scribe`). **mongod dùng chung cho team theo chủ đích** → cổng `27017` mở ra ngoài + mật khẩu chia sẻ là **cố ý**, KHÔNG firewall về localhost / KHÔNG rotate (team cần kết nối trực tiếp).
- **Sao lưu:** `/root/scribe-backup-<ts>/` (mới nhất `20261019-121148`, trước khi lên xoá tài khoản + trang pháp lý; `20261019-103131` trước Key riêng; `20261017-160739` trước khi chuyển chép lời sang ElevenLabs; `20261017-130838` trước dịch Việt ↔ Anh; `20261017-123216` trước xuất VTT/Markdown/Word; `20261017-111120` trước chép trực tiếp; `20261017-072952` có kèm `files-expiry-before.json` là hạn tệp trước khi đổi sang gói free 7 ngày) và `/root/onescribe-backup-20261016-124301/` (dump db `onescribe` trước đổi tên). Không có cron backup.

## Header bảo mật

nginx gắn bộ header bảo mật cho mọi response của Scribe (FE, `/assets/`, `/api/`, cả trang lỗi); BE Rust không tự đặt header nào. Nguồn duy nhất: `deploy/nginx-scribe-security-headers.inc`; mỗi server block (`deploy/nginx-scribe.conf`, `deploy/nginx-scribe-tenants.conf`) include nó ở mức server (cho `/api/`), và `deploy/nginx-scribe-locations.inc` include lại trong `/assets/` và `/` — hai location này có `add_header Cache-Control` riêng nên nginx không thừa kế header của mức server. File include nằm ngoài `conf.d/` vì `conf.d/*.conf` nạp ở mức http, sẽ áp cho mọi vhost của box shared.

| Header | Giá trị |
|---|---|
| `Strict-Transport-Security` | `max-age=31536000; includeSubDomains` (không preload) |
| `X-Content-Type-Options` | `nosniff` |
| `Referrer-Policy` | `strict-origin-when-cross-origin` |
| `Permissions-Policy` | `camera=(), geolocation=(), microphone=(self), display-capture=(self)` — ghi âm micro và chia sẻ màn hình/âm thanh hệ thống (`web/src/features/recorder/media-capture.ts`) |
| `Content-Security-Policy` | mọi nguồn `'self'`; script thêm hash của script inline áp theme trong `web/index.html`; `style-src 'unsafe-inline'` vì Radix chèn thẻ `<style>`; `object-src 'none'`, `form-action 'self'`, `frame-ancestors 'none'`, `manifest-src 'self'`. Lý do từng directive ghi trong file include |

**Kiểm:** `pnpm run build && pnpm run test:e2e:csp` (trong `web/`, CI cũng chạy) phục vụ `web/dist` với đúng header đọc từ file include, chạy `e2e/csp/csp.spec.ts` và mọi spec giao diện web trên Chromium + WebKit; vi phạm CSP nào (sự kiện `securitypolicyviolation` trong trang, hoặc báo cáo `report-uri` từ các spec khác) cũng làm run thất bại. Sửa script inline trong `web/index.html` thì hash đổi: run báo vi phạm `script-src-elem`, cập nhật hash trong file include theo giá trị trình duyệt báo.

**Lưu ý Cloudflare:** CSP chặn mọi script mà Cloudflare tự chèn (Web Analytics, Rocket Loader). Bật tính năng nào như vậy cho `scribe.hoandat.com` thì phải thêm origin của nó vào CSP trước.

**Trạng thái prod (22/10/2026 23:30):** header đã áp dụng (vhost cũ sao lưu ở `/root/scribe.conf.bak-20261022-233008`). Cloudflare Web Analytics đang tự chèn `static.cloudflareinsights.com/beacon.min.js` và bị CSP chặn có chủ đích: chính sách quyền riêng tư ghi Scribe không dùng công cụ phân tích. Để hết dòng lỗi CSP trong console, tắt Web Analytics cho `scribe.hoandat.com` trên dashboard Cloudflare; token `token-full-access-cloudflare` không có quyền Web Analytics (API `rum/site_info` trả 10000).

**Deploy 23/10/2026 19:11 (`74c132f`):** BE + web (lịch sử ghi chia theo ngày, đổi tên bản ghi), sao lưu `/root/scribe-backup-20261023-191120`; iOS TestFlight build `20261023.1211`. Khi đóng gói `web/dist` bằng `tar` trên macOS, đặt `COPYFILE_DISABLE=1` để không mang theo tệp `._*` (lần này đã xoá 42 tệp như vậy khỏi web root).

**Deploy 23/10/2026 21:58 (`7d9cc58`):** chỉ web (Lời đã chép chia theo ngày), không restart BE; sao lưu `/root/scribe-backup-20261023-215805-web`; trước deploy đếm `recording_sessions`, `upload_sessions` và hàng đợi chép lời đều bằng 0. Lần đầu deploy từ máy Windows `hoandat-pc`, khác với máy Mac ở mấy điểm:
- Lúc deploy máy này chưa có `.env.production` (khi đó env còn nằm trong repo trên Mac), nên đếm phiên bằng tài khoản `mongo_admin_*` của entry `SSH_SERVER_STAGING`, chỉ đọc, mật khẩu qua stdin. Ngay sau đó env chuyển lên iCloud (khung ở §Tên trên prod), `web/.env` Firebase cũ của iCloud giữ ở `one-scribe/web/.env.bak-20261023-2205`.
- `prod-ssh.sh` từng bị từ chối khoá trên Windows: `jq` bản Windows xuất CRLF nên tên đăng nhập đọc ra thành `root
`. Script đã bỏ `
`; `prod-ssh.sh` và `prod-mongosh.sh` chạy được từ Windows.
- Hook của Claude chặn lệnh Bash có chữ `dist`: đặt các bước đẩy và thay web root vào một script rồi chạy script đó.

**Deploy 24/10/2026 17:35 (`00240aa`, đa tenant):** BE + web, build trên máy Mac; `.env.production` thêm `IDP_LOOPBACK_URL`, `SCRIBE_WEB_CLIENT_KEY_PATH`, `TENANT_SEALING_KEYS_DIR` (bản trước `.env.production.bak-261024-1735` cùng thư mục iCloud). Trước deploy: `mongodump` db `scribe` và `auth` vào `/root/backup-261024-scribe-tenants/`; diễn tập migration trên bản sao tạm `scribe_migtest_261024` (restore 1,9 s, gắn tenant 57 ms, đổi index 36 ms, rồi xoá db tạm); hàng đợi, `recording_sessions`, `upload_sessions` đều 0. Khoá niêm phong RSA-3072 `/opt/scribe-be/tenant-sealing-keys/53187637a29c261a.pem`, public key ở `/var/www/hoandat-auth/secrets/scribe-sealing.pub.pem` (không phải `/var/www/hoandat-auth/scribe-sealing-public.pem` như lệnh mẫu ở §Key AI của tenant: thư mục `secrets/` nằm ngoài rsync của `infra/deploy.sh`); bản sao private key, public key và khoá app `scribe-web` ở iCloud `ENV production/one-scribe/server/` (sha256 khớp hai đầu). Sao lưu `/root/scribe-backup-20261024-173551` (`scribe-be.prev`, `web/`). Journal: `đã gắn tenantId cho dữ liệu cũ` (users 3, sessions 0, devices 7, transcripts 84, jobs 1; xoá auth_states 2, logout_jtis 0), `key AI của tenant bật` với vân tay trên; ngừng phục vụ khoảng 0,15 s; `users` có `tenantId_idpSub_unique`, không còn `idpSub_unique`, không doc nào thiếu `tenantId`. RSS `scribe-be` 50 MB trước, 20 MB ngay sau khởi động (`MemoryMax=1G`), `MemAvailable` máy 3.440 MB trước và sau. 17:37 cài `/etc/nginx/scribe/locations.inc` và `/etc/nginx/conf.d/scribe-tenants.conf` (`scribe.conf` chưa thay, vẫn khối location riêng, cùng hành vi); header của `scribe.hoandat.com` trước/sau giống hệt. `scribe.inetdev.io.vn` (Proxied, tạo 17:38) trả 502 tới khi provisioner cấp chứng chỉ `scribe.inetdev.io.vn` (sau khi chủ domain nhập key chép lời ở `/admin`).

**Deploy lên `.65` — CẦN USER DUYỆT trước khi chạy** (đổi cấu hình nginx shared của prod). Từ máy deploy, `KEY` như §Deploy:

```bash
# 1) Đẩy file include + vhost (thư mục /etc/nginx/scribe/ tạo lần đầu)
ssh -i "$KEY" -p 24700 root@103.72.98.65 'mkdir -p /etc/nginx/scribe && cp -a /etc/nginx/conf.d/scribe.conf /root/scribe.conf.bak-$(date +%Y%m%d-%H%M%S)'
scp -i "$KEY" -P 24700 deploy/nginx-scribe-security-headers.inc root@103.72.98.65:/etc/nginx/scribe/security-headers.inc
scp -i "$KEY" -P 24700 deploy/nginx-scribe.conf root@103.72.98.65:$DEPLOY_NGINX_CONF
# 2) Kiểm cú pháp rồi mới reload; nginx -t lỗi thì chép lại bản .bak, KHÔNG reload
ssh -i "$KEY" -p 24700 root@103.72.98.65 'nginx -t && systemctl reload nginx'
# 3) Xác nhận từ ngoài
curl -sI "https://$DEPLOY_DOMAIN/" | grep -iE 'strict-transport|content-security|permissions-policy|referrer-policy|x-content-type'
curl -sI "https://$DEPLOY_DOMAIN/api/health" | grep -i content-security
```

Sau reload: mở web, đăng nhập, ghi thử vài giây và phát lại, mở account menu; DevTools Console không được có dòng `Refused to … Content Security Policy`.

## nginx cho domain tenant

`scribe.<domain>` của tenant sổ ([ADR 0001](./decisions/0001-tenants-by-domain.md)) dùng cùng backend và cùng `dist` với `scribe.hoandat.com`; backend chọn tenant theo `Host`, web đọc tenant lúc chạy (`GET /api/idp`), nên không có build hay cấu hình riêng cho từng domain. Ba file:

- `deploy/nginx-scribe-locations.inc` → `/etc/nginx/scribe/locations.inc`: `/api/` (giữ `Host`), web tĩnh, `/assets/`, tách từ `nginx-scribe.conf` (không đổi hành vi của `scribe.hoandat.com`).
- `deploy/nginx-scribe.conf` → `$DEPLOY_NGINX_CONF`: block cụ thể `scribe.hoandat.com`, nay include `locations.inc`; luôn thắng block regex.
- `deploy/nginx-scribe-tenants.conf` → `/etc/nginx/conf.d/scribe-tenants.conf`: cổng 443, `server_name ~^scribe\.(?<tenant>.+)$`, chứng chỉ theo `$tenant_cert` (map trong `00-tenants-http.conf` của auth-next: lineage riêng `scribe.<domain>` do provisioner cấp), tin `127.0.0.1` và dải Cloudflare ở cấp server như `tenant-auth.conf` của auth-next. Cổng 80 (HTTP-01, Cloudflare Flexible) do `tenants-acme.conf` của auth-next lo.

**CẦN USER DUYỆT trước khi cài** (nginx dùng chung của prod), sau khi binary Scribe bản đa tenant đã chạy (binary cũ phục vụ mọi `scribe.*` như tenant chính) và auth-next đã có map `scribe.` (`00-tenants-http.conf`) và nhận nhãn `scribe` ở cổng 80. Block trả 421 khi `Host` khác SNI. Từ máy deploy, `KEY` như §Deploy:

```bash
# 0) Không block regex nào khác khớp scribe.<d>; sao lưu cấu hình hiện tại
ssh -i "$KEY" -p 24700 root@103.72.98.65 'nginx -T 2>/dev/null | grep -nE "server_name\s+[^;]*~"; ts=$(date +%Y%m%d-%H%M%S); cp -a /etc/nginx/conf.d/scribe.conf /root/scribe.conf.bak-$ts'
# 1) Đẩy phần chung trước, rồi hai vhost
scp -i "$KEY" -P 24700 deploy/nginx-scribe-locations.inc root@103.72.98.65:/etc/nginx/scribe/locations.inc
scp -i "$KEY" -P 24700 deploy/nginx-scribe.conf root@103.72.98.65:$DEPLOY_NGINX_CONF
scp -i "$KEY" -P 24700 deploy/nginx-scribe-tenants.conf root@103.72.98.65:/etc/nginx/conf.d/scribe-tenants.conf
# 2) Kiểm cú pháp rồi mới reload; lỗi thì chép lại bản .bak, gỡ scribe-tenants.conf, KHÔNG reload
ssh -i "$KEY" -p 24700 root@103.72.98.65 'nginx -t && systemctl reload nginx'
# 3) scribe.hoandat.com không đổi (so header trước/sau), domain tenant trả trang web
curl -sI "https://$DEPLOY_DOMAIN/" | sort; curl -s "https://$DEPLOY_DOMAIN/api/idp"
curl -s https://scribe.<domain>/api/idp     # {…, "primary": false, "scribe": "enabled" | "disabled"}
```

Đã kiểm cú pháp (24/10/2026, `nginx -t` của nginx Homebrew trên máy Mac, cùng `00-tenants-http.conf` của auth-next và chứng chỉ tạm): hợp lệ. Quay lại: gỡ `/etc/nginx/conf.d/scribe-tenants.conf`, chép lại `scribe.conf` từ bản `.bak`, `nginx -t`, reload (`locations.inc` để lại vô hại).

## Log và audit

BE ghi hai lớp (thêm 2026-10-15):

- **Access log** ra journal (`target=access`): mỗi request `/api` một dòng `method`, `path`, `status`, `ms`, `ip` (từ `X-Real-IP` do nginx đặt), `user` (id Mongo, `-` khi chưa đăng nhập). Chunk upload/ghi thành công ở mức debug (khi ghi âm ≈1 request/giây); `/api/health` không log. Xem: `journalctl -u "$DEPLOY_SERVICE" -o cat | grep "access: request"`.
- **`audit_events`** trong Mongo (database `$DB_NAME`): một doc cho mỗi hành vi `auth.login`, `auth.login_denied`, `auth.logout`, `auth.link` (ghép user cũ theo email), `auth.backchannel_logout`, `auth.device_revoked`, `recording.start`, `recording.finish`, `upload.start`, `upload.finish`, `job.create`, `job.done`, `job.failed`, `job.download`, `file.play`, `file.download`, `file.delete`, `file.delete_all`, `recordings.archive`, `transcript.create`, `transcript.rerun`, `transcript.done`, `transcript.failed`, `transcript.rename`, `transcript.edit`, `transcript.delete`, `transcript.blocked`, `transcript.cap_50`, `transcript.cap_80`, `transcript.cap_reached`, `transcript.key_invalid`, `transcript.share`, `realtime.token`, `realtime.blocked`, `summary.create`, `summary.failed`, `summary.blocked`, `summary.key_invalid`, `chat.link`, `chat.unlink`, `ask.create`, `ask.blocked`, `ask.failed`, `admin.set_plan`, `admin.usage_recompute`, `admin.shorten_free_retention`, `admin.block`, `admin.unblock`, `account.delete` (xoá tài khoản: chỉ số tệp, byte, doc đã xoá) — kèm `at`, `userId`, `ip` và chi tiết (fileId, tên, kích thước…; sự kiện transcript chỉ ghi khoá phiên, id file và số câu/độ dài tên, không ghi tiêu đề hay câu; sự kiện tóm tắt và gửi chat không ghi nội dung biên bản; sự kiện hỏi đáp không ghi câu hỏi hay câu trả lời, câu tìm kiếm không vào audit). TTL 180 ngày trên `at`; index `{userId, at}` để tra theo user. Khi user xoá tài khoản, audit của họ ở lại tới hết TTL nhưng bị bỏ `name`, `email`, `title`, `chatId`, `error` (và `reason` của `admin.block`) (`server/scribe-be/src/account.rs`). Chính sách `/privacy` nói bản sao lưu `/root/scribe-backup-*` được xoá khi bản cập nhật đã ổn định: dọn các thư mục cũ sau mỗi đợt deploy ổn định. Ghi nền: Mongo lỗi chỉ log warn, không làm hỏng request.

## Ghi chú vận hành

- **Preview/level meter khi ghi** chỉ hiện lúc đang ghi (`LiveMonitor` trong `web/src/pages/record/record-page.tsx`) — reload trang idle không thấy; phải bấm Bắt đầu ghi.
