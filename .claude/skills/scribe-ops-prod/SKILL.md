---
name: scribe-ops-prod
description: Tra cứu thông tin và vận hành Scribe (one-scribe) trên PRODUCTION (scribe.hoandat.com, server dùng chung 103.72.98.65 cổng SSH 24700, systemd scribe-be cổng 127.0.0.1:8787, binary /opt/scribe-be, FE /var/www/scribe/dist, database scribe trên mongod chung 27017). Chỉ đường đọc docs/deploy-web-production.md — credential ở $ALL_IN_ONE_STAGING_DEV entry SSH_SERVER_STAGING, cấu hình prod ở .env.production (không commit), SSH/mongosh qua scripts/ của skill, deploy bằng cargo zigbuild + npm run build theo §6. Kích hoạt khi user nhắc "prod", "production", "scribe.hoandat.com", "server .65", "SSH prod", "mongosh prod", "log prod", "journal prod", "deploy prod", "rollback", "scribe-be", "đăng nhập prod", "backup Scribe", "audit_events", "hàng đợi chép lời prod". KHÔNG dùng cho dev local (localhost:5173/8787, Mongo dev lab 176, cargo run, npm run dev) và không dùng cho app iPhone cài qua cáp (docs/ios-app.md).
---

# Scribe Ops — Production

Skill này là **bản chỉ đường**, không phải bản sao tài liệu. Nguồn chuẩn là `docs/deploy-web-production.md`
(server, topology, biến build, deploy, kiểm tra, log/audit) và `docs/codebase-summary.md` (module, API, collection).
Gặp chỗ skill và tài liệu/script nói khác nhau thì **tài liệu và script đúng** — sửa skill.

## Cổng bắt buộc — chạy TRƯỚC mọi bước khác

Mỗi lần skill này được dùng, làm đủ ba bước dưới **trước** khi đọc tài liệu, SSH hay chạy lệnh:

1. **Nạp skill `typesafe:typesafe-ai` và để model Jev ra quyết định.** Key: biến `TYPESAFE_API_KEY`,
   nạp từ `$ACCESS_TOKENS_JSON` mục `.tokens[]` có `service == "api.typesafe.ai"` (đọc thẳng vào biến,
   không in giá trị). Quyết định đầu tiên giao cho Jev là **chọn skill AgentKit** ở bước 2 — lượt gọi
   này đồng thời là phép thử Jev còn dùng được. Sau đó mọi điểm quyết định trong lượt việc (chọn hướng
   điều tra, chọn phương án, làm hay bỏ một bước) đều hỏi Jev, với state là các **sự thật đã kiểm**;
   báo lại user cả câu hỏi lẫn xác suất Jev trả về.
2. **Gọi skill AgentKit phù hợp** (Skill tool, tên `ak-*` trong danh mục skill đang cài) để xử lý việc:

   | Việc | Skill AgentKit |
   |---|---|
   | Điều tra sự cố, lỗi, log bất thường | `ak-debug` |
   | Sửa một lỗi đã xác định | `ak-fix` |
   | Deploy, rollback, đổi hạ tầng | `ak-deploy` / `ak-devops` |
   | Truy vấn, chỉ mục, dung lượng Mongo, backup/restore | `ak-databases` |
   | Tìm tệp, script, nơi đặt cấu hình | `ak-scout` |
   | Bí mật lộ, rà bảo mật | `ak-security` |
   | Không rõ chọn skill nào, việc nhiều bước | `ak-agentkit` (bộ định tuyến) |

3. **Không dùng được ⟹ báo ngay và DỪNG.** Skill AgentKit không có trong danh mục hoặc gọi lỗi, skill
   `typesafe:typesafe-ai` không nạp được, thiếu key, Jev trả lỗi/timeout/đáp án không hợp lệ ⟹ báo
   user ngay: thứ nào hỏng, thông điệp lỗi nguyên văn, đã tới bước nào. Rồi **ngưng toàn bộ công việc
   của AI** cho yêu cầu đó — không tự quyết thay Jev, không làm tiếp bằng công cụ gốc, không thay bằng
   skill/model khác. Chờ user chỉ đạo.

Jev quyết các điểm **kỹ thuật**; đáp án của Jev **không thay** lời yêu cầu tường minh của user cho thao
tác ghi prod (§1 luật 1) và không thay phép đo — Jev chọn "đo trước" thì phải đo thật.

