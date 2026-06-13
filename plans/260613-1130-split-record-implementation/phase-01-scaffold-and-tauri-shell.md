---
phase: 1
title: "Scaffold and Tauri shell"
status: done
priority: P1
effort: "2-3d"
dependencies: []
completed: "2026-06-13"
---

# Phase 1: Scaffold and Tauri shell

## Overview

Dựng nền **OneScribe** (Tauri v2, Rust + **Vue 3/Vite + Vuetify 3.5+**, theme **One Series dark `inetOneDark`**), vendor **iNET ViUI design tokens** (sinh từ MCP) + dựng bộ **wrapper `Vi*`** cần dùng, wiring ffmpeg sidecar, khung UI 3 tab rỗng, lớp settings persistence. productName/identifier = `OneScribe`/`vn.inet.onescribe`. Mọi phase sau build lên đây. **UI bắt buộc theo ViUI** — xem `plan.md` → "Design system — iNET ViUI (BẮT BUỘC)".

## Requirements

- Functional: app mở 1 cửa sổ, 3 tab (Ghi âm / Cắt file / Transcript) chuyển qua lại được; settings (mic, folder, format, N) lưu/đọc lại sau restart; ffmpeg sidecar gọi được (`ffmpeg -version` trả về).
- Non-functional: chạy `dev` được trên Windows (máy hiện tại); mỗi file Rust < 200 dòng; mỗi file `.vue`/`.ts`/CSS < 200 LOC; không hardcode path tuyệt đối.
- **Design system (bắt buộc)**: Vuetify + theme iNET (từ token vendor) cấu hình xong; bộ wrapper `Vi*` (≥ ViButton, ViSelect, ViInput, ViRadioGroup/ViButtonToggle, ViCard, ViAlert, ViIcon) dựng theo spec MCP; shell UI **chỉ dùng `Vi*`** (không raw Vuetify); **token-only** (không hardcode màu/spacing/typography); Brand DNA + WCAG AA; icon Tabler.

## Architecture

- **Tauri v2** + plugins: `tauri-plugin-shell` (sidecar), `tauri-plugin-dialog` (chọn file/folder), `tauri-plugin-store` (settings JSON), `tauri-plugin-fs` (mở thư mục).
- **UI**: **Vue 3 + Vite + Vuetify 3.5+** (đổi từ vanilla TS — Validation Session 2). Packages public: `vue`, `vuetify@^3.5`, `@tabler/icons-vue`, `@vitejs/plugin-vue`, `vite-plugin-vuetify`. Cấu trúc:
  - `src/assets/styles/viui-tokens.css`: **vendor** từ MCP `generate_color_vars` (css, light+dark; prefix `--viui-color-*`). Import trong `main.ts` trước style khác.
  - `src/tokens/`: spacing 8pt / typography / radius tra từ MCP `lookup_token`/`search_tokens` → file ts/css vendor.
  - `src/plugins/vuetify.ts`: `createVuetify` `defaultTheme: 'inetOneDark'` (One Series dark — green `#49c98d` primary, orange `#f7ad30` accent, nền `#0d0d0d`/card `#1a1a1a`), màu wire từ token vendor (`--viui-color-products-one-*`); iconset Tabler.
  - `src/design-system/vi-ui/vi-*/Vi*.vue`: **wrapper `Vi*` tự dựng** (mỏng quanh Vuetify) theo spec MCP `get_component_api` + `brand_dna_notes` (vd `ViButton` wrap `v-btn`, passthrough `color/variant/size/loading/disabled`). Barrel `index.ts`.
  - `src/App.vue`: shell 3 tab (tab switcher bằng Vi*/Vuetify tabs), render view theo tab (`<component :is>`, KISS — không vue-router).
  - `src/views/{RecordView,SplitView,TranscriptView}.vue`: mỗi tab 1 view rỗng (placeholder), điền P2/P3/P4.
  - `src/main.ts`: import token css → `createApp(App).use(vuetify).mount('#app')`.
  - `src/styles.css`: global, **chỉ token CSS vars**, không hex cứng, không `!important`, spacing 8pt.
