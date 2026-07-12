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

## Trạng thái hiện tại

- **Web**: LIVE, 5/5 phase + 2 bổ sung đã deploy .65. Ghi (mic + màn hình/system) · Cắt file · lịch sử/phát lại · retention/quota/rate-limit · xoá thủ công.
- **Desktop**: ổn định (3 mode Ghi/Cắt/Transcript + PhoWhisper) — KHÔNG đụng trong đợt web (workspace `exclude src-tauri`).

## Backlog / Next

| Ưu tiên | Hạng mục | Ghi chú |
|---------|----------|---------|
| P1 | **QA thủ công web** — ghi mic + màn hình/system thật (Chrome), cắt file thật | getUserMedia/getDisplayMedia không automate được; cần thiết bị + share picker thật |
| P2 | **Transcript web (Mode C)** | tab "Sắp có"; chờ hạ tầng GPU. Khung `trait Worker`/`TranscribeWorker` stub sẵn ở BE |
| P2 | **Desktop rewire dùng `core-split`** | hiện desktop vẫn dùng `src-tauri/src/splitter.rs`; cần regression test Mode B trước khi chuyển sang crate chung |
| P3 | **Unit test** `use-recorder` (mock fetch: offset/chainError) + `RateLimiter` (window reset) | logic pure, dễ test; bắt regression |
| P3 | **Hạ tầng CF Full/strict + firewall origin** | .65 shared → hiện chấp nhận origin-bypass; phòng thủ dồn vào quota + rate-limit + audit |

## Unresolved questions

- Chưa có (mọi quyết định business đã chốt ở validate log của plan).
