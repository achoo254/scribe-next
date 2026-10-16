# Đề xuất kiến trúc đích và lộ trình refactor Scribe (gửi Jev quyết định)

Ngày 2026-10-16. Trạng thái: **chờ quyết định của Jev, chưa triển khai gì**.
Ký hiệu: **[V]** đã xác minh (code, lệnh, REA capture) · **[S]** suy luận/nhận định · **[N]** kiến thức bên ngoài chưa kiểm trong phiên này.

## 1. Brainstorm contract

- **Outcome:** một bộ quyết định đã được Jev chốt về auth, nhà cung cấp chép lời và mô hình chi phí, Free/Paid, phạm vi tính năng tiếp theo, cấu trúc code và dọn legacy; đủ để lập plan triển khai.
- **Constraints:** ưu tiên app iPhone (đã chốt 2026-10-15); chỉ hai môi trường dev local và prod `.65`; thay đổi prod cần người dùng đồng ý; không thay đổi hệ thống trước khi Jev duyệt.
- **Non-goals:** không viết code, không đổi cấu hình, không deploy trong giai đoạn này; không mở rộng web ngoài những gì iOS cần.
- **Acceptance criteria:** mỗi điểm quyết định có đáp án Jev kèm xác suất, ghi trong mục 6; plan triển khai chỉ lập sau đó.

## 2. Khảo sát app tham khảo (REA MCP, 2026-10-16)

| Nguồn | Kết quả |
|---|---|
| TurboScribe `turboscribe.ai/pricing` | **Không khảo sát được** [V]: Cloudflare trả 403 + trang "Performing security verification" cho trình duyệt tự động (evidence `ev_cdad37a9…`). Không vượt qua lớp chống bot. |
| Otter.ai `/pricing` | Capture accessibility đầy đủ (evidence `ev_454946b4…`) [V] |
| Notta `/en/pricing` | Capture accessibility đầy đủ (evidence `ev_961dffde…`) [V] |

Chỉ khảo sát được trang công khai (pricing). Luồng upload/xử lý trong app cần đăng nhập nên **không quan sát được**; mọi nhận định về kiến trúc nội bộ của họ dưới đây đều không đưa ra.

**Mô hình Free/Paid quan sát được [V]:**

| | Otter | Notta |
|---|---|---|
| Free | 300 phút/tháng, tối đa 30 phút/cuộc, 3 file import trọn đời, lịch sử 25 bản gần nhất, AI Chat 20 câu/tháng | 120 phút/tháng, tối đa 3 phút/bản ghi, 50 file upload/tháng, 10 tóm tắt AI/tháng |
| Pro (năm) | $8,33/tháng: 1.200 phút, 90 phút/cuộc, 10 file import/tháng, lưu không giới hạn, từ vựng riêng | $8,17/tháng: 1.800 phút, 5 giờ/bản ghi, 100 file/tháng, 100 tóm tắt, export, dịch, từ vựng riêng |
| Business | $19,99: không giới hạn ghi, 6.000 phút import | $16,67: chép không giới hạn, 200 file/tháng |
| Đơn vị đo | phút/tháng, không cộng dồn; trần độ dài mỗi cuộc; số file import | phút/tháng; trần độ dài; số file; số lần tóm tắt; "AI Credits" cho tính năng AI, chỉ trừ khi ra kết quả |
| Thanh toán | thẻ trên web + in-app purchase Apple/Google | thẻ trên web + in-app purchase Apple/Google; Enterprise chuyển khoản |
| Hạ cấp | — | dữ liệu giữ, Free chỉ xem 3 phút đầu của transcript |

**Tính năng chung cả hai liệt kê [V]:** phát lại đồng bộ highlight từng từ, sửa transcript và tên người nói, nhận diện người nói, tìm kiếm, thư mục, chia sẻ link, export (txt/docx/pdf/srt), tóm tắt AI, hỏi đáp AI trên transcript, app iOS/Android, từ vựng riêng, MCP server.

**Nhận định [S]:** cạnh tranh không nằm ở chép lời thô mà ở *quản lý transcript + AI trên transcript*, và đơn vị bán là **phút chép lời/tháng**. Scribe hiện chỉ có chép lời thô, không có thực thể "phút".

