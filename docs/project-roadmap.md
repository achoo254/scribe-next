# Project Roadmap — Scribe

Cập nhật 2026-10-24. Chi tiết kiến trúc: [codebase-summary.md](./codebase-summary.md).

## Scribe Web — plan `260711-1657-scribe-web-media-rust` — ✅ HOÀN TẤT (5/5 phase)

Web hóa Scribe làm ghi + cắt media (BỎ AI transcribe — box .65 không GPU). Live tại `https://scribe.hoandat.com`.

| Phase | Nội dung | Trạng thái |
|-------|----------|-----------|
| 1 | Foundation & deploy gate — Cargo workspace + `core-split` + axum skeleton + **musl static binary chạy .65** (GATE) + FE scaffold | ✅ DONE — GATE passed (`/api/health` 200 qua CF) |
| 2 | Auth Google (Firebase) — `signInWithPopup` + Rust verify ID token (securetoken JWKS) + session cookie httpOnly + Mongo users + allowlist(mở)+audit | ✅ DONE — login Google thật OK |
| 3 | Mode B — Cắt file — upload chunked resumable + job queue + SplitWorker (core-split) + tải zip + FE | ✅ DONE (live) |
| 4 | Mode A — Ghi — getUserMedia/getDisplayMedia + Web Audio mix + MediaRecorder streaming + lịch sử + player (stream/Range) + cắt webm (remux+copy) | ✅ DONE (code+review-fix+deploy+E2E live) |
| 5 | Retention & hardening — sweep purge 14 ngày + quota 5GB + rate-limit real-IP + size cap 2GB + magic-byte + allowlist + security pass | ✅ DONE (code+review-fix+deploy) |

**Quyết định chốt (validate):** domain prod riêng · allowlist MỞ mọi tài khoản Google (audit-only) · retention 14 ngày · quota 5GB/user · size cap 2GB/file · cắt recording webm = remux+copy.

## Bổ sung ngoài plan (2026-07-12)

- ✅ **Refactor UI web bám 100% desktop** — theme dark của app (xanh lá/cam) + BrandMark + Be Vietnam Pro + shell 3 tab (thay giao diện trắng/xanh dương cũ). Đã thay bằng khung Localman ngày 2026-10-16.
- ✅ **Xoá recording thủ công** — `DELETE /api/files/:id` (ownership) + nút Xoá (xác nhận inline) ở lịch sử. Live.

## Bổ sung ngoài plan (2026-07-13)

- ✅ **Desktop rewire dùng `core-split`** — `src-tauri/src/splitter.rs` bỏ bản logic song sinh (~308 dòng), uỷ thác thuật toán cắt cho crate `core-split` (một nguồn chân lý chung desktop + server). Desktop giữ phần Tauri: resolve PATH sidecar (`ffmpeg::sidecar_path`, tái tạo đúng `relative_command_path` của tauri-plugin-shell) + batch dedup subdir + emit event. `split_file` giữ nguyên chữ ký (server BE **không đổi**); thêm `split_file_into(sub)` cho desktop. Regression test tự động (ffmpeg thật) trong `core-split/tests/split_file_ffmpeg.rs`. Verify: `cargo test -p core-split` 12/12 · `cargo clippy` sạch · desktop + workspace compile sạch. **Chờ QA runtime Mode B trên bản build Windows/macOS thật** (không automate được GUI).

## 2026-10-15 — web React, app iPhone, chép lời

- ✅ **Web viết lại bằng React** (plan `dattqh/261015-1524-web-react-rewrite`), parity với bản Vue, **đã lên prod** cùng ngày. Bản Vue đã xoá ngày 2026-10-16 (còn trong lịch sử git).
- ✅ **Chỉ còn hai môi trường**: dev local (Mongo dev trên lab 176) và prod `.65`; bỏ staging.
- ✅ **Audit**: access log mọi request + `audit_events` (TTL 180 ngày).
- ✅ **Chép lời bằng OpenAI** `gpt-4o-mini-transcribe`, key người dùng tự nhập, chỉ email trong `TRANSCRIBE_EMAILS` (hiện: email của chủ dự án). Đoạn 10–15' buổi họp tham chiếu khớp TurboScribe 84,9%. Đã lên prod.
- ✅ **Sửa cắt "Số phần"** trong `core-split` (trước đây luôn ra thêm một đoạn cuối ~800 byte hỏng), áp cho cả web và desktop.
- 🟡 **App iPhone** (plan `260728-1609-scribe-ios-app-ghi-nen-khoa-man-hinh`): code Phase 1–3 xong, self-test Simulator đạt; QA trên iPhone thật đang chờ. Cài bằng `cd web && pnpm run ios:device` (free provisioning, cài lại mỗi 7 ngày). Chi tiết: [ios-app.md](./ios-app.md).

