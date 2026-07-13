# Project Roadmap — OneScribe

Cập nhật 2026-07-12. Chi tiết kiến trúc: [codebase-summary.md](./codebase-summary.md).

## OneScribe Web — plan `260711-1657-onescribe-web-media-rust` — ✅ HOÀN TẤT (5/5 phase)

Web hóa OneScribe làm ghi + cắt media (BỎ AI transcribe — box .65 không GPU). Live tại **https://scribe.inetdev.io.vn**.

| Phase | Nội dung | Trạng thái |
|-------|----------|-----------|
| 1 | Foundation & deploy gate — Cargo workspace + `core-split` + axum skeleton + **musl static binary chạy .65** (GATE) + FE scaffold | ✅ DONE — GATE passed (`/api/health` 200 qua CF) |
| 2 | Auth Google (Firebase) — `signInWithPopup` + Rust verify ID token (securetoken JWKS) + session cookie httpOnly + Mongo users + allowlist(mở)+audit | ✅ DONE — login Google thật OK |
| 3 | Mode B — Cắt file — upload chunked resumable + job queue + SplitWorker (core-split) + tải zip + FE | ✅ DONE (live) |
| 4 | Mode A — Ghi — getUserMedia/getDisplayMedia + Web Audio mix + MediaRecorder streaming + lịch sử + player (stream/Range) + cắt webm (remux+copy) | ✅ DONE (code+review-fix+deploy+E2E live) |
| 5 | Retention & hardening — sweep purge 14 ngày + quota 5GB + rate-limit real-IP + size cap 2GB + magic-byte + allowlist + security pass | ✅ DONE (code+review-fix+deploy) |

**Quyết định chốt (validate):** domain `scribe.inetdev.io.vn` · allowlist MỞ mọi tài khoản Google (audit-only) · retention 14 ngày · quota 5GB/user · size cap 2GB/file · cắt recording webm = remux+copy.

## Bổ sung ngoài plan (2026-07-12)

- ✅ **Refactor UI web bám 100% desktop** — theme dark One Series (xanh lá/cam) + BrandMark + Be Vietnam Pro + shell 3 tab (thay giao diện trắng/xanh dương cũ). Live.
- ✅ **Xoá recording thủ công** — `DELETE /api/files/:id` (ownership) + nút Xoá (xác nhận inline) ở lịch sử. Live.

## Bổ sung ngoài plan (2026-07-13)

- ✅ **Desktop rewire dùng `core-split`** — `src-tauri/src/splitter.rs` bỏ bản logic song sinh (~308 dòng), uỷ thác thuật toán cắt cho crate `core-split` (một nguồn chân lý chung desktop + server). Desktop giữ phần Tauri: resolve PATH sidecar (`ffmpeg::sidecar_path`, tái tạo đúng `relative_command_path` của tauri-plugin-shell) + batch dedup subdir + emit event. `split_file` giữ nguyên chữ ký (server BE **không đổi**); thêm `split_file_into(sub)` cho desktop. Regression test tự động (ffmpeg thật) trong `core-split/tests/split_file_ffmpeg.rs`. Verify: `cargo test -p core-split` 12/12 · `cargo clippy` sạch · desktop + workspace compile sạch. **Chờ QA runtime Mode B trên bản build Windows/macOS thật** (không automate được GUI).

## Trạng thái hiện tại

- **Web**: LIVE, 5/5 phase + 2 bổ sung đã deploy .65. Ghi (mic + màn hình/system) · Cắt file · lịch sử/phát lại · retention/quota/rate-limit · xoá thủ công.
- **Desktop**: ổn định (3 mode Ghi/Cắt/Transcript + PhoWhisper) — KHÔNG đụng trong đợt web (workspace `exclude src-tauri`).

## Backlog / Next

| Ưu tiên | Hạng mục | Ghi chú |
|---------|----------|---------|
| P1 | **QA thủ công web** — ghi mic + màn hình/system thật (Chrome), cắt file thật | getUserMedia/getDisplayMedia không automate được; cần thiết bị + share picker thật |
| P2 | **Transcript web (Mode C)** | tab "Sắp có"; chờ hạ tầng GPU. Khung `trait Worker`/`TranscribeWorker` stub sẵn ở BE |
| ~~P2~~ | ~~**Desktop rewire dùng `core-split`**~~ | ✅ Xong 2026-07-13 (xem "Bổ sung ngoài plan"). Còn lại: QA runtime Mode B trên bản build thật |
| P3 | **Unit test** `use-recorder` (mock fetch: offset/chainError) + `RateLimiter` (window reset) | logic pure, dễ test; bắt regression |
| P3 | **Hạ tầng CF Full/strict + firewall origin** | .65 shared → hiện chấp nhận origin-bypass; phòng thủ dồn vào quota + rate-limit + audit |

## Unresolved questions

- Chưa có (mọi quyết định business đã chốt ở validate log của plan).
