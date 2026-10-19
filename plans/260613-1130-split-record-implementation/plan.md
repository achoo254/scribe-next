---
title: "Scribe Desktop App Implementation"
description: "Tauri v2 desktop app (Win+macOS): ghi mic + split live, cắt file sẵn có, transcribe offline tiếng Việt"
status: completed
priority: P2
branch: ""
tags: [tauri, rust, audio, ffmpeg, whisper, desktop]
blockedBy: []
blocks: []
created: "2026-06-13T04:55:41.086Z"
createdBy: "ck:plan"
source: skill
---

# Scribe Desktop App Implementation

> **Đóng plan (2026-10-19):** Mode A/B/C đã giao và có bộ cài Windows NSIS (2026-07-09). App desktop đã bỏ ngày
> 2026-10-16 (sản phẩm chỉ còn web và iPhone), nên bản macOS và QA máy sạch không làm nữa; logic cắt sống tiếp trong crate `core-split`.

> **Tên app**: **Scribe** · bundle id `com.hoandat.scribe` · package `scribe-next` · window title "Scribe". *(Thư mục repo hiện tại `split-record` — đổi tên `scribe-next` là tùy chọn, không bắt buộc để build.)*

## Overview

**Scribe** — Desktop app (Tauri v2, Rust backend + **Vue 3/Vite + Vuetify, UI bắt buộc theo design system**) cho Windows + macOS với 3 capability:
- **Mode A** — Ghi audio micro, split LIVE mỗi N phút/MB (hybrid: cpal capture → ffmpeg stdin segment muxer), level meter, pause/resume.
- **Mode B** — Cắt file audio sẵn có theo phút / MB / chia-K đều + batch (ffmpeg `-c copy`, lossless).
- **Mode C** — Transcribe offline tiếng Việt (faster-whisper Python sidecar + model `kiendt/PhoWhisper-large-ct2`), thủ công + batch, xuất `.txt`/`.srt` + hiển thị in-app.

Design doc nguồn (đã chốt): [`docs/split-record-design.md`](../../docs/split-record-design.md).

**Build-block phân phase** (thứ tự BUILD; chỉ phát hành 1 lần ở P5):
- **Build-block 1 (record + split)** = Phase 1 + 2 + 3 → làm trước, rủi ro thấp.
- **Build-block 2 (transcribe)** = Phase 4 → phần nặng + rủi ro đóng gói Python, làm sau.
- **Release** = Phase 5 → **một installer đầy đủ** (cả 2 build-block, kèm cả 2 sidecar). Không phát hành sớm build-block 1 riêng.

## Phases

| Phase | Name | Status |
|-------|------|--------|
| 1 | [Scaffold and Tauri shell](./phase-01-scaffold-and-tauri-shell.md) | ✅ Done (2026-06-13) |
| 2 | [Mode A - Record and split live](./phase-02-mode-a-record-and-split-live.md) | 🟡 Implemented (2026-06-13) — chờ test ghi âm thủ công |
| 3 | [Mode B - Split existing files](./phase-03-mode-b-split-existing-files.md) | 🟡 Implemented (2026-06-13) — chờ test cắt file thủ công |
| 4 | [Mode C - Transcription sidecar](./phase-04-mode-c-transcription-sidecar.md) | ✅ Done (2026-06-15) — PyInstaller build + transcribe thật OK + picker đa-model (small/medium/large) + batching |
| 5 | [Packaging and cross-platform release](./phase-05-packaging-and-cross-platform-release.md) | 🟡 Windows NSIS installer BUILT (2026-07-09) — macOS + QA máy sạch còn lại |

## Dependency graph (nội bộ plan)

```
P1 (scaffold) ──┬──► P2 (Mode A)
                ├──► P3 (Mode B)
                └──► P4 (Mode C)
P2 + P3 + P4 ───────► P5 (release)
```

- P2, P3, P4 đều phụ thuộc P1 (Tauri shell + ffmpeg sidecar + settings + 3-tab UI).
- P2/P3 dùng chung ffmpeg sidecar; làm tuần tự (tránh đụng UI shell + commands.rs).
- P5 phụ thuộc P2+P3 (build-block 1) và P4 (build-block 2).

## Key constraints

- Mỗi file code < 200 dòng (tách module khi vượt).
- KISS/YAGNI/DRY. Không nhúng model 1.5GB vào installer (download-on-first-use).
- ffmpeg dùng bản **LGPL** (ghi attribution). macOS cần `NSMicrophoneUsageDescription` + sign/notarize sidecars.

## Design system (BẮT BUỘC)