**Quyết định để lại cho user ⟹ kèm tư vấn của Jev, user vẫn chọn.** Mỗi khi kết thúc bằng các phương án cho user
chọn (thao tác ghi prod, đổi chính sách, phạm vi…), gửi đúng các phương án đó cho Jev dưới dạng câu hỏi `choice`
(state = sự thật đã kiểm, kể cả rủi ro và đường lùi của từng phương án) rồi trình cho user: phương án Jev chọn + xác
suất từng phương án. Jev chỉ **tư vấn**: AI KHÔNG tự thực hiện theo lựa chọn đó, chờ user quyết.

## 0. Xác định đúng môi trường trước

Scribe chỉ có **hai môi trường: dev local và prod**. Không có staging (chữ STAGING trong tên credential là tên
của máy edge iNET, không phải môi trường của Scribe).

| Dấu hiệu | Môi trường | Ai lo |
|---|---|---|
| `scribe.hoandat.com`, `103.72.98.65`, `scribe-be`, `/opt/scribe-be`, `/var/www/scribe`, db `scribe` trên `.65` (tên cũ `onescribe*` từ trước 16/10/2026) | **Scribe prod** | skill này |
| `localhost:5173` / `:8787`, `.env.development`, Mongo `192.168.122.176` (lab 176), `cargo run`, `npm run dev` | Dev local | `README.md` §Chạy dev, skill `inet-lab-ops` cho box lab |
| App iPhone cài qua cáp (`npm run ios:device`) | Bản build trên máy người dùng, nói chuyện với prod | `docs/ios-app.md` |
| `localman.hoandat.com`, `auth.hoandat.com`, pm2 `localman-api`/`hoandat-auth`, `tunelo`, mongod `27018` | Repo khác trên cùng máy | skill của repo đó; không đụng |

Máy .65 là **máy dùng chung** (CentOS Stream 9, ~15 vhost nginx, mongod chung): một thao tác sai ở đó ảnh hưởng mọi
app trên máy, không chỉ Scribe.

## 1. Luật cứng (vi phạm = sự cố thật)

1. **Mặc định CHỈ ĐỌC.** Deploy, `systemctl restart|stop scribe-be`, thay binary/FE, sửa systemd unit hay nginx
   conf, ghi Mongo, `mongorestore`, xoá file trong `/opt/scribe-be/data` ⟹ chỉ làm khi user yêu cầu **tường minh
   cho chính lần đó**. Không tự đề xuất deploy prod vào "việc còn lại".
2. **Chỉ chạm thứ của Scribe.** Chỉ `systemctl … scribe-be`; không đụng service/pm2 của app khác; không reload
   nginx khi chưa `nginx -t`; chỉ THÊM server block của Scribe, không sửa vhost khác.
3. **DB `scribe` nằm trên mongod CHUNG cổng 27017** (cùng các db của app khác). Chỉ truy vấn db `scribe` qua
   `scripts/prod-mongosh.sh`; không `dropDatabase`, không đụng db khác, không đổi cấu hình mongod. Cổng 27017 mở ra
   ngoài và mật khẩu chia sẻ là **cố ý** (team kết nối trực tiếp) — không tự firewall/rotate.
4. **Restart backend khi đang chép lời làm tốn tiền:** lượt dở không mất (key OpenAI nằm ở server, lượt `running`
   được xếp lại) nhưng chạy lại từ đầu, audio gửi lại bị OpenAI tính lần nữa. Trước mọi restart: hàng đợi nên rỗng
   (§6); khác 0 thì chờ nếu được.
5. **KHÔNG build trên server.** Build ở máy dev (`cargo zigbuild` cho backend musl, `npm run build` cho FE), đẩy
   artifact lên.
6. **Cấu hình nằm trong binary.** `MONGO_URI`, `DB_NAME`, `TRANSCRIBE_EMAILS`, allowlist… nhúng lúc build từ
   `.env.production` (`option_env!`), server không có `.env`. Đổi danh sách email chép lời = sửa `.env.production` +
   build + deploy lại. Không `strings` binary để đọc cấu hình.
7. **KHÔNG xử lý dữ liệu nặng trên .65**: ~7,5 GB RAM, còn trống khoảng 3 GB cho cả máy (đo 16/10/2026); unit có
   `MemoryMax=1G`. Truy vấn có projection/`limit`, log lọc tại nguồn (`journalctl … | grep`).