## 3. Hiện trạng (repo scribe-next) [V]

- **3 app:** desktop Tauri+Vue (offline PhoWhisper hoặc OpenAI), web React + Rust/axum + Mongo (live prod), iPhone Capacitor 8 + plugin Swift (Phase 1–3 code xong, chờ QA máy thật; Phase 5 realtime chưa có code).
- **Chép lời bị viết hai lần:** Python `sidecar/transcriber/openai_transcriber.py:19-60` và Rust `server/scribe-be/src/transcribe/chunking.rs`, `openai.rs`; cùng hằng số và thuật toán, khác cách dò tiếng (silero VAD vs `silencedetect`). Crate chung duy nhất là `core-split`.
- **Data model transcript:** một transcript/file, `sentences[{start,end,text}]`; không có word timestamps (web), speaker, tiêu đề, thư mục; không có PATCH. Tab Transcript web là placeholder.
- **Key OpenAI:** người dùng tự nhập, chỉ giữ RAM; allowlist `TRANSCRIBE_EMAILS` nhúng lúc compile (`config.rs:56`), đổi phải build lại.
- **Quota:** chỉ 5 GB lưu trữ; rate-limit in-memory theo IP. Không có plan, phút, ledger, trần chi phí.
- **Auth:** Firebase → session JWT HS256 12h, không thu hồi được; allowlist mở; không CSRF token.
- **Legacy:** `web/src-vue-legacy/` (23 file, 2.663 dòng, plan React yêu cầu xoá sau parity); `ios-spike/` (8 file, 993 dòng, Phase 0 đã xong).
- **CI:** `.gitlab-ci.yml` chỉ build + deploy, không test/lint/typecheck; docs deploy mâu thuẫn GitHub vs GitLab. Test: Rust ~29, web 23 file vitest, `src-tauri` và sidecar 0, Swift 0.
- **Mâu thuẫn còn mở:** report realtime ghi giá $0,003 và $0,006/phút ở hai chỗ.

## 4. Repo hệ sinh thái HoanDat [V]

| Repo | Có gì liên quan | Tái dùng | Không mang sang |
|---|---|---|---|
| **hoandat-auth** (`auth.hoandat.com`, Node/oidc-provider, Mongo, `.65`) | OIDC Code+PKCE bắt buộc, access token ES256 15 phút, refresh xoay vòng + thu hồi, trang `/account` thu hồi thiết bị, client native PKCE (mẫu `chat-mobile` dùng custom scheme), chỉ Google/GitHub | Thay Firebase: giải quyết session không thu hồi được | Không có billing/allowlist/SDK; client và resource mới phải thêm bằng code trong `src/oidc/clients.ts`, `resources.ts`; chưa có Sign in with Apple |
| **localman-next** (Node/Express/Mongo, React 19) | `packages/shared/src/plan-limits.ts` (FREE/PRO), `billing/plan-guard.ts`, CLI `set-user-plan.ts`, ADR-0002 "không hiện giá, Pro 30 ngày tự động"; token CSS (Scribe đang copy, giá trị trùng); API client, i18n, check-tokens, feature flag | Pattern plan-limits + guard + CLI admin; pattern API client/i18n | Tenancy, mock runtime, MCP, deploy pm2 |
| **chat-next** (Go + Postgres) | Tích hợp hoandat-auth web/mobile/desktop, WebSocket gateway, media upload S3 + quota byte | Tài liệu tích hợp hoandat-auth cho mobile làm mẫu | Không có AI/LLM/STT, không billing; stack khác |

## 5. Điểm quyết định gửi Jev

Mỗi mục: phương án, trade-off, đề xuất của tôi.