- **ViUI mandate**: app view chỉ `Vi*` (cấm `v-btn`/`v-card`... ngoài internals wrapper); token-only; WCAG AA; icon Tabler. Tra spec component qua MCP `get_component_api`/`get_component_usage`; token qua `generate_color_vars`/`lookup_token`/`search_tokens`/`suggest_token`; icon qua `list_icons`; verify bằng `validate_brand_usage` + `review_design_tokens`.
- **ffmpeg sidecar**: tải binary LGPL cho `x86_64-pc-windows-msvc` (+ macOS sau ở P5), đặt `src-tauri/binaries/ffmpeg-<target>(.exe)`, khai báo `externalBin` trong `tauri.conf.json`. Helper `ffmpeg_path()` resolve sidecar.
- **settings.rs**: struct `AppSettings { mic_id, out_dir, format, split_value, split_unit }`, đọc/ghi qua store plugin.

## Related Code Files

- Create: `package.json` (deps public: `vue`, `vuetify@^3.5`, `@tabler/icons-vue`; dev: `@vitejs/plugin-vue`, `vite-plugin-vuetify`), `vite.config.ts`, `index.html`, `src/main.ts`, `src/App.vue`, `src/styles.css`
- Create: `src/plugins/vuetify.ts`, `src/assets/styles/viui-tokens.css` (vendor MCP), `src/tokens/*` (spacing/typography vendor)
- Create: `src/design-system/vi-ui/vi-*/Vi*.vue` (bộ wrapper Vi* cần dùng) + `src/design-system/vi-ui/index.ts` (barrel)
- Create: `src/views/{RecordView,SplitView,TranscriptView}.vue`
- Create: `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/src/main.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/settings.rs`, `src-tauri/src/ffmpeg.rs`
- Create: `src-tauri/binaries/ffmpeg-x86_64-pc-windows-msvc.exe`
- Create: `README.md` (kèm prereq + ghi chú ViUI/Vuetify + lệnh dev)

## Implementation Steps

1. `npm create tauri-app` (**vue-ts** template) hoặc dựng tay; pin Tauri v2. Cài plugins shell/dialog/store/fs.
2. **Cài Vuetify (public)** + cấu hình: `npm i vuetify@^3.5 @tabler/icons-vue` + dev `@vitejs/plugin-vue vite-plugin-vuetify`; `vite.config.ts` thêm 2 plugin; `src/plugins/vuetify.ts` `createVuetify` `defaultTheme: 'inetOneDark'` (One Series dark) + Tabler iconset.
3. **Vendor token từ MCP**: chạy MCP `generate_color_vars(format=css)` → lưu `src/assets/styles/viui-tokens.css` (`--viui-color-*` gồm `products-one-*`, light+dark); tra spacing 8pt/typography/radius qua `lookup_token`/`search_tokens` → vendor `src/tokens/`. Import token css vào `main.ts`; wire palette **One Series** vào theme `inetOneDark` (primary=`products-one-green`, accent=`products-one-orange`, background=`products-one-bg`, surface=`products-one-bg-card`). Smoke-check: console `getComputedStyle(document.documentElement).getPropertyValue('--viui-color-products-one-green')` ≈ `#49c98d`. **KHÔNG hardcode hex.**
4. **Dựng wrapper `Vi*`** (theo MCP `get_component_api` + `get_component_usage` + `brand_dna_notes`): tối thiểu `ViButton, ViSelect, ViInput, ViRadioGroup`(hoặc `ViButtonToggle`)`, ViCard, ViAlert, ViIcon` — wrapper mỏng quanh Vuetify (`v-bind="$attrs"` + slot), props passthrough đúng spec. Barrel `src/design-system/vi-ui/index.ts`.
5. `tauri.conf.json`: khai báo `externalBin: ["binaries/ffmpeg"]`, **identifier `vn.inet.onescribe`**, window 1 cái (~900x600) title "OneScribe", **productName "OneScribe"**.
6. Tải ffmpeg LGPL build (Windows) → đặt đúng tên target sidecar. Viết `ffmpeg.rs::ffmpeg_command()` trả `tauri_plugin_shell` sidecar Command.
7. `commands.rs`: command `ffmpeg_version()` (smoke test) + `get_settings()/set_settings()`. Đăng ký trong `main.rs` `invoke_handler`.
8. UI shell: `App.vue` render 3 tab (tab switcher Vi*/Vuetify) + chuyển tab (`<component :is>`); 3 view rỗng (`RecordView`/`SplitView`/`TranscriptView`) placeholder dùng `ViCard`/`ViAlert`. **Chỉ Vi* + token, không raw Vuetify, không hex cứng.**
9. `settings.rs` + store: load lúc khởi động, expose qua command.
10. `README.md`: mô tả app **OneScribe**, prereq (Rust, Node, ffmpeg sidecar), ghi chú UI Vue 3 + Vuetify + ViUI token (theme One Series dark), lệnh `npm run tauri dev`.
11. Chạy `npm run tauri dev` → app mở, đổi tab OK, `ffmpeg_version()` trả version.
12. **Verify design system**: chạy ViUI MCP `validate_brand_usage` + `review_design_tokens` trên `src/**/*.{vue,css,ts}` → 0 raw-Vuetify-trong-view / 0 hardcoded value / 0 vi phạm Brand DNA.

