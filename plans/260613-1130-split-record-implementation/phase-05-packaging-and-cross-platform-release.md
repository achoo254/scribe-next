---
phase: 5
title: "Packaging and cross-platform release"
status: pending
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
6. QA toàn bộ success criteria (P2/P3/P4) trên cả 2 OS; kiểm tra tải model offline. **QA design system**: build production render đúng theme iNET (token CSS vars + Vuetify tree-shaking ok), 3 view dùng Vi* hiển thị đúng; chạy ViUI MCP `validate_brand_usage` + `review_design_tokens` trên toàn `src/**/*.vue` → 0 vi phạm; kiểm WCAG AA (contrast text ≥4.5:1, focus state, keyboard nav) qua `get_accessibility_guide` checklist.
7. (Tùy chọn) GitHub Actions matrix tự build artifact.
8. Cập nhật README (cài đặt, quyền, lần đầu tải model ~1.5GB) + deployment-guide.

## Todo List

- [ ] ffmpeg sidecar macOS (intel+arm)
- [ ] transcriber PyInstaller macOS
- [ ] tauri.conf.json bundle targets + externalBin per-target
- [ ] macOS plist + entitlements + ký + notarize
- [ ] Windows installer + test máy sạch
- [ ] QA success criteria cả 2 OS + offline model
- [ ] (tùy chọn) CI workflow
- [ ] README + deployment-guide

## Success Criteria

- [ ] Installer Windows chạy trên máy sạch: record + split + transcribe OK
- [ ] Installer macOS (ký+notarize) chạy: xin quyền mic, record + split + transcribe OK
- [ ] Model tải-khi-dùng hoạt động cả 2 OS, sau đó offline
- [ ] Không nhúng model vào installer; sidecar đúng target
- [ ] UI tuân thủ ViUI design system: Vi*-only, token-only, WCAG AA — 0 vi phạm khi verify bằng ViUI MCP trên cả 2 OS

## Risk Assessment

- macOS notarize fail vì sidecar chưa ký/hardened runtime. Mitigation: ký từng binary trong bundle trước notarize; test `spctl`/`codesign --verify`.
- PyInstaller macOS gatekeeper chặn ctranslate2 dylib. Mitigation: ký dylib + entitlement `disable-library-validation` nếu cần.
- Installer lớn do 2 sidecar. Chấp nhận (model 1.5GB vẫn tải sau, không nhúng) — đây là 1 installer đầy đủ theo quyết định validation.

## Next Steps

Release v1. Backlog v2: silence-detect split, merge, re-encode cắt chính xác, GPU, model nhỏ hơn, audio hệ thống/loopback. Chạy `/ck:journal` ghi nhật ký.