> Toàn bộ UI **PHẢI** tuân thủ design system ở **mức component + token + brand + WCAG**. Quy tắc này **non-negotiable**, áp dụng cho mọi phase có UI (P1–P4) và QA (P5).
>
> **Phạm vi (quyết định cuối — Validation Session 2)**: stack UI = **Vue 3 + Vite + Vuetify (public)**; **tự dựng wrapper `Ui*`** trong repo theo spec MCP (vì package `@hobui/ui` ở registry private không truy cập được); **token vendor từ MCP** (`@hobui/tokens` cũng không có trên npm/CDN). Mọi tham chiếu chuẩn lấy từ **MCP design system**, không cài `@hobui/*`.

- **Stack UI = Vue 3 + Vite + Vuetify 3.5+** (đổi từ vanilla TS — xem Validation Session 2). Packages **public**: `vue`, `vuetify@^3.5`, `@tabler/icons-vue`, `@vitejs/plugin-vue`, `vite-plugin-vuetify`. **KHÔNG cài `@hobui/ui`/`@hobui/tokens`** (private/không có trên npm).
- **Component = wrapper `Ui*` tự dựng** trong `src/design-system/ui/vi-*/Ui*.vue` — wrapper mỏng quanh Vuetify, bám **spec MCP** `get_component_api` (props/slots/emits) + `get_component_usage` (ví dụ) + `brand_dna_notes`. App view **chỉ dùng `Ui*`** (cấm raw Vuetify ngoài internals wrapper). Cần component mới → dựng wrapper Ui* trước.
- **Theme = dark (`scribeDark`)**: Scribe → **dark-first**. Nền `--ui-color-brand-bg` `#0d0d0d` / card `#1a1a1a`; **primary green** `--ui-color-brand-green` `#49c98d` (Record/CTA); **accent orange** `--ui-color-brand-orange` `#f7ad30`; text `#e0e0e0`. ⚠️ Token tự cảnh báo green/orange **FAIL 3:1 trên nền trắng** → chỉ dùng trên nền tối (đã thỏa vì dark-first).
- **Token vendor từ MCP**: chạy MCP `generate_color_vars` (css, light+dark) → lưu `src/assets/styles/design-tokens.css` (prefix `--ui-color-*`, gồm nhóm `brand-*`); spacing 8pt / typography / radius tra qua `lookup_token`/`search_tokens` → vendor vào `src/tokens/`. Import token vào `main.ts`; wire màu **brand** vào Vuetify theme `scribeDark` (defaultTheme). **KHÔNG hardcode hex/spacing/typography**; **KHÔNG `!important`**.
- **Brand DNA + WCAG 2.1 AA bắt buộc**: màu semantic đúng ngữ cảnh (One green=primary, orange=accent, error/success/warning/info từ token); text ≥ 4.5:1, icon ≥ 3:1, focus state rõ, keyboard-navigable. Tham chiếu MCP `get_brand_rules`, `get_accessibility_guide`.
- **Icon = Tabler** (`@tabler/icons-vue`, public) — tra cứu qua MCP `list_icons`.
- **Verify mỗi UI change**: chạy MCP `validate_brand_usage` + `review_design_tokens` trên file `.vue`/CSS mới/sửa → 0 vi phạm. `code-reviewer` kiểm tra tuân thủ design system (Ui*-only, token-only, WCAG) như một acceptance criterion.
- **Lưu ý lệch chuẩn**: vì Ui* là *tự dựng* (không phải `@hobui/ui` gốc), wrapper có thể lệch design system thật về CSS DNA theme (gradient/glass...). Chấp nhận cho v1; nếu sau có credentials registry → thay wrapper bằng package gốc (re-plan).

## Dependencies

<!-- Cross-plan dependencies: none (greenfield, no overlapping plans) -->

## Validation Log

### Session 1 — 2026-06-13

**Verification Results**
- Greenfield: mọi file là *Create* → không có codebase để grep-verify path.
- Claim đối ngoại đã verify: faster-whisper hỗ trợ tiếng Việt + model `kiendt/PhoWhisper-large-ct2` (CTranslate2, MIT) ✓; Tauri v2 sidecar/externalBin ✓; ffmpeg segment muxer + stdin s16le + M4A frag ✓.
- Claims checked: 4 external | Verified: 4 | Failed: 0 | Unverified: 0 | Tier: Full (5 phases, nhưng greenfield).

**Decisions confirmed**
1. **Format ghi live mặc định = MP3** (stream-friendly, crash-safe); M4A-AAC vẫn là option (dùng `+frag_keyframe+empty_moov`). → P2.
2. **Model ASR v1 = chỉ PhoWhisper-large**; picker model nhỏ hơn (medium) để v2. → P4 (resolved open question).
3. **Transcript lưu cạnh file gốc** (đổi được trong settings sau). → P4 (resolved open question).
4. **Chỉ 1 installer đầy đủ** ở P5 (cả 2 build-block). KHÔNG phát hành sớm build-block 1 riêng. → P5 (gỡ phương án bundle variant).
5. (Liên quan) Bitrate/sample-rate ghi âm **cố định mặc định** ở v1 (resolved open question, theo đề xuất design doc).

