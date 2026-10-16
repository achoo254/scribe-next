# Code review — Scribe Web Phase 1 (foundation) + Phase 2 (auth Firebase)

Scope: file list do cook cung cấp (Rust workspace/core-split/scribe-be + web/ FE). Đối chiếu plan.md,
phase-01, phase-02 (bao gồm Red-team #4/#6/#9/#13, Validate #2). Verify trực tiếp: `cargo check --workspace`,
`cargo test -p core-split` (9/9 pass, tự chạy lại), `vue-tsc --noEmit` (pass, tự chạy lại), `curl` JWKS
endpoint thật để xác nhận shape `{keys:[{kid,n,e}]}` khớp code, `grep` Cargo.lock xác nhận không có
openssl/native-tls trong dependency graph (mongodb→rustls, jsonwebtoken→ring).

## Critical
Không có finding Critical. Auth core (jwks.rs verify RS256+aud+iss+exp+kid+email_verified, session.rs
HS256 key runtime chmod 600, middleware guard, allowlist order) đúng theo spec Phase 2 — xem chi tiết ở
"Đã verify đúng" cuối report.

## High

### H1 — Race điều kiện upsert user: thiếu unique index trên `firebaseUid` → tạo user trùng
`server/scribe-be/src/models/user.rs:24-30` dùng `find_one_and_update(... ).upsert(true)` theo filter
`firebaseUid` nhưng KHÔNG có unique index trên field này (grep toàn bộ `server/scribe-be/src` +
`deploy/`/`scripts/` — không có index creation nào). Đây là anti-pattern MongoDB đã biết: khi 2 request
upsert cùng filter chạy đồng thời và document chưa tồn tại, cả 2 có thể đều "not found" rồi đều insert →
2 document `_id` khác nhau cùng `firebaseUid`. Kịch bản thực tế: user double-click nút login trước khi
`loading` disable kịp (race client-side), hoặc mở 2 tab cùng lúc bấm "Đăng nhập Google" — mỗi lần Firebase
`signInWithPopup` trả token riêng nhưng cùng `sub`.
- **Hệ quả:** 2 session cookie với `sub` (user `_id`) khác nhau cho cùng 1 người → `/api/me` trả dữ liệu
  không nhất quán giữa 2 tab; ở Phase 3-5 quota 5GB/user + lịch sử gắn theo `_id` sẽ bị phân mảnh — về lâu
  dài đây là vector để 1 người thật lách quota (spam login đồng thời để tạo nhiều "user" cho cùng email).
- **Fix:** tạo unique index `db.users.createIndex({firebaseUid:1}, {unique:true})` lúc BE khởi động (1 lần,
  idempotent — `create_index` trong `db::connect` hoặc `main.rs` sau `db::connect`), và bắt lỗi
  `E11000 duplicate key` ở `upsert_and_get_id` → retry `find_one` (đọc lại document đã được request khác
  tạo) thay vì propagate lỗi 500.

### H2 — JWKS refetch không giới hạn tần suất khi `kid` lạ → DoS amplification qua endpoint public
`server/scribe-be/src/jwks.rs:80-102` (`key_for`): nếu cache miss/hết hạn HOẶC `kid` không khớp bất kỳ
key nào trong cache, code luôn gọi `refetch()` (HTTP GET tới Google) — không có cooldown/backoff độc lập
với rate-limit theo IP. `POST /api/auth/session` (`routes.rs:17`) là route PUBLIC, không cần auth trước.
Attacker gửi request với `idToken` giả có header `kid` ngẫu nhiên mỗi lần → mỗi request ép BE gọi ra ngoài
Google JWKS endpoint — tốn latency + có thể bị Google giới hạn IP nếu tần suất cao, và tự DoS chính BE
(mỗi request giờ phụ thuộc round-trip mạng ra ngoài thay vì reject nhanh tại chỗ). Phase 5 rate-limit theo
CF real-IP sẽ giảm nhưng KHÔNG đóng hoàn toàn (1 IP đơn cũng đủ flood trước khi Phase 5 lên, và rate-limit
theo IP không ngăn được việc mỗi request hợp lệ về rate vẫn kích refetch).
- **Fix:** thêm cooldown độc lập theo kid — ví dụ lưu `last_refetch_at` (không phân biệt kid), nếu đã
  refetch trong N giây gần nhất (vd 10s) mà vẫn miss → trả lỗi ngay (Unauthorized) thay vì refetch lại.
  Đây là fix rẻ, không đụng contract, nên áp trước khi mở public endpoint kể cả trước khi Phase 5 xong.

## Medium

### M1 — `.session-key` (fallback path dev non-unix) không có trong `.gitignore`
`server/scribe-be/src/config.rs:75-84` fallback path khi build trên Windows/dev là `.session-key`
(relative, cwd). Grep `.gitignore` (root + `server/scribe-be/`) không thấy pattern `.session-key`/`*.key`.
Nếu dev chạy binary từ repo root rồi lỡ `git add -A`, secret 32-byte ký session có thể lọt vào commit (rủi
ro thấp — chỉ ảnh hưởng session cục bộ dev, không phải secret production — nhưng nên chặn từ gốc).
- **Fix:** thêm `.session-key` vào `.gitignore` gốc (cạnh `.env`/`.env.*` sẵn có).

### M2 — FE cache trạng thái đăng nhập (`ready`) không bao giờ re-validate giữa phiên SPA
`web/src/router.ts:15-19`: `ready.value` chỉ được set `true` một lần (lần đầu load); các navigation sau
KHÔNG gọi lại `/api/me` dù session cookie đã hết hạn (`session_ttl_secs` 12h ở `config.rs:41`) hoặc bị BE
xoá phía server. Không phải lỗ hổng bảo mật (mọi API bảo vệ vẫn re-check cookie ở `require_auth` — chỉ FE
router-guard bị stale), nhưng gây UX xấu: user tưởng đã login (route `/` hiển thị bình thường) nhưng gọi
API bảo vệ sẽ nhận 401 không rõ ràng nếu chưa có xử lý global 401→login redirect (chưa có ở phase này).
- **Fix (có thể hoãn tới Phase 3 khi có API thật cần global 401 handler):** thêm interceptor fetch dùng
  chung, bắt 401 → set `user.value = null` + điều hướng `/login`.

## Low

### L1 — Deviation so với plan Phase 1 bước 8: chưa cài design-system design system
Plan (`phase-01-foundation-deploy-gate.md:64`) chỉ định cài `@hobui/ui @hobui/tokens vuetify
@tabler/icons-vue`; `web/package.json` hiện chỉ có `firebase`, `vue`, `vue-router` — chưa có design system. UI hiện
tại (`LoginView.vue`, `HomeView.vue`, `style.css`) dùng CSS thuần tự viết, không hardcode design token sai
(không vi phạm rule `design-system`) — chỉ là CHƯA tích hợp. Có thể là chủ đích hoãn tới khi có
UI thật (Phase 3+); flag để cook xác nhận có chủ đích hay thiếu sót.

### L2 — Red-team #13 fix bằng cơ chế khác chữ plan nhưng đã verify hoạt động
Plan yêu cầu `[workspace] default-members = [...]` loại `scribe-be` khỏi check mặc định; code KHÔNG có
`default-members` trong `Cargo.toml` root. Thay vào đó `config.rs` dùng `option_env!` (không phải `env!`)
với fallback — đã tự chạy `cargo check --workspace` xác nhận PASS không cần secret nào. Tương đương mục
tiêu, chỉ khác cách làm — không phải bug, note lại để tránh cook/lead nhầm là thiếu sót khi đối chiếu plan
text theo câu chữ.

### L3 — Race hi hữu lúc first-boot cho `load_or_create_key`
`session.rs:21-38`: đọc-rồi-ghi không atomic; nếu 2 process BE khởi động đồng thời lần đầu (chưa có file
key) có thể ghi đè nhau → 2 tiến trình có key khác nhau → session issue bởi 1 instance không verify được ở
instance kia. Chỉ xảy ra đúng 1 lần (first-ever boot) và với model deploy hiện tại (systemd, 1 instance,
`Restart=on-failure` tuần tự) khả năng trùng thời điểm khởi động cực thấp — không block, chỉ note.

## Đã verify ĐÚNG (không phải finding, liệt kê theo acceptance criteria user yêu cầu xác nhận)

- **jwks.rs (`server/scribe-be/src/jwks.rs:65-77`):** RS256 + `set_audience(["$FIREBASE_PROJECT_ID"])` +
  `set_issuer("https://securetoken.google.com/$FIREBASE_PROJECT_ID")` + `leeway=60` + `email_verified` reject
  đúng thứ tự (check trong `verify_id_token`, TRƯỚC khi trả claims ra ngoài cho allowlist dùng — khớp
  red-team #9). `Validation::new(Algorithm::RS256)` giới hạn `algorithms=[RS256]` → chặn alg-confusion
  (không thể giả `alg:none` hay HS256-với-public-key-làm-secret). Kid-miss → refetch (đúng key rotation,
  không hard-fail) — xem H2 về mặt lạm dụng.
- **JWK endpoint xác nhận LIVE:** `curl https://www.googleapis.com/service_accounts/v1/jwk/securetoken@...`
  trả đúng `{keys:[{kid,n,e,...}]}`, có `Cache-Control: max-age=...` — khớp struct `Jwk`/`JwkSet` và
  `cache_max_age()` ở `jwks.rs:123-133`.
- **session.rs:** key sinh runtime `rand::thread_rng()` (CSPRNG) 32 byte, ghi file + `chmod 600` (unix) —
  đúng red-team #4, không bake compile-time. HS256 issue/verify dùng `Validation::new(Algorithm::HS256)`
  (mặc định `validate_exp=true`) → hết hạn tự reject đúng.
- **auth.rs:** allowlist (`config.rs:50-64`) chỉ gọi SAU `email_verified` (thứ tự đúng trong
  `create_session:36-42`); domain-match dùng `rsplit('@').next()` (lấy đúng phần SAU dấu `@` CUỐI CÙNG) —
  thử case `a@b@evil.com` → domain nhận diện là `evil.com` (đúng ngữ nghĩa domain thật, KHÔNG bị lách bằng
  local-part chứa `@`). Cookie `HttpOnly`+`Secure`+`SameSite=Lax` (`auth.rs:88-96`). Middleware
  `require_auth` (`routes.rs:11-13`, `route_layer`) chỉ áp cho router "protected" (`/api/me`), không áp
  nhầm sang public — verify bằng đọc code + cargo check pass (không lỗi extractor order Json-last).
- **config.rs:** `option_env!` (không `env!`) → `cargo check --workspace` tự chạy lại PASS không cần secret
  (H2/L2 note ở trên). Allowlist rỗng = mở (đúng Validate #2 user chốt).
- **Musl-compat:** `grep openssl|native-tls` trên `Cargo.lock` → 0 match. `mongodb 3.8.0` deps có `rustls`
  (không `openssl`). `jsonwebtoken 9.3.1` deps có `ring` (không openssl). `reqwest` khai báo tường minh
  `default-features=false, features=["json","rustls-tls"]`. Sạch cho musl static.
- **core-split vs `src-tauri/src/splitter.rs`:** đối chiếu từng hàm — `compute_segment_time`,
  `size_cut_times` (parse csv `pts_time,pos,flags`, logic `next_boundary`/keyframe-gộp), `is_segment`,
  `cleanup_segments`, `list_outputs` — logic THUẦN GIỐNG HỆT bản gốc (copy 1-1, chỉ đổi ký hiệu enum thay
  string criterion). `split_file`/`run_ffmpeg` (`split.rs:101-148`): đọc `-progress pipe:1` qua
  `BufReader::lines()` trên stdout TRONG task chính, đồng thời `tokio::spawn` task riêng đọc hết stderr
  (`read_to_string`) — đây là pattern ĐÚNG để tránh deadlock 2 pipe (nếu chỉ đọc stdout mà không đọc stderr
  song song, ffmpeg có thể block khi pipe stderr đầy). Khác bản gốc (Tauri `CommandEvent` channel đã tự lo
  việc này ở tầng plugin) nhưng hành vi tương đương, không có sai lệch logic cắt. `new_command()`
  (`lib.rs:89-98`) set `CREATE_NO_WINDOW` trên Windows qua method có sẵn của `tokio::process::Command`
  (không cần import `CommandExt` — đã tự build/test xác nhận compile). `cargo test -p core-split` tự chạy
  lại: 9/9 pass.
- **FE flow (`use-auth.ts`, `router.ts`, `firebase.ts`):** `signInWithPopup` → `getIdToken()` →
  `POST /api/auth/session` (`credentials:"include"`) → set `user.value`; `logout()` gọi cả
  `/api/auth/logout` (BE xoá cookie) VÀ Firebase `signOut()` — đúng yêu cầu "xoá cả 2". Router guard gọi
  `/api/me` (qua `fetchMe`), redirect `/login` khi `requiresAuth` mà chưa có user — đúng. `firebase.ts`
  gán `apiKey` qua bracket-index tránh false-positive secret-scan (không phải hardcode secret — apiKey
  Firebase Web vốn public theo thiết kế Google).
- **Build/typecheck tự chạy lại:** `cargo check --workspace` OK; `cargo test -p core-split` 9/9 OK;
  `npx vue-tsc --noEmit` (trong `web/`) OK không output lỗi.

## Unresolved questions
1. H1 (unique index) có cần chặn GATE Phase 2 hay để dồn vào Phase 5 hardening cùng các index khác? Đề
   xuất: fix ngay (rẻ, 1 dòng `create_index` + 1 nhánh bắt lỗi duplicate-key) trước khi mở public vì đây là
   endpoint không cần đăng nhập trước.
2. H2 (JWKS refetch cooldown) — mức cooldown hợp lý bao nhiêu giây? Đề xuất 10-30s, cook quyết theo mức độ
   chấp nhận trễ khi Google thật sự xoay `kid` (rotation thực tế theo `Cache-Control` header ~ nhiều giờ).
3. L1 (design system chưa cài) — chủ đích hoãn hay thiếu sót cook quên? Cần xác nhận trước khi build UI thật ở Phase 3.

**Status:** DONE_WITH_CONCERNS
**Summary:** Auth core (jwks/session/middleware/allowlist) đúng theo spec + red-team fix; core-split khớp
splitter.rs gốc, không sai lệch hành vi; musl-compat sạch (rustls/ring, không openssl); build/test tự verify
lại pass. 2 High (race duplicate user do thiếu unique index; JWKS refetch không cooldown → DoS amplification
qua endpoint public) nên fix trước khi mở public. 3 Medium/Low là hygiene/UX, không block.
**Concerns/Blockers:** H1, H2 nên fix trước khi deploy Phase 1 GATE lên .65 công khai; còn lại không block.
