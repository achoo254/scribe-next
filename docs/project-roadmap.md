# Project Roadmap — Scribe

Cập nhật 2026-10-16. Chi tiết kiến trúc: [codebase-summary.md](./codebase-summary.md).

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
- ✅ **Chép lời bằng OpenAI** `gpt-4o-mini-transcribe`, key người dùng tự nhập, chỉ email trong `TRANSCRIBE_EMAILS` (hiện: `quocdat254@gmail.com`). Đoạn 10–15' buổi họp tham chiếu khớp TurboScribe 84,9%. Đã lên prod.
- ✅ **Sửa cắt "Số phần"** trong `core-split` (trước đây luôn ra thêm một đoạn cuối ~800 byte hỏng), áp cho cả web và desktop.
- 🟡 **App iPhone** (plan `260728-1609-scribe-ios-app-ghi-nen-khoa-man-hinh`): code Phase 1–3 xong, self-test Simulator đạt; QA trên iPhone thật đang chờ. Cài bằng `cd web && pnpm run ios:device` (free provisioning, cài lại mỗi 7 ngày). Chi tiết: [ios-app.md](./ios-app.md).

## 2026-10-16 — QA iPhone, dọn repo, CI

- ✅ **QA iPhone thật** (iPhone 15 Pro Max, prod): đăng nhập Google, ghi khi khoá màn hình, tắt app khi đang ghi, đăng xuất/đăng nhập lại khi còn đoạn chưa tải đều đạt. Chi tiết: [ios-app.md](./ios-app.md).
- ✅ **Bỏ GitLab hoàn toàn**: repo duy nhất là GitHub; thêm CI `.github/workflows/ci.yml` (chỉ kiểm: lint, typecheck, test web; clippy, test Rust). Deploy prod vẫn làm tay.
- ✅ **Xoá code cũ** `web/src-vue-legacy/` và `ios-spike/`.
- ✅ **Bỏ hoàn toàn app desktop** (Tauri + Vue + sidecar chép lời offline). Sản phẩm chỉ còn web và iPhone; lịch sử desktop vẫn còn trong git.
- 🟡 **Đề xuất kiến trúc đích** (Jev đã duyệt một phần): `plans/dattqh/reports/brainstorm-261016-0809-scribe-target-architecture-refactor.md`.

## 2026-10-16 — Bước 2: key server + đo phút, đăng nhập hoandat-auth (plan `dattqh/261016-1114-server-key-metering-and-hoandat-auth`)

- ✅ **Phần A — key server + đo phút**: bỏ ô nhập key; chép lời dùng key OpenAI của server, giữ chỗ phút khi xếp hàng rồi quyết toán theo giây thật sự gửi; gói `free` 120 phút / `plus` 3.000 phút, quỹ free $10, trần tổng $30 mỗi tháng; CLI `scribe-be admin`. Restart không còn làm mất lượt chép.
- ✅ **Chống lạm dụng gói free** (Phase 9): 30 phút/ngày mỗi tài khoản, tài khoản mới chờ 24 giờ, `admin block|unblock`, báo động qua bot chat.hoandat.com `@bot_scribe_bot`. Không giới hạn theo IP (người dùng bỏ: 4G chung IP nhà mạng).
- ✅ **Bỏ mảnh im lặng**: mảnh hoàn toàn im lặng (đỉnh < −30 dB) không gửi OpenAI, không tính phút.
- ✅ **7a — đổi tên hạ tầng**: service `scribe-be`, user `scribe`, `/opt/scribe-be`, `/var/www/scribe/dist`, db `scribe` (user Mongo riêng). Bản `onescribe*` đã xoá.
- ✅ **7b — đăng nhập hoandat-auth + domain**: web (`scribe-web`) và iPhone (`scribe-mobile`) đăng nhập qua `auth.hoandat.com`, user cũ ghép theo email, phiên lưu Mongo thu hồi được; prod ở `scribe.hoandat.com` (Cloudflare SSL Full); bỏ Firebase và domain `scribe.inetdev.io.vn`. iPhone mới đã cài và kiểm trên máy thật.
- ✅ **7c — mở chép lời cho mọi người** (`TRANSCRIBE_OPEN=1`): bật 16/10 14:19 theo quyết định của chủ sản phẩm (Jev khuyên chờ vài ngày, 0,78). Gói free: 120 phút/tháng, 30 phút/ngày, tài khoản mới chờ 24 giờ, quỹ free $10, trần tổng $30.

## Scribe iOS — lộ trình App Store (chưa thực thi)

Chi tiết và lập luận: phase 4 của plan iOS. Tóm tắt:

