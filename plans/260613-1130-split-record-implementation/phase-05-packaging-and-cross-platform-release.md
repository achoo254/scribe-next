---
phase: 5
title: "Packaging and cross-platform release"
status: "in-progress — Windows NSIS (2026-07-09) + macOS arm64 DMG ad-hoc (2026-07-10) BUILT; QA máy sạch + (tùy chọn) Intel/notarize còn lại"
priority: P2
effort: "2-3d"
dependencies: [2, 3, 4]
---

# Phase 5: Packaging and cross-platform release

<!-- Updated: Validation Session 1 - CHỈ 1 installer đầy đủ (cả 2 build-block); KHÔNG release sớm build-block 1 riêng -->
## Overview

Đóng gói **một installer đầy đủ** cho Windows + macOS với cả 2 sidecar (ffmpeg + transcriber), thêm quyền/notarize macOS, QA cuối theo success criteria, cập nhật docs. Phát hành 1 lần sau khi xong cả 3 mode (không có biến thể release sớm).

## Requirements

- Functional: installer Windows (NSIS/MSI) + macOS (.dmg/.app) chạy được trên máy sạch; mic xin quyền đúng; model tải-khi-dùng hoạt động trên cả 2 OS.
- Non-functional: sidecar đúng target mỗi OS; macOS app ký + notarize; kích thước/đính kèm hợp lý (không nhúng model).

## Architecture

- **Sidecar per-target**: `ffmpeg-{x86_64-pc-windows-msvc, x86_64-apple-darwin, aarch64-apple-darwin}` + `transcriber-<target>` tương ứng. PyInstaller build trên từng OS (không cross-build được Python sidecar).
- **Tauri bundle**: `tauri.conf.json` bundler targets per-OS; macOS `Info.plist` thêm `NSMicrophoneUsageDescription`; entitlements cho mic + hardened runtime; ký + `notarytool`.
- **ffmpeg licensing**: bản LGPL + ghi attribution trong About/README.
- **CI (tùy chọn)**: GitHub Actions matrix windows + macos build + (macOS) sign/notarize bằng secrets.

## Related Code Files

- Modify: `src-tauri/tauri.conf.json` (bundle targets, externalBin per-target, macOS plist/entitlements)
- Create: `src-tauri/entitlements.plist` (macOS mic + hardened runtime)
- Create: `.github/workflows/build-release.yml` (tùy chọn)
- Create/Modify: `sidecar/transcriber/build_pyinstaller.{sh,ps1}` (macOS target)
- Modify: `README.md`, `docs/split-record-design.md` (deployment notes), `docs/deployment-guide.md`

## Implementation Steps

1. Build ffmpeg LGPL macOS (intel + arm) → đặt sidecar đúng target.
2. Build transcriber PyInstaller trên macOS → `transcriber-{x86_64,aarch64}-apple-darwin`.
3. `tauri.conf.json`: cấu hình bundle Windows (nsis) + macOS (dmg); khai báo externalBin per-target.
4. macOS: `Info.plist` `NSMicrophoneUsageDescription`; entitlements; ký Developer ID + notarize (notarytool); staple.
5. Windows: build MSI/NSIS; test mic + record + split + transcribe trên máy sạch (không có Python/Rust).
6. QA toàn bộ success criteria (P2/P3/P4) trên cả 2 OS; kiểm tra tải model offline. **QA design system**: build production render đúng theme brand (token CSS vars + Vuetify tree-shaking ok), 3 view dùng Ui* hiển thị đúng; chạy MCP design system `validate_brand_usage` + `review_design_tokens` trên toàn `src/**/*.vue` → 0 vi phạm; kiểm WCAG AA (contrast text ≥4.5:1, focus state, keyboard nav) qua `get_accessibility_guide` checklist.
7. (Tùy chọn) GitHub Actions matrix tự build artifact.
8. Cập nhật README (cài đặt, quyền, lần đầu tải model ~1.5GB) + deployment-guide.

## Todo List

- [x] ffmpeg sidecar macOS — **arm64** (`ffmpeg/ffprobe-aarch64-apple-darwin`) OK; Intel để sau (cần máy Intel)
- [x] transcriber PyInstaller macOS — **arm64** (`transcriber-aarch64-apple-darwin`, gồm mlx, ~190 MB)
- [x] tauri.conf.json bundle targets + externalBin per-target — Windows `nsis` + macOS `app`/`dmg` (qua `tauri.macos.conf.json`) + 3 externalBin OK
- [x] macOS plist + entitlements + ký — `Info.plist` (mic) + `entitlements.plist` + **ad-hoc sign** (`signingIdentity:"-"`). Notarize KHÔNG làm (tool nội bộ, quyết định 2026-07-10)
- [~] Windows installer — **BUILT** `Scribe_0.1.0_x64-setup.exe` (~174 MB, verify plumbing OK); **test máy sạch chờ QA**
- [~] macOS installer — **BUILT** `Scribe_0.1.0_aarch64.dmg` (~185 MB); verify chữ ký/plist/sidecar OK; **test máy sạch chờ QA**
- [ ] QA success criteria cả 2 OS + offline model
- [ ] (tùy chọn) CI workflow
- [x] README + deployment-guide — `docs/deployment-guide.md` (Windows + macOS); README đã có env + sidecar