8. **Không in bí mật** (`MONGO_URI`, private key SSH, các tệp 600 trong `/opt/scribe-be`: `.session-key`,
   `.oidc-web-secret`, `.openai-key` — chủ sở hữu giữ bản gốc ở `$ACCESS_TOKENS_JSON` mục `api.openai.com` —,
   `.notify-bot-token`; `SCRIBE_WEB_CLIENT_SECRET` trong `/var/www/hoandat-auth/.env`): đọc vào biến, truyền qua env/stdin chứ không qua argv; `.env.production` chỉ liệt kê tên biến
   (`sed 's/=.*//' .env.production`).
9. **Chọn đúng tệp credential:** máy .65 nằm trong `$ALL_IN_ONE_STAGING_DEV`, entry `SSH_SERVER_STAGING`; **không** có
   trong `$ALL_IN_ONE_PROD`. Cổng 24700 có allowlist IP (`CP_SSH_ACL`) — SSH bị chặn thì báo user, không gỡ chain.
10. Bí mật đã lộ: báo một câu (thứ gì, lộ ở đâu) rồi dừng, chờ user.

## 2. Câu hỏi → đọc mục nào

Tìm nhanh tiêu đề: `grep -n '^#' docs/deploy-web-production.md`.

| Cần biết | Đọc ở đâu | Ghi chú |
|---|---|---|
| Tên thật trên prod (domain, service, thư mục, db) | `docs/deploy-web-production.md` **§Tên trên prod** | khớp `.env.production` (`DEPLOY_*`) |
| Tệp bí mật trên server, đăng nhập hoandat-auth | **§Tệp bí mật trên server**, **§Đăng nhập hoandat-auth** | |
| Máy, cổng SSH, credential | **§Server**, **§SSH source-IP allowlist** | |
| nginx, thư mục, luồng request | **§Deploy topology** | |
| Biến build, vì sao cấu hình nằm trong binary | **§Ràng buộc then chốt — cấu hình bake compile-time** | |
| Dựng lại từ đầu | **§Setup 1 lần trên `.65`** | |
| Deploy, sao lưu, chờ hàng đợi | **§Deploy (làm tay)** + §6 dưới đây | |
| Nghiệm thu | **§Verify sau deploy** | |
| Access log, `audit_events` | **§Log và audit** | |
| Module, route, collection | `docs/codebase-summary.md` | |
| App iPhone | `docs/ios-app.md` | |
| Kiểm dữ liệu `transcripts` | `scripts/transcripts-audit.js` (chỉ đọc) | chạy qua `prod-mongosh.sh` |

## 3. Đường đọc ưu tiên — rẻ nhất trước

1. **Từ ngoài, không SSH:**

   ```bash
   curl -s -o /dev/null -w '%{http_code}\n' https://scribe.hoandat.com/        # 200 = FE
   curl -s https://scribe.hoandat.com/api/health                                 # {"status":"ok"}
   curl -s -o /dev/null -w '%{http_code}\n' https://scribe.hoandat.com/api/me    # 401 = BE sống, cần đăng nhập
   curl -s https://scribe.hoandat.com/ | grep -o 'assets/index-[^"]*\.js'        # so với web/dist sau build
   ```

2. **Trên server, chỉ đọc** (từ gốc repo; `S=.claude/skills/scribe-ops-prod/scripts`):

   ```bash
   $S/prod-ssh.sh 'systemctl is-active scribe-be; systemctl show scribe-be -p NRestarts -p ActiveEnterTimestamp' </dev/null
   $S/prod-ssh.sh 'journalctl -u scribe-be --since "-1 hour" --no-pager -o cat | grep -E "ERROR|WARN" | tail -50' </dev/null
   $S/prod-ssh.sh 'journalctl -u scribe-be --since "-1 hour" -o cat | grep "access: request" | tail -50' </dev/null
   $S/prod-ssh.sh 'free -h; df -h /; ls -d /root/scribe-backup-* /root/onescribe-backup-*' </dev/null
   ```

3. **Dữ liệu** (script mongosh chỉ đọc, bắt đầu thẳng bằng truy vấn trên `db`):

   ```bash
   $S/prod-mongosh.sh scripts/transcripts-audit.js
   $S/prod-mongosh.sh /tmp/q.js   # vd: print(db.transcripts.countDocuments({status:{$in:["queued","running"]}}))
   ```

## 4. Sự thật dễ nhầm

