# Project Roadmap — Scribe

Cập nhật 2026-10-15. Chi tiết kiến trúc: [codebase-summary.md](./codebase-summary.md).

## Scribe Web — plan `260711-1657-scribe-web-media-rust` — ✅ HOÀN TẤT (5/5 phase)

Web hóa Scribe làm ghi + cắt media (BỎ AI transcribe — box .65 không GPU). Live tại domain prod (`DEPLOY_DOMAIN` trong `.env.production`).

| Phase | Nội dung | Trạng thái |
|-------|----------|-----------|
| 1 | Foundation & deploy gate — Cargo workspace + `core-split` + axum skeleton + **musl static binary chạy .65** (GATE) + FE scaffold | ✅ DONE — GATE passed (`/api/health` 200 qua CF) |
| 2 | Auth Google (Firebase) — `signInWithPopup` + Rust verify ID token (securetoken JWKS) + session cookie httpOnly + Mongo users + allowlist(mở)+audit | ✅ DONE — login Google thật OK |
| 3 | Mode B — Cắt file — upload chunked resumable + job queue + SplitWorker (core-split) + tải zip + FE | ✅ DONE (live) |
| 4 | Mode A — Ghi — getUserMedia/getDisplayMedia + Web Audio mix + MediaRecorder streaming + lịch sử + player (stream/Range) + cắt webm (remux+copy) | ✅ DONE (code+review-fix+deploy+E2E live) |
| 5 | Retention & hardening — sweep purge 14 ngày + quota 5GB + rate-limit real-IP + size cap 2GB + magic-byte + allowlist + security pass | ✅ DONE (code+review-fix+deploy) |

**Quyết định chốt (validate):** domain prod riêng · allowlist MỞ mọi tài khoản Google (audit-only) · retention 14 ngày · quota 5GB/user · size cap 2GB/file · cắt recording webm = remux+copy.

## Bổ sung ngoài plan (2026-07-12)

- ✅ **Refactor UI web bám 100% desktop** — theme dark của app (xanh lá/cam) + BrandMark + Be Vietnam Pro + shell 3 tab (thay giao diện trắng/xanh dương cũ). Live.
- ✅ **Xoá recording thủ công** — `DELETE /api/files/:id` (ownership) + nút Xoá (xác nhận inline) ở lịch sử. Live.

## Bổ sung ngoài plan (2026-07-13)

- ✅ **Desktop rewire dùng `core-split`** — `src-tauri/src/splitter.rs` bỏ bản logic song sinh (~308 dòng), uỷ thác thuật toán cắt cho crate `core-split` (một nguồn chân lý chung desktop + server). Desktop giữ phần Tauri: resolve PATH sidecar (`ffmpeg::sidecar_path`, tái tạo đúng `relative_command_path` của tauri-plugin-shell) + batch dedup subdir + emit event. `split_file` giữ nguyên chữ ký (server BE **không đổi**); thêm `split_file_into(sub)` cho desktop. Regression test tự động (ffmpeg thật) trong `core-split/tests/split_file_ffmpeg.rs`. Verify: `cargo test -p core-split` 12/12 · `cargo clippy` sạch · desktop + workspace compile sạch. **Chờ QA runtime Mode B trên bản build Windows/macOS thật** (không automate được GUI).

## 2026-10-15 — web React, app iPhone, chép lời

- ✅ **Web viết lại bằng React** (plan `dattqh/261015-1524-web-react-rewrite`), parity với bản Vue, **đã lên prod** cùng ngày. Bản Vue đã xoá ngày 2026-10-16 (còn trong lịch sử git).
- ✅ **Chỉ còn hai môi trường**: dev local (Mongo dev trên lab 176) và prod `.65`; bỏ staging.
- ✅ **Audit**: access log mọi request + `audit_events` (TTL 180 ngày).
- ✅ **Chép lời bằng OpenAI** `gpt-4o-mini-transcribe`, key người dùng tự nhập, chỉ email trong `TRANSCRIBE_EMAILS` (hiện: `quocdat254@gmail.com`). Đoạn 10–15' buổi họp tham chiếu khớp TurboScribe 84,9%. Đã lên prod.
- ✅ **Sửa cắt "Số phần"** trong `core-split` (trước đây luôn ra thêm một đoạn cuối ~800 byte hỏng), áp cho cả web và desktop.
- 🟡 **App iPhone** (plan `260728-1609-scribe-ios-app-ghi-nen-khoa-man-hinh`): code Phase 1–3 xong, self-test Simulator đạt; QA trên iPhone thật đang chờ. Cài bằng `cd web && npm run ios:device` (free provisioning, cài lại mỗi 7 ngày). Chi tiết: [ios-app.md](./ios-app.md).