## Build Log — Windows installer (2026-07-09)

- **Prereq fix**: `transcriber-x86_64-pc-windows-msvc.exe` cũ (build 2026-06-15) trong khi `main.py` đổi lớn 2026-07-08 (glossary/word_timestamps/export VTT-JSON/model mgmt) → **build lại** qua `build_pyinstaller.ps1` (PyInstaller 6.21, Python 3.14.3). Smoke test onefile: `{"cmd":"shutdown"}` → `{"type":"ready"}` exit 0.
- **Build**: `npm run tauri build` → Rust release 1m00s + makensis → `src-tauri/target/release/bundle/nsis/Scribe_0.1.0_x64-setup.exe` (182,467,244 B).
- **Verify plumbing**: 3 sidecar staged cạnh `scribe.exe` trong `target/release/` (`ffmpeg.exe`, `ffprobe.exe`, `transcriber.exe` @20:49 = bản mới). `resolve()` (`transcribe.rs:182`) khớp tên trần `transcriber.exe`; ffmpeg/ffprobe qua `app.shell().sidecar()`. Model KHÔNG nhúng (size 174MB xác nhận).
- **Còn lại**: test cài trên máy sạch (không dev tools) — vai QA của user; checklist ở `docs/deployment-guide.md`.

## Build Log — macOS DMG arm64 (2026-07-10)

- **Quyết định scope** (user, 2026-07-10): chỉ **arm64** (Apple Silicon) + **ad-hoc sign** (không notarize) cho phân phối nội bộ. Intel + Developer ID notarize để backlog (máy hiện có 0 signing identity; PyInstaller không cross-build).
- **Prereq**: `transcriber-aarch64-apple-darwin` cũ (2026-07-08) < `main.py` (commit c9f0286 GPU CUDA, 2026-07-10) → **build lại** qua `build_pyinstaller.sh` (PyInstaller 6.21, Python 3.14.6, gồm mlx 0.32/mlx-whisper 0.4.3, loại torch). Smoke onefile: `{"cmd":"shutdown"}` → `{"type":"ready"}`.
- **Config mới**: `src-tauri/tauri.macos.conf.json` (targets `app`+`dmg`, `minimumSystemVersion 11.0`, `signingIdentity "-"`, `entitlements`), `src-tauri/entitlements.plist` (audio-input + disable-library-validation + allow-unsigned-executable-memory), `src-tauri/Info.plist` (`NSMicrophoneUsageDescription`).
- **Build**: `npm run tauri build` → Rust release 34s → bundle `.app` (ad-hoc sign 4 binary + .app) + `bundle_dmg.sh`. Output: `Scribe.app` (195 MB) + `Scribe_0.1.0_aarch64.dmg` (185 MB).
- **Verify**: `codesign --verify --deep --strict` → "valid on disk" + "satisfies Designated Requirement"; Info.plist có mic + id `com.hoandat.scribe` + min 11.0; entitlements đủ 3 key; bundled transcriber → `{"type":"ready"}`, ffmpeg 8.1.2 chạy; `spctl` = rejected (đúng với ad-hoc → cần bỏ chặn Gatekeeper lần đầu, đã ghi hướng dẫn trong deployment-guide).
- **Còn lại**: QA máy sạch (vai user); (backlog) Intel build + notarize nếu cần phân phối rộng.

## Success Criteria

- [ ] Installer Windows chạy trên máy sạch: record + split + transcribe OK
- [ ] Installer macOS (ký+notarize) chạy: xin quyền mic, record + split + transcribe OK
- [ ] Model tải-khi-dùng hoạt động cả 2 OS, sau đó offline
- [ ] Không nhúng model vào installer; sidecar đúng target
- [ ] UI tuân thủ design system: Ui*-only, token-only, WCAG AA — 0 vi phạm khi verify bằng MCP design system trên cả 2 OS

## Risk Assessment

- macOS notarize fail vì sidecar chưa ký/hardened runtime. Mitigation: ký từng binary trong bundle trước notarize; test `spctl`/`codesign --verify`.
- PyInstaller macOS gatekeeper chặn ctranslate2 dylib. Mitigation: ký dylib + entitlement `disable-library-validation` nếu cần.
- Installer lớn do 2 sidecar. Chấp nhận (model 1.5GB vẫn tải sau, không nhúng) — đây là 1 installer đầy đủ theo quyết định validation.

## Next Steps

Release v1. Backlog v2: silence-detect split, merge, re-encode cắt chính xác, GPU, model nhỏ hơn, audio hệ thống/loopback. Chạy `/ck:journal` ghi nhật ký.