- **Định danh user:** `userId` trong mọi collection là `_id` hex của `users`, không phải email, `idpSub` (tài
  khoản hoandat-auth) hay `firebaseUid` (chỉ còn ở user cũ). Tra theo email ⟹ tìm `users` trước.
- **Collection:** `files` (`kind` upload|output|recording, hạn 14 ngày theo `expiresAt`, sweep mỗi giờ xoá đĩa trước
  rồi doc), `transcripts` (kết quả máy + hàng đợi chép lời, `_id` = id file), `transcript_docs` (tiêu đề + câu sửa theo
  phiên, `{userId, key}` unique), `jobs`, `upload_sessions`/`recording_sessions`, `audit_events` (TTL 180 ngày).
- **Phiên iPhone** là nhiều file tên `<sessionId>#<index>`; khoá transcript `s:<sessionId>`, bản ghi trình duyệt
  `f:<fileId>`.
- **`audit_events` không chứa tiêu đề hay câu transcript** — đừng tìm nội dung ở đó; chỉ có `key`, `fileId`, số câu.
- **Hàng đợi chép lời sau restart:** lượt `running` được xếp lại; lượt bị cắt ngang lần hai thì `failed` với thông báo
  "Máy chủ đã khởi động lại…". Thấy lỗi này hàng loạt ⟹ restart lặp lại khi đang chép.
- **Tiền chép lời:** sổ `transcribe_usage` (tháng giờ VN), chi tiết `transcribe_runs`. Tra: `sudo -u <user> <binary> admin
  usage [--email …]`; đối soát `--recompute` (chỉ xem), `--apply` là thao tác ghi. Cấp gói `admin set-plan` cũng là ghi.
- **Chép lời tạm ngưng toàn hệ thống** (503 "Chép lời đang tạm ngưng"): key OpenAI hỏng hoặc hết credit — tìm
  `transcript.key_invalid` trong `audit_events` và dòng "key OpenAI của server không dùng được" trong journal.
- **Đĩa:** dữ liệu người dùng ở `/opt/scribe-be/data/<userId>/` (`recordings/`, `uploads/`, `outputs/`, `tmp/*.part`).
- **`lsof -ti :PORT` liệt kê cả client** — dùng `ss -ltnp` hoặc `-sTCP:LISTEN` trước khi kết luận hay kill gì.
- **Bản sao lưu** nằm ở `/root/scribe-backup-<YYYYMMDD-HHMMSS>/` trên chính .65 (`scribe-be.prev`, `dist/`,
  dump Mongo). Bản trước 16/10/2026 nằm ở `/root/onescribe-backup-*`. Không có cron backup cho Scribe.
- **Đổi tên 16/10/2026 12:43:** db `onescribe` → `scribe` (user Mongo `scribe`, chỉ `readWrite`+`dbAdmin` trên `scribe`),
  `onescribe-be` → `scribe-be` (user hệ thống `scribe`). Bản cũ (service, db `onescribe`, user Mongo/hệ thống
  `onescribe`, thư mục) đã xoá 16/10/2026 theo yêu cầu user; chỉ còn dump `/root/onescribe-backup-20261016-124301/`. Tạo user/db Mongo mới cần
  tài khoản quản trị `mongo_admin_*` của entry `SSH_SERVER_STAGING` (mật khẩu qua stdin vào tệp 600, không argv).
- **Đổi đăng nhập + domain 16/10/2026 13:27:** đăng nhập qua hoandat-auth (client `scribe-web` confidential, secret ở
  `/opt/scribe-be/.oidc-web-secret` và `SCRIBE_WEB_CLIENT_SECRET` trong `/var/www/hoandat-auth/.env`; `scribe-mobile` cho
  iPhone), vhost `/etc/nginx/conf.d/scribe.conf` (443 với chứng chỉ chung `/etc/pki/nginx/server.{crt,key}`). Domain
  cũ `scribe.inetdev.io.vn` đã bỏ (vhost và DNS xoá, không 301). Firebase không còn.
- **Máy dev:** hook của Claude chặn lệnh có chữ `target` (đường dẫn build) ⟹ build prod với
  `CARGO_TARGET_DIR=<thư mục scratch>`. `tar` trên macOS kèm xattr (cảnh báo `LIBARCHIVE.xattr` ở server, vô hại) ⟹
  nén bằng `COPYFILE_DISABLE=1 tar --no-xattrs …`, giải nén bằng `tar --no-same-owner` và `rsync --chown=root:root`
  (lần 15/10 FE mang chủ `501:games` của máy Mac).