## 2026-10-16 — QA iPhone, dọn repo, CI

- ✅ **QA iPhone thật** (iPhone 15 Pro Max, prod): đăng nhập Google, ghi khi khoá màn hình, tắt app khi đang ghi, đăng xuất/đăng nhập lại khi còn đoạn chưa tải đều đạt. Chi tiết: [ios-app.md](./ios-app.md).
- ✅ **Bỏ GitLab hoàn toàn**: repo duy nhất là GitHub; thêm CI `.github/workflows/ci.yml` (chỉ kiểm: lint, typecheck, test web; clippy, test Rust). Deploy prod vẫn làm tay.
- ✅ **Xoá code cũ** `web/src-vue-legacy/` và `ios-spike/`.
- ✅ **Bỏ hoàn toàn app desktop** (Tauri + Vue + sidecar chép lời offline). Sản phẩm chỉ còn web và iPhone; lịch sử desktop vẫn còn trong git.
- 🟡 **Đề xuất kiến trúc đích** (Jev đã duyệt một phần): `plans/dattqh/reports/brainstorm-261016-0809-scribe-target-architecture-refactor.md`.

## Scribe iOS — lộ trình App Store (chưa thực thi)

Chi tiết và lập luận: phase 4 của plan iOS. Tóm tắt:

- **Cần tài khoản Apple Developer ($99/năm)**: profile 1 năm thay 7 ngày, TestFlight, Live Activity (đồng hồ + nút Dừng trên màn hình khoá), mở đường ghi màn hình bằng ReplayKit.
- **Giấy tờ khi nộp**: Privacy Manifest (`PrivacyInfo.xcprivacy`); App Privacy khai "Audio Data" liên kết danh tính, và khai việc gửi audio tới OpenAI khi người dùng bấm Chép lời; ghi chú cho App Review về `UIBackgroundModes: audio`; nút xoá tài khoản trong app (chưa có).
- **Chặn thật nằm ở hạ tầng, không phải iOS**: allowlist đăng nhập đang mở cho mọi tài khoản Google; session JWT 12 giờ không thu hồi được; chặng Cloudflare↔origin là HTTP; chưa có chính sách riêng tư và điều khoản; server chỉ giữ file 14 ngày.
- **Rà mã iOS (2026-10-15), cả 4 mục đạt**: `AudioRecorder` không import Capacitor · Team ID chỉ có trong `web/scripts/ios-device.sh`, không có trong mã app hay `project.pbxproj` · chuỗi xin quyền micro nêu rõ mục đích kể cả khi khoá màn hình · không dùng API không công khai.
- **Thứ tự nếu quyết định lên Store**: trả $99 → profile 1 năm → Live Activity → TestFlight vài người thử → hạ tầng đủ điều kiện công khai → nộp.

## Trạng thái hiện tại

- **Web**: LIVE bản React. Ghi (mic + màn hình/system) · Cắt file · lịch sử/phát lại · chép lời (email được phép) · retention/quota/rate-limit · xoá thủ công.
- **iPhone**: chờ QA trên máy thật.

## Backlog / Next

| Ưu tiên | Hạng mục | Ghi chú |
|---------|----------|---------|
| P1 | **QA app iPhone trên máy thật** — đăng nhập, ghi khoá màn hình 60 phút, cuộc gọi đến, AirPods, máy bay, chép lời | Cần iPhone cắm vào Mac |
| P1 | **Realtime transcript trên iPhone** (Phase 5 plan iOS) | Gate 0 bước 1 đo xong từ Mac; bước 2 (WebSocket khi khoá màn hình) cần iPhone |
| P2 | **QA thủ công web** — ghi mic + màn hình/system thật (Chrome), cắt file thật | getUserMedia/getDisplayMedia không automate được |
| P3 | **Unit test** `RateLimiter` (window reset) | logic pure, dễ test; bắt regression |
| P3 | **Hạ tầng CF Full/strict + firewall origin** | .65 shared → hiện chấp nhận origin-bypass; phòng thủ dồn vào quota + rate-limit + audit |

## Unresolved questions

- Chưa có (mọi quyết định business đã chốt ở validate log của plan).