### D1. Auth
- **A. Giữ Firebase, thêm session lưu DB thu hồi được.** Ít việc nhất; vẫn lệ thuộc Firebase, không vào SSO HoanDat.
- **B. Chuyển sang hoandat-auth OIDC** (web confidential + iPhone native PKCE). Thu hồi được, SSO hệ sinh thái, đã có mẫu chat-next; phải sửa repo hoandat-auth (client + resource), viết verify ES256 bằng Rust, migrate user theo email; iOS lên Store có thể cần Sign in with Apple [N].
- **C. Để nguyên tới khi lên App Store.** Không tốn gì; rủi ro session 12h vẫn còn.
- Đề xuất: **B**, làm trước khi mở Free/Paid. Giả định chịu tải: email Google trùng giữa hai hệ để ghép user. Hỏng đầu tiên khi: Apple yêu cầu Sign in with Apple mà IdP chưa có.

### D2. Ai trả tiền OpenAI
- **A. Giữ key người dùng tự nhập.** Không chi phí cho chủ sản phẩm; không bán được gói, trải nghiệm iPhone kém (phải dán key).
- **B. Key của server, đo phút theo người dùng.** Điều kiện cần cho Free/Paid; chủ sản phẩm chịu ~$0,003/phút [V từ README]; phải có ledger + trần chi phí trước khi mở.
- **C. Lai: server key cho người được cấp quota, vẫn cho nhập key riêng.**
- Đề xuất: **B** (giữ đường key riêng trên desktop). Hỏng đầu tiên khi: lạm dụng vì allowlist mở → cần trần phút cứng.

### D3. Free/Paid giai đoạn này
- **A. Chưa bán: quota phút/tháng theo plan trong DB, CLI admin cấp plan, không UI giá** (giống localman ADR-0002). Rẻ, đo được hành vi thật.
- **B. Bán ngay**: trên iPhone, gói số phải qua In-App Purchase [N, App Store Guideline 3.1.1]; web có thể SePay/VietQR. Nặng: StoreKit, webhook, đối soát.
- **C. Không giới hạn gì.** Rủi ro chi phí.
- Đề xuất: **A**, mức Free tham khảo Otter/Notta (vd 120–300 phút/tháng, trần độ dài mỗi bản ghi).

### D4. Phạm vi tính năng kế tiếp (iOS-first)
- **A. QA iPhone thật → quản lý transcript** (tiêu đề, đổi tên, tìm kiếm, xem/sửa, export txt/srt/docx) **→ tóm tắt AI**.
- **B. Realtime transcript khi khoá màn hình (Phase 5) trước.** Hấp dẫn; chưa có code, giá còn mâu thuẫn, cần iPhone để đo WebSocket nền.
- **C. Diarization (người nói) trước.** `gpt-4o-mini-transcribe` không trả speaker [S]; cần provider khác.
- Đề xuất: **A**; B sau khi QA máy thật xong.

### D5. Trùng lặp logic chép lời
- **A. Tách crate Rust `core-transcribe`** dùng chung server + desktop, bỏ đường OpenAI Python. Một nguồn chân lý; desktop phải gọi crate thay sidecar.
- **B. Desktop gọi API server để chép.** Mất offline-first của chế độ OpenAI? (chế độ PhoWhisper offline vẫn còn) và buộc desktop đăng nhập.
- **C. Giữ trùng lặp.** Rẻ hôm nay; desktop ít ưu tiên.
- Đề xuất: **C hiện tại** (desktop không phải ưu tiên), ghi nợ; làm A khi chạm lại desktop.

### D6. Dọn legacy
- Xoá `web/src-vue-legacy/` (đúng plan React) và `ios-spike/`; sửa tham chiếu `biome.json`, docs. Đề xuất: **có**, một commit riêng, sau khi người dùng đồng ý.

### D7. CI gate
- Thêm job test + lint + typecheck (Rust `cargo test`, web `npm run lint/typecheck/test`) vào pipeline thật đang dùng; chốt GitHub hay GitLab và sửa docs mâu thuẫn. Đề xuất: **có**, trước refactor auth.

## 6. Quyết định của Jev

Gọi `jev-latest` (`api.typesafe.ai/v1/systemone`) hai lần, 2026-10-16. Lần 1: state là mục 1–5 nguyên văn, có cả dòng "Đề xuất". Lần 2: cùng tài liệu nhưng đã xoá mọi dòng đề xuất, để kiểm tra Jev có chỉ chọn theo đề xuất hay không. Kết quả thô nằm trong scratchpad của phiên, không commit.