- **Key OpenAI của server:** ghi/đổi bằng stdin, không qua argv: `jq -r '.tokens[]|select(.service=="api.openai.com")
  |.token' "$ACCESS_TOKENS_JSON" | $S/prod-ssh.sh "umask 077; cat > $OPENAI_KEY_PATH.tmp; chown <user>: …; mv -f …"`
  rồi restart (key chỉ đọc lúc khởi động). Đó là thao tác ghi.

## 5. Kết luận từ dữ liệu prod

Kết quả âm tính (*rỗng / 0 / không thấy lỗi*) mặc định là **chưa kết luận được** tới khi trả lời đủ: đã đọc đúng nguồn
chưa (đúng db `scribe` chứ không phải db khác trên cùng mongod, đúng unit `scribe-be`), phạm vi thời gian nào,
nguồn đó có thật sự ghi loại sự kiện này không (chunk upload thành công chỉ log mức debug; `/api/health` không log).
Báo phép đo đã làm trước, rồi mới tới kết quả. Còn mắt xích chưa đo thì gọi là giả thuyết.

## 6. Khi chuẩn bị thao tác GHI (chỉ khi user đã yêu cầu tường minh)

- **Trước deploy:** `cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`,
  `cd web && npm run lint && npm run typecheck && npm run check:tokens && npm test` sạch; `git status` sạch, đã push.
  Đổi dữ liệu `transcripts` ⟹ chạy `scripts/transcripts-audit.js` trước.
- **Build** (từ gốc repo):

  ```bash
  set -a; . ./.env.production; set +a
  B="$(mktemp -d)"   # thư mục build riêng
  CARGO_TARGET_DIR="$B" cargo zigbuild --release --target x86_64-unknown-linux-musl -p scribe-be
  (cd web && npm run build)
  ```

- **Ngay trước khi thay bản đang chạy** (một lệnh SSH, dừng nếu hàng đợi khác 0):

  ```bash
  TS=$(date +%Y%m%d-%H%M%S)
  $S/prod-ssh.sh "set -e; B=/root/scribe-backup-$TS; mkdir -p \$B; chmod 700 \$B
    cp -p $DEPLOY_APP_DIR/$DEPLOY_SERVICE \$B/scribe-be.prev; cp -a $DEPLOY_WEB_ROOT \$B/dist" </dev/null
  ```

  Hàng đợi rỗng và dump `transcripts` cần `MONGO_URI` — chạy qua `prod-mongosh.sh` (đếm) hoặc truyền URI qua stdin
  (`mongodump --uri` trong một script gửi bằng stdin), không đặt URI vào argv trên máy dùng chung.
- **Thay bản:** đẩy binary bằng stdin (`cat … | prod-ssh.sh 'cat > /tmp/scribe-be.new'`), FE bằng
  `COPYFILE_DISABLE=1 tar -C web/dist -czf - . | prod-ssh.sh 'mkdir -p /tmp/scribe-fe && tar -xzf - -C /tmp/scribe-fe'`,
  rồi theo đúng khối lệnh `install … && mv -f … && rsync -a --delete … && systemctl restart` ở
  `docs/deploy-web-production.md` §Deploy (làm tay). So `sha256sum` binary hai đầu.
- **Nghiệm thu:** §Verify sau deploy + `/api/health`, `NRestarts`, journal không có `ERROR` mới sau 30 phút; FE trả
  đúng `assets/index-*.js` vừa build.
- **Rollback code:** `install -o $DEPLOY_USER -g $DEPLOY_USER -m 700 /root/scribe-backup-<ts>/scribe-be.prev
  $DEPLOY_APP_DIR/$DEPLOY_SERVICE.new && mv -f … && systemctl restart scribe-be`; FE: `rsync -a --delete
  /root/scribe-backup-<ts>/dist/ $DEPLOY_WEB_ROOT/`. Rollback code không đảo ngược dữ liệu (bản cũ bỏ qua
  `transcript_docs`).
- **Khôi phục dữ liệu** chỉ khi dữ liệu hỏng và user đồng ý: `mongorestore --drop --nsInclude "scribe.<coll>"` từ
  dump trong thư mục sao lưu. Ghi tên thư mục sao lưu vào báo cáo.
- **Dọn sao lưu cũ** (> 14 ngày hoặc khi user xác nhận ổn định) cũng là thao tác ghi: hỏi trước.