## 2026-10-16 — QA iPhone, dọn repo, CI

- ✅ **QA iPhone thật** (iPhone 15 Pro Max, prod): đăng nhập Google, ghi khi khoá màn hình, tắt app khi đang ghi, đăng xuất/đăng nhập lại khi còn đoạn chưa tải đều đạt. Chi tiết: [ios-app.md](./ios-app.md).
- ✅ **Bỏ GitLab hoàn toàn**: repo duy nhất là GitHub; thêm CI `.github/workflows/ci.yml` (chỉ kiểm: lint, typecheck, test web; clippy, test Rust). Deploy prod vẫn làm tay.
- ✅ **Xoá code cũ** `web/src-vue-legacy/` và `ios-spike/`.
- ✅ **Bỏ hoàn toàn app desktop** (Tauri + Vue + sidecar chép lời offline). Sản phẩm chỉ còn web và iPhone; lịch sử desktop vẫn còn trong git.
- 🟡 **Đề xuất kiến trúc đích** (Jev đã duyệt một phần): `plans/dattqh/reports/brainstorm-261016-0809-scribe-target-architecture-refactor.md`.

## 2026-10-16 — Bước 2: key server + đo phút, đăng nhập auth-next (plan `dattqh/261016-1114-server-key-metering-and-hoandat-auth`)

- ✅ **Phần A — key server + đo phút**: bỏ ô nhập key; chép lời dùng key OpenAI của server, giữ chỗ phút khi xếp hàng rồi quyết toán theo giây thật sự gửi; gói `free` 120 phút / `plus` 3.000 phút, quỹ free $10, trần tổng $30 mỗi tháng; CLI `scribe-be admin`. Restart không còn làm mất lượt chép.
- ✅ **Chống lạm dụng gói free** (Phase 9): 30 phút/ngày mỗi tài khoản, tài khoản mới chờ 24 giờ (bỏ 19/10/2026: mặc định 0, prod `TRANSCRIBE_FREE_DELAY_HOURS=0`), `admin block|unblock`, báo động qua bot chat.hoandat.com `@bot_scribe_bot`. Không giới hạn theo IP (người dùng bỏ: 4G chung IP nhà mạng).
- ✅ **Bỏ mảnh im lặng**: mảnh hoàn toàn im lặng (đỉnh < −30 dB) không gửi OpenAI, không tính phút.
- ✅ **7a — đổi tên hạ tầng**: service `scribe-be`, user `scribe`, `/opt/scribe-be`, `/var/www/scribe/dist`, db `scribe` (user Mongo riêng). Bản `onescribe*` đã xoá.
- ✅ **7b — đăng nhập auth-next + domain**: web (`scribe-web`) và iPhone (`scribe-mobile`) đăng nhập qua `auth.hoandat.com`, user cũ ghép theo email, phiên lưu Mongo thu hồi được; prod ở `scribe.hoandat.com` (Cloudflare SSL Full); bỏ Firebase và domain `scribe.inetdev.io.vn`. iPhone mới đã cài và kiểm trên máy thật.
- ✅ **7c — mở chép lời cho mọi người** (`TRANSCRIBE_OPEN=1`): bật 16/10 14:19 theo quyết định của chủ sản phẩm (Jev khuyên chờ vài ngày, 0,78). Gói free: 120 phút/tháng, 30 phút/ngày, tài khoản mới chờ 24 giờ, quỹ free $10, trần tổng $30.

## Scribe iOS — lộ trình App Store