| Điểm | Lần 1 (có đề xuất) | Lần 2 (trung lập) | Kết luận |
|---|---|---|---|
| D1 Auth | B 1,00 | B 0,85 (A 0,12) | **Duyệt B**: chuyển sang hoandat-auth |
| D2 Ai trả OpenAI | B 1,00 | B 0,93 | **Duyệt B**: key server, đo phút, có trần |
| D3 Free/Paid | A 1,00 | A 0,99 | **Duyệt A**: quota phút, chưa bán |
| D4 Phạm vi kế tiếp | A 1,00 | A 1,00 | **Duyệt A**: QA iPhone → quản lý transcript → tóm tắt AI |
| D5 Trùng lặp chép lời | C 1,00 | A 0,53 / C 0,46 | **Chưa quyết**: hai lần ra hai đáp án, lần 2 gần như chia đôi |
| D6 Dọn legacy | xoá cả hai 1,00 | xoá cả hai 0,98 | **Duyệt**: xoá cả hai (vẫn cần người dùng đồng ý trước khi xoá) |
| D7 CI gate trước auth | có, trước auth 0,98 | không 0,52 / để sau 0,38 / trước auth 0,10 | **Không duyệt "trước auth"**: lần 2 chỉ 0,10 cho làm trước. Có làm hay không thì chưa rõ |
| Bước ngay sau dọn legacy | — | QA iPhone 0,57 / auth 0,27 / metering 0,15 | Nghiêng về QA iPhone, độ tin cậy 0,43 |
| Lộ trình mục 7 hợp lý | 0,52 | 0,37 | **Không duyệt** nguyên lộ trình mục 7 |

Lần 1 có thiên lệch theo đề xuất: ở D5 và D7, đáp án đổi khi bỏ dòng đề xuất. Vì vậy chỉ coi các đáp án giữ nguyên ở cả hai lần là quyết định.

### Lộ trình sửa theo quyết định (chờ Jev duyệt lại)

Dọn legacy → QA iPhone thật → auth hoandat-auth → plan + metering phút + key server → quản lý transcript → tóm tắt AI → realtime. CI gate không nằm trước auth. D5 giữ nguyên hiện trạng cho tới khi có quyết định.

### Vòng 2 và 3 (08:45, sau QA iPhone)

State: tài liệu trung lập ở trên, cộng kết quả QA iPhone sáng 2026-10-16 (đăng nhập, ghi khoá màn hình, tắt app khi ghi, đăng xuất/đăng nhập lại đều đạt), hệ quả của D2 lên D5, số test hiện có, và phụ thuộc giữa các bước (quản lý transcript và metering chỉ cần `userId` nội bộ, không phụ thuộc nhà cung cấp đăng nhập).

| Điểm | Kết quả | Kết luận |
|---|---|---|
| D7 CI gate | bước 0 trước auth 0,96 | **Duyệt**: CI chạy test + lint + typecheck sẵn có, làm ở bước 0. Vòng 1 trung lập ra "không" 0,52 vì state khi đó chưa nêu chi phí (cỡ S) và số test có sẵn |
| D5 Trùng lặp chép lời | giữ 0,64 / crate chung 0,35 | **Nghiêng giữ nguyên, chưa chắc**. Hai vòng ra hai đáp án; giữ nguyên là phương án rẻ nhất để đổi ý sau |
| Bước sau bước 0 | quản lý transcript 0,54 | Khớp vòng 3 |
| Thứ tự đầy đủ | R3 0,38 / R2 0,36 / R1 0,20 / R4 0,06 | **Quản lý transcript đi trước** (R2 + R3 = 0,74). **Thứ tự auth và metering sau đó chưa quyết** (gần như chia đôi) |

### Lộ trình đã chốt tới đâu