## Todo List

- [x] Scaffold Tauri v2 + Vite **vue-ts** + plugins (OneScribe, vn.inet.onescribe)
- [x] Cài Vuetify (public) + `src/plugins/vuetify.ts` theme `inetOneDark` + Tabler iconset
- [x] Vendor token từ MCP (`viui-tokens.css` + `src/tokens/one-series.ts`) + wire vào theme
- [x] Dựng bộ wrapper Vi* (Button/Select/Input/Card/Alert/Icon/Tabs/Tab/RadioGroup/Radio) + barrel
- [x] tauri.conf.json + externalBin ffmpeg + ffprobe
- [x] ffmpeg sidecar (Windows LGPL) + ffmpeg.rs helper + ffmpeg_version command
- [x] commands.rs + main.rs invoke_handler
- [x] UI 3-tab shell (App.vue + 3 view rỗng) dùng Vi*
- [x] settings.rs + store persistence
- [x] README.md
- [x] `tauri dev` chạy được (onescribe.exe boot OK; ffmpeg_version runtime OK)
- [x] ViUI MCP `validate_brand_usage` → 0 vi phạm (App.vue). *`review_design_tokens` quét-FS không chạy được do MCP server sandbox không thấy ổ local — thay bằng validate_brand_usage trên code string + code-review.*

## Success Criteria

- [x] App mở, 3 tab chuyển được (Vi* tab switcher) — onescribe.exe boot OK
- [x] `ffmpeg_version()` trả về version từ sidecar — **runtime verified** (`ffmpeg OK: N-124986…`)
- [x] Settings: get/set_settings compile + đăng ký + store wiring (round-trip qua UI kiểm ở P2)
- [x] Mỗi file Rust < 200 dòng; mỗi `.vue`/`.ts`/CSS < 200 LOC
- [x] ViUI active: token `--viui-color-products-one-green` ≈ `#49c98d`, theme **inetOneDark**, 0 raw Vuetify trong view, 0 hex ngoài file token (code-review PASS sau fix H1)

## Risk Assessment

- ffmpeg sidecar naming sai target → Tauri không tìm thấy. Mitigation: kiểm tra đúng `rustc -Vv` host triple.
- Tauri v2 API khác v1 nhiều → bám docs v2 (`tauri-plugin-shell` sidecar API mới).
- **Wrapper Vi* tự dựng lệch ViUI gốc** (không có CSS DNA theme glass/bold/glow của `@hobui/viui`). Mitigation: bám sát `get_component_api`/`brand_dna_notes`; v1 chỉ cần component chuẩn (button/select/input/list/alert) — không dùng DNA CTA gradient. Có credentials registry → thay bằng package gốc.
- Vuetify tăng bundle vs vanilla TS. Mitigation: tree-shaking `vite-plugin-vuetify`; chấp nhận (quyết định Session 2 — UI bắt buộc ViUI mức component).
- Token chỉ vendor được màu qua `generate_color_vars`; spacing/typography phải tra lẻ. Mitigation: dùng `lookup_token`/`search_tokens` lúc vendor; ghi nguồn/version trong file vendor; không hardcode tên token.
- *(Lưu ý lịch sử)* `@hobui/viui` + `@hobui/tokens` E404 trên npm + jsDelivr (verified 2026-06-13) → không cài/CDN được; chọn hướng Vuetify public + tự dựng Vi* + vendor token.

## Next Steps

Mở khóa P2 (Mode A), P3 (Mode B), P4 (Mode C) — tất cả dùng shell + ffmpeg sidecar từ phase này.
