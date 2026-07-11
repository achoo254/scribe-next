---
phase: 1
title: "Foundation & deploy gate"
status: pending
priority: P1
effort: "2-3d"
dependencies: []
---

# Phase 1: Foundation & deploy gate

## Overview

Dựng bộ khung: **Cargo workspace** (extract `core-split` từ `splitter.rs`), **BE axum skeleton** build ra **1 static musl binary**, deploy lên box .65 qua **nginx (listen :80) + systemd** và **chứng minh binary chạy sạch (không lib/env/node_modules)** — GATE de-risk trước khi xây tính năng. Kèm scaffold FE web (Vue3/Vite/ViUI).

## Requirements

- **Functional:** `GET /api/health` trả 200 qua nginx; FE static (trang trắng có ViUI) served tại `/`.
- **Non-functional:** BE = 1 binary musl static (`ldd` → "not a dynamic executable"); server .65 KHÔNG có Rust runtime/env/node_modules; secret nhúng compile-time; TLS out-bound = rustls.

## Architecture

Cấu trúc monorepo (workspace Rust + 2 FE Vite tách):

```
(root)
├── Cargo.toml              # [workspace] members: src-tauri, crates/core-split, server/onescribe-be
├── crates/core-split/      # NEW — thuật toán cắt thuần (từ splitter.rs), KHÔNG Tauri
├── server/onescribe-be/    # NEW — axum BE (bin: onescribe-be)
│   ├── build.rs            # bake env → cargo:rustc-env
│   └── src/{main,config,routes,db,worker}.rs
├── src-tauri/              # desktop — refactor splitter.rs GỌI core-split (giữ hành vi)
├── src/                    # desktop FE (giữ nguyên)
└── web/                    # NEW — web FE Vue3/Vite/ViUI (port views ở phase sau)
```

**core-split (crate thuần):** chuyển `compute_segment_time`, `size_cut_times`, cấu trúc lệnh ffmpeg segment (phút/MB/parts, keyframe MB-cut, HEVC hvc1) sang API độc lập Tauri. Chữ ký kiểu: `async fn split(input, criterion, value, out_dir, ffmpeg_path, on_progress) -> Result<Vec<OutFile>>`. Spawn ffmpeg qua `tokio::process::Command` (thay `tauri shell`); tiến độ qua callback (thay `app.emit`). Desktop `splitter.rs` giữ lớp mỏng wrap Tauri quanh core-split → **0 regression Mode B desktop** (test đối chiếu).

**BE crates (từ research):** `axum` 0.8 + `tokio` 1.40 · `tower-http` (RequestBodyLimitLayer) · `mongodb` 3.x feature **`rustls-tls`** · `reqwest` 0.12 `rustls-tls` · `serde`. Bind `127.0.0.1:PORT`; graceful shutdown SIGTERM (systemd gửi). `env!()` đọc secret bake bởi `build.rs`. **TLS out-bound (Google JWKS/OAuth) = rustls** — tránh OpenSSL/native-tls (C lib) vì phá musl static; MongoDB local `127.0.0.1` plaintext không cần TLS.

**Build musl (từ Windows/WSL2):** Docker `messense/rust-musl-cross:x86_64-musl` (WSL2+rustup fail do ring/aws-lc trên musl). Script `scripts/build-musl.sh` chạy trong WSL: `docker run --rm -v $(pwd):/home/rust/src ghcr.io/rust-cross/rust-musl-cross:x86_64-musl cargo build --release --target x86_64-unknown-linux-musl -p onescribe-be`. Secret truyền qua env vars của shell build.

**Deploy:** scp binary → `/opt/onescribe-be/`; **systemd** unit (`ExecStart=/opt/onescribe-be/onescribe-be`, `Restart=on-failure`, `User` riêng) → `systemctl enable --now`; nginx vhost **listen :80** 2 block: `location /` → FE dist, `location /api/` → `proxy_pass 127.0.0.1:PORT` (`client_max_body_size 500M`, `proxy_request_buffering off`, `proxy_buffering off`, timeout 3600s); **KHÔNG cần cert/443** — CF proxied lo TLS ở edge (client↔CF=HTTPS, CF↔.65=HTTP). CF DNS proxied trỏ domain.