**19/10/2026 — sẵn sàng nộp** (plan `dattqh/261019-1105-app-store-readiness`): trang `/privacy` và `/terms` công khai; "Xoá tài khoản" trong app (`DELETE /api/me`); `PrivacyInfo.xcprivacy`, `ITSAppUsesNonExemptEncryption`, chỉ iPhone; script `pnpm run ios:release` (archive + tải lên App Store Connect bằng key API); hồ sơ App Store và ảnh chụp ở `docs/app-store/`. Các bước phát hành: [ios-app.md](./ios-app.md#phát-hành-testflight--app-store). Backend có xoá tài khoản và trang pháp lý đã lên prod 19/10 (sao lưu trước deploy `20261019-121148`); tài khoản Developer đã có, app đã lên TestFlight (build `20261023.1211`, [deploy-web-production.md](./deploy-web-production.md)).

Phân tích ban đầu (trước 19/10):

Chi tiết và lập luận: phase 4 của plan iOS. Tóm tắt:

- **Cần tài khoản Apple Developer ($99/năm)**: profile 1 năm thay 7 ngày, TestFlight, Live Activity (đồng hồ + nút Dừng trên màn hình khoá), mở đường ghi màn hình bằng ReplayKit.
- **Giấy tờ khi nộp**: Privacy Manifest, App Privacy, ghi chú cho App Review về `UIBackgroundModes: audio`, xoá tài khoản trong app — đã làm 19/10 (xem trên).
- **Chặn thật nằm ở hạ tầng, không phải iOS**: allowlist đăng nhập đang mở cho mọi tài khoản auth-next có email đã xác minh; chặng Cloudflare↔origin là HTTPS nhưng chứng chỉ tự ký (SSL Full, chưa strict); server chỉ giữ file 14 ngày (nói rõ trong điều khoản). Chính sách riêng tư và điều khoản: có từ 19/10. (Đã xong 16/10: phiên lưu Mongo, thu hồi được.)
- **Rà mã iOS (2026-10-15), cả 4 mục đạt**: `AudioRecorder` không import Capacitor · Team ID chỉ có trong `web/scripts/ios-device.sh`, không có trong mã app hay `project.pbxproj` · chuỗi xin quyền micro nêu rõ mục đích kể cả khi khoá màn hình · không dùng API không công khai.
- **Thứ tự nếu quyết định lên Store**: trả $99 → profile 1 năm → Live Activity → TestFlight vài người thử → hạ tầng đủ điều kiện công khai → nộp.

## Trạng thái hiện tại

- **Web**: LIVE bản React tại `scribe.hoandat.com`, đăng nhập auth-next, giao diện theo khung Localman. Ghi (mic + màn hình/system) · Cắt file · lịch sử/phát lại · chép lời bằng key server có hạn mức (mở cho mọi người từ 16/10) · biên bản, dấu mốc, gửi biên bản sang chat (từ 16/10 22:31) · tìm kiếm toàn văn và hỏi đáp có trích mốc giây (từ 17/10 06:23) · tải tệp lên để chép lời, xoá nhanh có hoàn tác, xoá tất cả, thẻ gói và dùng lượng (từ 17/10 07:30) · gói free 200 MB, tự xoá sau 7 ngày; plus 5 GB, 14 ngày · rate-limit.
- **iPhone**: bản `com.hoandat.scribe` đăng nhập auth-next, đã kiểm trên máy thật (ghi khoá màn hình, tải lên, chép lời); còn cuộc gọi đến, AirPods, chế độ máy bay.

## Backlog / Next

| Ưu tiên | Hạng mục | Ghi chú |
|---------|----------|---------|
| ✅ P0 | **Refactor UI/UX theo format hệ sinh thái hoandat** (plan `dattqh/261016-1426-ui-ux-hoandat-ecosystem-refactor`) | Xong phase 1–5, web đã lên prod 16/10: khung Localman (top bar, account menu, footer), URL cho từng trang, từ điển tiếng Việt, Playwright sáng/tối. Plan đóng 16/10; QA tay trên iPhone bản cuối hoãn tới lần cắm máy sau |
| ✅ P1 | **Gói "Sau cuộc họp"** (plan `dattqh/261016-2133-after-meeting-package`): biên bản (tóm tắt + quyết định + việc cần làm, mốc giây) bằng `gpt-4.1-mini` khi bấm "Tóm tắt", đánh dấu khoảnh khắc khi đang ghi, liên kết chat.hoandat.com (xác nhận trong chat) và gửi biên bản tới chat riêng | **Lên prod 16/10 22:31.** Lượt ngày + trần tháng riêng, chi phí tính vào trần tổng như chép lời. Còn lại: đánh dấu từ màn hình khoá / AirPods đã có mã 17/10 (Now Playing, xem [ios-app.md](./ios-app.md)), còn thử trên iPhone thật + AirPods; chủ sản phẩm tự thử đầu cuối bấm deep link → xác nhận liên kết → gửi biên bản (cả trong WebView iPhone); đánh dấu khi ghi nền trên iPhone thật |
| ✅ P1 | **Gói "Kho bản ghi"** (plan `dattqh/261016-2257-recording-library`): tìm kiếm toàn văn mọi transcript, gõ có dấu hay không dấu (bấm kết quả là nhảy tới đúng giây), hỏi đáp trên cả kho hoặc một bản ghi bằng `gpt-4.1-mini`, có trích mốc giây | **Lên prod 17/10 06:23.** Lượt hỏi mỗi ngày riêng; trần chi phí tháng và trần tổng dùng chung với tóm tắt. Còn lại: chủ sản phẩm tự thử tìm kiếm và hỏi đáp thật trên prod sau khi đăng nhập |
| ✅ P1 | **Gói free: lưu trữ và tải tệp lên** (plan `dattqh/261017-0644-free-tier-storage-and-upload`): free 200 MB, tự xoá sau 7 ngày; xoá nhanh có hoàn tác, xoá tất cả, tải tệp âm thanh lên để chép lời, thẻ gói và dùng lượng | **Lên prod 17/10 07:30.** Phút chép lời free giữ 120/tháng. Lúc deploy chưa có tệp của user free nào nên không phải rút hạn tệp cũ |
| ✅ P2 | **Xuất phụ đề và tài liệu** (plan `dattqh/261017-1150-export-formats`): menu "Tải xuống" (TXT, SRT, VTT, Markdown, Word) ở Lịch sử ghi và màn hình đọc; Markdown/Word gồm biên bản, dấu mốc và lời chép có mốc | Xong và lên prod 17/10 (sao lưu trước deploy `20261017-123216`). Tạo ở máy chủ nên web và iPhone nhận cùng một tệp |
| ✅ P2 | **Dịch lời chép Việt ↔ Anh** (plan `dattqh/261017-1235-transcript-translation`): nút "Dịch" ở màn hình đọc chọn Tiếng Anh / Tiếng Việt, dịch từng câu giữ mốc giây bằng `gpt-4.1-mini`, xem Gốc / Bản dịch / Song song, câu sửa sau khi dịch có nhãn "bản dịch cũ" và nút "Dịch lại câu đã sửa"; menu "Tải xuống" chọn ngôn ngữ cho cả 5 định dạng | Xong và lên prod 17/10 (sao lưu trước deploy `20261017-130838`). Lượt dịch mỗi ngày riêng (`TRANSLATE_FREE_DAILY` 3 / `TRANSLATE_PAID_DAILY` 20); trần chi phí tháng và trần tổng dùng chung với tóm tắt và hỏi đáp. Biên bản trong tài liệu xuất giữ ngôn ngữ gốc |
| ✅ P1 | **Chép lời bằng ElevenLabs + phân biệt người nói** (plan `dattqh/261017-1530-elevenlabs-transcription`): chủ dự án chốt 17/10 sau bài nghe mù. Scribe v2 (0,22 USD/giờ) cho chép lời tệp; phiên iPhone chép một lần cho cả phiên; câu có mốc thật và người nói ("Người 1/2", đổi tên được, có trong tệp xuất và biên bản); trần giờ ElevenLabs mỗi tháng `ELEVENLABS_MONTHLY_HOURS` (25); hết credit thì dừng tới tháng sau và báo bot | Xong và lên prod 17/10 (sao lưu trước deploy `20261017-160739`). `TRANSCRIBE_PROVIDER=openai` giữ luồng cũ một bản phát hành |
| ✅ P1 | **Chuyển chép trực tiếp iPhone khỏi `gpt-4o-mini-transcribe` trước 26/2/2027** | **Backend đã lên prod; còn thử trên iPhone thật.** Chép trực tiếp chạy ElevenLabs Scribe v2 Realtime (0,39 USD/giờ, token dùng một lần do server cấp, tính vào phút, trần USD và sổ giờ ElevenLabs); `REALTIME_PROVIDER=openai` giữ OpenAI Realtime một bản phát hành. Đã thử từ Mac: cấp token qua đường mã backend, stream 10 giây ra câu tiếng Việt. Deploy cùng lúc với bản app iPhone mới (app 17/10 chỉ nói được với OpenAI). Sau một bản phát hành ổn định thì bỏ cả `REALTIME_PROVIDER=openai` và `TRANSCRIBE_PROVIDER=openai` |
| ✅ P1 | **Key riêng — người dùng tự nhập key API** (plan `dattqh/261019-0905-user-api-keys`): ElevenLabs, OpenAI, Google Gemini, Anthropic Claude; lưu mã hoá trên server, chỉ ghi; chọn nhà cung cấp cho chép lời tệp, chép trực tiếp, biên bản/hỏi đáp/dịch (kèm model); việc chạy bằng key riêng không tính hạn mức của Scribe. Server chọn được model chữ (`LLM_PROVIDER`/`LLM_MODEL`) | **Lên prod 19/10** (sao lưu trước deploy `20261019-103131`; tệp `USER_KEYS_SECRET_PATH` theo deploy doc, mục "Key riêng"). Gemini và Claude chưa thử với key thật |
| ✅ P1 | **Bỏ đa tenant, về đơn tenant `hoandat.com`** (plan `dattqh/261024-2129-single-tenant-npm-packages`; thay plan `dattqh/261024-1250-scribe-multi-tenant` và [ADR 0001](./decisions/0001-tenants-by-domain.md), nay superseded): bỏ tenant theo Host, danh bạ và API nội bộ của IdP, OIDC qua loopback, key AI niêm phong của tenant, lệnh admin theo tenant, nginx `scribe.<domain>`, trang "Scribe chưa bật". Giữ `tenantId: "primary"` ở dữ liệu, index `tenantId_idpSub_unique` và shape `GET /api/idp` (app di động đã phát hành đọc nó) | **Lên prod 24/10 23:31** (bản đa tenant `00240aa` chạy từ 17:35 cùng ngày; dữ liệu tenant khác đã xoá, khoá tenant đã huỷ trên server và iCloud). Bản ghi deploy: [deploy-web-production.md](./deploy-web-production.md), mục "Đơn tenant" |
| P1 | **QA app iPhone còn lại** — cuộc gọi đến, AirPods, máy bay | Cần máy thứ hai + AirPods |
| ✅ P1 | **Realtime transcript trên iPhone** (Phase 5 plan iOS) | Đã làm và deploy 17/10 (token tạm thời từ server, `REALTIME_EMAILS`, tính phút chung sổ). Còn đo trên máy thật: WebSocket khi khoá màn hình lâu, độ trễ, mất mạng giữa buổi |
| ✅ P2 | **QA thủ công web** — ghi mic, tiếng hệ thống, màn hình (video) thật, cắt file thật | Xong 19/10 trên Chrome của chủ dự án (tài khoản Plus): mic 40 s + dấu mốc giây 38, tiếng hệ thống 21,8 s, màn hình 17,9 s 3824×1616, cắt 2:00 theo phút + zip. Lỗi tìm thấy: cắt theo phút sinh đoạn cuối 0,02 s — sửa `45c53d1`, prod 10:48 |
| ✅ P3 | **Unit test** `RateLimiter` (window reset) | Xong (commit `3917615`): reset cửa sổ và thứ tự ưu tiên IP client |
| P3 | **Hạ tầng CF Full (strict) + firewall origin** | Đang SSL Full với chứng chỉ tự ký; .65 shared → hiện chấp nhận origin-bypass; phòng thủ dồn vào hạn mức + rate-limit + audit |

## Unresolved questions

- Chưa có (mọi quyết định business đã chốt ở validate log của plan).