- **Cần tài khoản Apple Developer ($99/năm)**: profile 1 năm thay 7 ngày, TestFlight, Live Activity (đồng hồ + nút Dừng trên màn hình khoá), mở đường ghi màn hình bằng ReplayKit.
- **Giấy tờ khi nộp**: Privacy Manifest (`PrivacyInfo.xcprivacy`); App Privacy khai "Audio Data" liên kết danh tính, và khai việc gửi audio tới OpenAI khi người dùng bấm Chép lời; ghi chú cho App Review về `UIBackgroundModes: audio`; nút xoá tài khoản trong app (chưa có).
- **Chặn thật nằm ở hạ tầng, không phải iOS**: allowlist đăng nhập đang mở cho mọi tài khoản hoandat-auth có email đã xác minh; chặng Cloudflare↔origin là HTTPS nhưng chứng chỉ tự ký (SSL Full, chưa strict); chưa có chính sách riêng tư và điều khoản; server chỉ giữ file 14 ngày. (Đã xong 16/10: phiên lưu Mongo, thu hồi được.)
- **Rà mã iOS (2026-10-15), cả 4 mục đạt**: `AudioRecorder` không import Capacitor · Team ID chỉ có trong `web/scripts/ios-device.sh`, không có trong mã app hay `project.pbxproj` · chuỗi xin quyền micro nêu rõ mục đích kể cả khi khoá màn hình · không dùng API không công khai.
- **Thứ tự nếu quyết định lên Store**: trả $99 → profile 1 năm → Live Activity → TestFlight vài người thử → hạ tầng đủ điều kiện công khai → nộp.

## Trạng thái hiện tại

- **Web**: LIVE bản React tại `scribe.hoandat.com`, đăng nhập hoandat-auth, giao diện theo khung Localman. Ghi (mic + màn hình/system) · Cắt file · lịch sử/phát lại · chép lời bằng key server có hạn mức (mở cho mọi người từ 16/10) · retention/quota/rate-limit · xoá thủ công.
- **iPhone**: bản `com.hoandat.scribe` đăng nhập hoandat-auth, đã kiểm trên máy thật (ghi khoá màn hình, tải lên, chép lời); còn cuộc gọi đến, AirPods, chế độ máy bay.

## Backlog / Next

| Ưu tiên | Hạng mục | Ghi chú |
|---------|----------|---------|
| ✅ P0 | **Refactor UI/UX theo format hệ sinh thái hoandat** (plan `dattqh/261016-1426-ui-ux-hoandat-ecosystem-refactor`) | Xong phase 1–5, web đã lên prod 16/10: khung Localman (top bar, account menu, footer), URL cho từng trang, từ điển tiếng Việt, Playwright sáng/tối. Còn QA cuối trên iPhone thật (phase 6) |
| P1 | **Gói "Sau cuộc họp"**: tóm tắt + quyết định + việc cần làm (LLM trên transcript), đánh dấu khoảnh khắc khi đang ghi (cả màn hình khoá iPhone), gửi biên bản sang chat.hoandat.com qua bot | Người dùng chốt 16/10 sau refactor UI; Jev chấm giá trị/công sức 2,34/1,08 · 1,80/0,69 · 1,99/0,81 (thang 0–3). Mọi tính năng LLM đi qua quota + trần chi phí như chép lời |
| P1 | **Gói "Kho bản ghi"**: tìm kiếm toàn văn mọi transcript (nhảy tới đúng giây), rồi hỏi đáp trên bản ghi có trích mốc thời gian | Sau gói "Sau cuộc họp"; Jev 1,96/1,27 · 2,37/1,53 |
| P2 | **Xuất phụ đề SRT/VTT, DOCX/Markdown**; sau đó **dịch transcript Việt ↔ Anh** | Jev 1,79/1,09 · 1,68/1,05 |
| P3 | **Phân biệt người nói** (Người 1/2, đổi tên được) | Cần nghiên cứu khả năng của model chép lời trước; Jev 1,54/1,00 (công sức có thể bị đánh giá thấp) |
| P1 | **QA app iPhone còn lại** — cuộc gọi đến, AirPods, máy bay | Cần máy thứ hai + AirPods |
| P1 | **Realtime transcript trên iPhone** (Phase 5 plan iOS) | Gate 0 bước 1 đo xong từ Mac; bước 2 (WebSocket khi khoá màn hình) cần iPhone |
| P2 | **QA thủ công web** — ghi mic + màn hình/system thật (Chrome), cắt file thật | getUserMedia/getDisplayMedia không automate được |
| P3 | **Unit test** `RateLimiter` (window reset) | logic pure, dễ test; bắt regression |
| P3 | **Hạ tầng CF Full (strict) + firewall origin** | Đang SSL Full với chứng chỉ tự ký; .65 shared → hiện chấp nhận origin-bypass; phòng thủ dồn vào hạn mức + rate-limit + audit |

## Unresolved questions

- Chưa có (mọi quyết định business đã chốt ở validate log của plan).