## Related Code Files

- Create: `Cargo.toml` (workspace root), `crates/core-split/{Cargo.toml,src/lib.rs}`, `server/onescribe-be/{Cargo.toml,build.rs,src/main.rs,src/config.rs,src/routes.rs}`, `scripts/build-musl.sh`, `deploy/nginx-onescribe.conf` (listen :80), `deploy/onescribe-be.service` (systemd), `web/{package.json,vite.config.ts,index.html,src/main.ts}`
- Modify: `src-tauri/Cargo.toml` (thêm dep core-split), `src-tauri/src/splitter.rs` (wrap core-split), `src-tauri/src/ffmpeg.rs` (giữ nguyên cho desktop)
- Delete: none

## Implementation Steps

1. Tạo workspace `Cargo.toml` (members: src-tauri, crates/core-split, server/onescribe-be). Verify `cargo build` desktop vẫn pass.
2. Extract `crates/core-split`: bê thuật toán thuần từ `splitter.rs` (compute_segment_time, size_cut_times, split_one lệnh ffmpeg), bỏ Tauri, dùng `tokio::process` + callback progress. Unit test thuật toán (compute_segment_time các tiêu chí, parse keyframe csv).
3. Refactor `src-tauri/src/splitter.rs` gọi core-split; giữ event `split_progress/split_file_done/...`. Chạy app desktop verify Mode B **0 regression**.
4. Scaffold `server/onescribe-be` axum: `GET /api/health` → `{status:"ok"}`; bind 127.0.0.1; graceful shutdown; `config.rs` đọc `env!("MONGO_URI")`/OAuth (chưa dùng, chỉ verify bake).
5. `build.rs`: đọc env build-time → `cargo:rustc-env`. `scripts/build-musl.sh` (Docker messense). Build → verify `ldd` "not a dynamic executable".
6. Deploy .65: scp binary, viết `deploy/onescribe-be.service` (systemd) + `systemctl enable --now`, viết `deploy/nginx-onescribe.conf` (listen :80, 2 block), `nginx -t` + reload. CF DNS proxied domain.
7. **GATE:** từ ngoài internet gọi `https://<domain>/api/health` → 200; kiểm `/opt/onescribe-be` chỉ có binary (không node_modules/env); FE static served.
8. Scaffold `web/` Vue3/Vite/ViUI (cài `@hobui/viui @hobui/tokens vuetify @tabler/icons-vue`), trang trắng có ViUI, `vite build` → dist → nginx serve.

## Success Criteria

- [ ] `cargo build` workspace pass; desktop Mode B chạy **0 regression** (đối chiếu output cắt trước/sau refactor).
- [ ] `core-split` có unit test (compute_segment_time phút/MB/parts + parse keyframe) pass.
- [ ] BE build ra binary musl; `ldd` báo "not a dynamic executable".
- [ ] `https://<domain>/api/health` → 200 qua nginx; `/opt/onescribe-be` KHÔNG có env/node_modules/lib ngoài.
- [ ] FE `web/` build static, nginx serve trang có ViUI.

## Risk Assessment

- **musl + ring/aws-lc build fail:** dùng Docker messense image (đã verify trong research); nếu vẫn fail → thử `cross` hoặc pin rustls provider `ring`. **Đây là gate — xử lý trước khi qua Phase 2.**
- **core-split refactor phá Mode B desktop:** test đối chiếu output byte-for-byte trước/sau; giữ nguyên chữ ký event.
- **Quản process = systemd** (user chốt bỏ PM2 vì BE Rust không cần Node runtime của PM2): unit `Restart=on-failure` + `journald` log; nhẹ, chuẩn Linux, không thêm dependency.
- **CF Flexible (CF↔.65 = HTTP):** traffic CF→origin không mã hoá — chấp nhận cho nội bộ; nếu cần chặt hơn về sau → bật CF Full + cert origin (không đổi app).
- **Bind 0.0.0.0 nhầm → BE lộ public:** hard-code `127.0.0.1`; verify `ss -tlnp` chỉ localhost.
