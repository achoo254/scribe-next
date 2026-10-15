---
phase: 1
title: "Khung dự án, token và primitive UI"
status: pending
priority: P1
effort: M
dependencies: []
---

# Phase 1: Khung dự án, token và primitive UI

## Overview

Thay khung `web/` bằng đúng stack và design system của `localman-next`, chưa có tính năng. Kết thúc phase, một trang demo hiển thị đủ primitive ở cả sáng lẫn tối và `check:tokens` chạy sạch.

## Nguồn chép từ `localman-next` (`~/inet-gitlab/localman-next`)

| Nguồn | Đích | Ghi chú |
|---|---|---|
| `packages/design-tokens/tokens.css` | `web/src/styles/tokens.css` | Chép nguyên, đổi header comment sang OneScribe |
| `apps/web/src/styles/global.css` | `web/src/styles/global.css` | Giữ `@theme inline reference`, bỏ phần riêng của Localman (grid canvas, tree) |
| `apps/web/src/styles/fonts-*.css` | `web/src/styles/` | Inter + JetBrains Mono, subset latin/latin-ext/vietnamese, tự host qua `@fontsource` |
| `scripts/check-tokens.ts` + `scripts/check-tokens/` | `web/scripts/` | Đổi đường dẫn quét sang `web/src` |
| `biome.json` | `web/biome.json` | Giữ rule, bỏ rule riêng monorepo |
| `apps/web/src/components/ui/` | `web/src/components/ui/` | Chỉ lấy: `button`, `icon-button`, `card`, `alert`, `badge`, `input`, `switch`, `tabs`, `toast`, `tooltip`, `modal`, `modal-footer`, `confirm-modal`, `empty-state`, `skeleton`, `progress-bar`, `panel-progress`, `segmented-control`, `form-field`, kèm test tương ứng |

## Requirements

- React 19, Vite 7, Tailwind 4 (`@tailwindcss/vite`), React Router 7, TanStack Query 5, Radix primitives cần cho các primitive đã chọn, `@tabler/icons-react`. Version ghim theo `localman-next/apps/web/package.json` lúc thực hiện.
- npm (giữ như `web/` hiện tại và `docs/deploy-web-staging.md`), có `package-lock.json`.
- Scripts: `dev`, `build` (`tsc -b && vite build --mode staging`), `lint` (biome), `typecheck`, `test` (vitest), `check:tokens`.
- Vite dev proxy `/api` → `127.0.0.1:8787` như `web/vite.config.ts`.
- Sáng/tối theo `prefers-color-scheme`, có nút chuyển `data-theme` như `localman-next` (`theme-toggle`).
- Wordmark `OneScribe`: JetBrains Mono đậm, chữ "One" màu `--c-primary` (cùng quy ước wordmark Localman).
- `index.html`: `lang="vi"`, `theme-color` lấy theo token nền, không nháy màu khi tải.

## Related code files

- Trước khi bắt đầu: ghi SHA commit cuối của bản Vue vào plan.md (mục Rủi ro).
- Move: `web/src/` → `web/src-vue-legacy/` (bằng `git mv`, giữ lịch sử). Thư mục này chỉ để đọc khi chuyển logic ở Phase 2–3, bị loại khỏi `tsconfig`, biome và `check:tokens`, xoá ở Phase 4.
- Thay: `web/package.json`, `vite.config.ts`, `tsconfig*.json`, `index.html`.
- Create: `web/biome.json`, `web/src/main.tsx`, `web/src/styles/*`, `web/src/components/ui/*`, `web/src/components/brand/wordmark.tsx`, `web/src/components/theme-toggle.tsx`, `web/scripts/check-tokens*`

## Implementation steps

1. Scaffold Vite React TS trong `web/`, thêm Tailwind 4, biome, vitest.
2. Chép token, global.css, font; chạy `check:tokens` sạch.
3. Chép primitive đã chọn cùng test; sửa import alias.
4. Trang `/__ui` (chỉ ở `import.meta.env.DEV`) hiển thị mọi primitive để soát sáng/tối.

## Success Criteria

- [ ] `npm ci && npm run lint && npm run typecheck && npm test && npm run check:tokens && npm run build` EXIT 0 trong `web/`.
- [ ] Trang `/__ui` đúng màu ở cả sáng lẫn tối; đổi `data-theme` đổi màu ngay.
- [ ] Không có mã màu hay khoảng cách px ngoài `tokens.css` (theo `check:tokens`).