1. Bước 0: CI gate + xoá `web/src-vue-legacy/` và `ios-spike/` (xoá cần người dùng đồng ý).
2. Quản lý transcript trên iPhone và web.
3. Auth hoandat-auth và metering phút + key server: thứ tự để quyết khi xong bước 2. Ràng buộc đã biết: nếu metering + key server đi trước auth, allowlist đang mở cho mọi tài khoản Google, nên trần phút là lớp chặn chi phí duy nhất.
4. Tóm tắt AI → realtime.

QA iPhone còn lại (cuộc gọi, AirPods, máy bay; pin 60 phút đang chạy) làm song song khi có điều kiện.

### Đổi tên sản phẩm thành scribe-next (người dùng yêu cầu 2026-10-16)

Mẫu của chat-next và localman-next: repo GitHub `*-next`, tên hiển thị ngắn, app id `com.hoandat.*`, domain `*.hoandat.com`, đăng nhập qua hoandat-auth. Các lớp: (1) trong repo: tên repo, package, docs, tên hiển thị; (2) bundle id iOS cũ → `com.hoandat.scribe`; (3) hạ tầng prod: domain, systemd, thư mục, nginx, database Mongo.

| Câu hỏi | Jev | Kết luận |
|---|---|---|
| Thương hiệu | hệ hoandat 1,00 | **Duyệt**: tên hiển thị "Scribe", id `com.hoandat.*` |
| Phạm vi làm ngay | lớp 1 + 2 0,88 | Duyệt lớp 1. Lớp 2 phải hỏi lại, xem dòng cuối |
| Domain | chuyển sang `scribe.hoandat.com` cùng đợt auth 0,96 | **Duyệt** |
| Database prod | đổi tên cùng đợt lớp 3, có sao lưu 0,99 | **Duyệt** |
| Bundle id, sau khi biết API key iOS của Firebase khoá theo bundle id cũ (`GoogleAuthPlugin.swift:129`) | cùng đợt auth 0,44 / ngay, chấp nhận đăng nhập hỏng 0,31 / ngay, sửa Firebase 0,25 (độ tin cậy 0,16) | **Chưa quyết**, chuyển người dùng |

### Quản lý transcript — Giai đoạn 1 (Jev, 2026-10-16 10:20)

State trung lập: data model hiện tại (một transcript cho mỗi file, phiên iPhone chỉ ghép khi xuất), xoá file ghi xoá luôn transcript (`file::delete_doc`), chép lời chỉ cho email được phép, tham khảo Otter/Notta.

| Câu hỏi | Jev | Kết luận |
|---|---|---|
| Đơn vị transcript | một transcript cho mỗi phiên ghi 0,89 | **Duyệt**: phiên iPhone nhiều đoạn gộp thành một; đổi data model và chuyển dữ liệu hiện có |
| Thời hạn lưu | xoá cùng audio 0,98 | **Duyệt**: giữ như hiện nay (khi người dùng xoá hoặc sau 14 ngày) |
| Người dùng | làm ngay cho nhóm email đang được phép 0,83 | **Duyệt**: tự mở rộng khi có bước tính phút |
| Nền tảng | thiết kế cho iPhone, web dùng chung mã 0,81 | **Duyệt**: chỉ kiểm web không vỡ giao diện |

## 7. Lộ trình đề xuất ban đầu (Jev KHÔNG duyệt nguyên văn, xem bản sửa ở mục 6)

| Bước | Nội dung | Tier |
|---|---|---|
| 0 | CI gate + dọn legacy + sửa docs deploy | S |
| 1 | QA iPhone thật (đã có backlog P1) | S (cần người dùng cầm máy) |
| 2 | Auth sang hoandat-auth (web + iOS), thu hồi session | L (2 repo) |
| 3 | Plan + metering phút + key server + trần chi phí | M |
| 4 | Quản lý transcript trên iPhone/web | M |
| 5 | Tóm tắt AI | M |
| 6 | Realtime transcript iPhone | L |

## Câu hỏi chưa giải quyết

- Giá realtime $0,003 hay $0,006/phút (report mâu thuẫn).
- Có cần Sign in with Apple khi lên Store — phụ thuộc quyết định có lên Store không.
- `TRANSCRIBE_EMAILS` trên prod hiện là gì (không đọc giá trị trong phiên này).