### Whole-Plan Consistency Sweep
- Đã re-read plan.md + 5 phase + design doc. Đồng bộ: P2 default MP3; P4 large-only + transcript cạnh file gốc; P5 single installer (gỡ variant). Design doc §7/§10/§12 cập nhật khớp.
- Không còn mâu thuẫn chưa giải quyết.

### Session 2 — 2026-06-13 (UI design system mandate — Vue 3 + self-built Ui*)

**Quyết định cuối của người dùng**: UI **BẮT BUỘC** theo design system ở **mức component**. Stack UI = **Vue 3 + Vite + Vuetify (public)**; **tự dựng wrapper `Ui*`** theo spec MCP; **token vendor từ MCP**. KHÔNG cài `@hobui/*`.

**Quá trình quyết định (3 bước)**:
1. Ban đầu chọn mức component (Vue 3 + Ui* qua `@hobui/ui`).
2. Verify phát hiện **blocker**: `@hobui/ui` + `@hobui/tokens` **không có trên npm public lẫn CDN jsDelivr** (cùng E404, kiểm 2026-06-13) → cần registry private (chưa có credentials).
3. Người dùng xác nhận: **dùng Vuetify public + tự dựng wrapper Ui* theo spec MCP**, token sinh từ MCP `generate_color_vars` rồi vendor vào repo. (Đã loại phương án token-only và phương án xin credentials.)

**Drift được ủy quyền (ghi nhận minh bạch)**: quyết định cũ R1 = "vanilla TS/Vite", §5 = "không framework nặng (KISS)". Người dùng **chủ động chọn** đổi → Vue 3 + Vuetify. Đảo quyết định *có người dùng xác nhận* (không phải audit/Claude tự ý). Backend Rust + sidecar/ffmpeg/whisper **không đổi**.

**Tác động phase**: P1 đổi scaffold vanilla-ts → vue-ts + Vuetify + vendor token + dựng bộ wrapper Ui* cần dùng (effort ~+1d); P2/P3/P4 UI viết `.vue` dùng Ui* + token; P5 QA thêm WCAG AA + brand + Vuetify bundle. Xem mục "Design system (BẮT BUỘC)".

**Đã verify (external, 2026-06-13)**: `@hobui/ui`+`@hobui/tokens` E404 npm + jsDelivr (không cài/CDN được); Vuetify + `@tabler/icons-vue` là public; MCP `get_component_api`/`get_component_usage` trả spec + ví dụ Ui* (Ui* = wrapper mỏng Vuetify, vd UiButton passthrough `color/variant/size/loading/disabled`); MCP `generate_color_vars` trả đủ CSS vars `--ui-color-*` (light+dark) → vendor được; quy tắc Ui*-only + token + WCAG (MCP `code-standards`).

**Open question còn lại**:
- Wrapper Ui* tự dựng có thể **lệch design system gốc** về CSS DNA theme (glass/bold/glow gradient) — package gốc mới có CSS đó. Chấp nhận v1 (app utility 3 tab không cần DNA CTA). Có credentials registry → thay bằng `@hobui/ui` gốc.
- Tên prefix token: `generate_color_vars` cho `--ui-color-*`; spacing/typography tra `lookup_token`/`search_tokens` lúc implement — không hardcode tên.

### Session 3 — 2026-06-13 (Naming + theme)

**Quyết định người dùng**:
1. **Tên app = Scribe**. Định danh kỹ thuật: productName/window title `Scribe`; bundle id `com.hoandat.scribe`; package `scribe-next`. Thư mục repo hiện `split-record` — đổi `scribe-next` là tùy chọn (không bắt buộc để build).
2. **Theme = dark (`scribeDark`)**, dark-first: nền `#0d0d0d`/card `#1a1a1a`, primary green `#49c98d`, accent orange `#f7ad30`, text `#e0e0e0`.

**Đã verify (external)**: token `--ui-color-brand-*` có sẵn trong output MCP `generate_color_vars` (green `#49c98d`, orange `#f7ad30`, bg `#0d0d0d`/`#1a1a1a`/border `#2e2e2e`); **cảnh báo WCAG ngay trong token**: green+orange FAIL 3:1 trên nền trắng → chỉ dùng nền tối → khớp với chọn dark-first (an toàn AA vì text sáng trên nền tối).

**Tác động phase**: P1 set productName/identifier/package = Scribe + cấu hình Vuetify `scribeDark` (defaultTheme) wire palette brand; P2/P3/P4 dùng green=primary (Record/CTA), error=red cho Stop/Delete; P5 QA WCAG trên nền tối + tên installer Scribe.
