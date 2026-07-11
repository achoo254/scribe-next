---
phase: 1
title: "Foundation & deploy gate"
status: in-progress
priority: P1
effort: "2-3d"
dependencies: []
---

# Phase 1: Foundation & deploy gate

> **Progress 2026-07-11:** Foundation local XONG — Cargo workspace (`exclude src-tauri`), `crates/core-split` (extract từ splitter.rs + 9 unit test), `server/onescribe-be` axum skeleton (`/api/health`), `web/` Vue3/Vite scaffold. `cargo check --workspace` + `cargo test` + `vite build` pass.
>
> **✅ GATE PASSED 2026-07-11:** musl static binary build XONG (Docker `rust-musl-cross` trong WSL; rustls dùng `ring` KHÔNG aws-lc → `ELF static-pie linked`). Deploy .65 (CentOS Stream 9): user `onescribe` + systemd (`active`, 17MB, CPUQuota/MemoryMax) + nginx vhost `:80` (`nginx -t` OK → reload) + CF proxied. **`https://scribe.inetdev.io.vn/api/health` → 200** qua Cloudflare. ffmpeg/ffprobe đã sẵn trên .65 (khỏi cài — red-team #5 thỏa).
>
> **Deviation ghi nhận:** (a) SELinux DISABLED trên .65 → không cần context/boolean. (b) nginx SHARED (15 vhost) → CHỈ thêm 1 server block, KHÔNG firewall :80 chỉ-cho-CF (sẽ chặn vhost khác) → red-team #2 firewall-part BỎ, chỉ `set_real_ip_from`. (c) FE static `/var/www/onescribe/dist` (khớp convention box). (d) Desktop rewire core-split tách slice riêng (chưa đụng src-tauri). (e) FE Vue thuần (ViUI hoãn — không có registry `@hobui/*`).

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

**core-split (crate thuần):** chuyển `compute_segment_time`, `size_cut_times`, **`probe_file`** (đọc `MediaInfo` — dùng chung cho compute/split), cấu trúc lệnh ffmpeg segment (phút/MB/parts, keyframe MB-cut, HEVC hvc1) sang API độc lập Tauri. Chữ ký kiểu: `async fn split(input, criterion, value, out_dir, ffmpeg_path, **ffprobe_path**, on_progress) -> Result<Vec<OutFile>>` — **BẮT BUỘC có cả `ffprobe_path`** vì `size_cut_times` + `probe_file` spawn ffprobe, không phải ffmpeg (verify `splitter.rs:34,120`). Spawn qua `tokio::process::Command` (thay `tauri shell`); tiến độ qua callback (thay `app.emit`). Desktop `splitter.rs` giữ lớp mỏng wrap Tauri quanh core-split → **0 regression Mode B desktop** (test đối chiếu).

**[Red-team #6] Resolve path binary + window parity cho desktop (BẮT BUỘC scope trước cook):** đổi từ `app.shell().sidecar()` (auto-resolve externalBin + tự ẩn console) sang `tokio::process` làm mất 2 thứ desktop đang dựa vào — (a) **path binary bundled**: desktop wrapper phải tự resolve path ffmpeg/ffprobe sidecar (qua `tauri::path` resource resolver / `current_binary`) TRƯỚC khi truyền vào core-split, KHÔNG hard-code `"ffmpeg"`; (b) **`CREATE_NO_WINDOW`**: raw spawn trên Windows phải set `creation_flags(CREATE_NO_WINDOW)` (tiền lệ `transcribe.rs:26`, `cuda.rs:13`) nếu không sẽ nháy console mỗi lần cắt — regression vô hình với output-diff test.

**BE crates (từ research):** `axum` 0.8 + `tokio` 1.40 · `tower-http` (RequestBodyLimitLayer) · `mongodb` 3.x feature **`rustls-tls`** · `reqwest` 0.12 `rustls-tls` · `serde`. Bind `127.0.0.1:PORT`; graceful shutdown SIGTERM (systemd gửi). `env!()` đọc secret bake bởi `build.rs`. **TLS out-bound (Google JWKS/OAuth) = rustls** — tránh OpenSSL/native-tls (C lib) vì phá musl static; MongoDB local `127.0.0.1` plaintext không cần TLS.

**Build musl (từ Windows/WSL2):** Docker image **`ghcr.io/rust-cross/rust-musl-cross:x86_64-musl`** (dùng 1 ref duy nhất — đây là repo `messense/rust-musl-cross` đã chuyển sang org `rust-cross` publish chính trên ghcr.io; WSL2+rustup fail do ring/aws-lc trên musl). Script `scripts/build-musl.sh` chạy trong WSL: `docker run --rm -v $(pwd):/home/rust/src ghcr.io/rust-cross/rust-musl-cross:x86_64-musl cargo build --release --target x86_64-unknown-linux-musl -p onescribe-be`. Secret truyền qua env vars của shell build.

**[Red-team #13] Workspace không được vỡ trên máy dev thiếu secret:** `env!()` là hard-fail compile-time → nếu build/`cargo check --workspace`/rust-analyzer chạy `onescribe-be` mà thiếu `MONGO_URI`/`GOOGLE_CLIENT_ID`/… → đỏ toàn workspace kể cả code desktop. Fix: `Cargo.toml` root đặt `[workspace] default-members = ["src-tauri", "crates/core-split"]` (loại `onescribe-be` khỏi build/check mặc định); build BE tường minh bằng `-p onescribe-be`. Optional thêm `.cargo/config.toml` placeholder cho dev.

**Deploy:** scp binary → `/opt/onescribe-be/` (`chmod 600` — chứa secret); cài **ffmpeg + ffprobe** trên .65 (`apt install ffmpeg` hoặc static linux-x64 đóng kèm) — path đọc từ config, KHÔNG hard-code (xem [Red-team #5] bên dưới); **systemd** unit (`ExecStart=/opt/onescribe-be/onescribe-be`, `Restart=on-failure`, `User` riêng, **`CPUQuota=200%`** (~2/4 core), **`MemoryMax=`** để chừa tài nguyên cho MongoDB + ~15 service chung box) → `systemctl enable --now`; nginx vhost **listen :80** 2 block: `location /` → FE dist, `location /api/` → `proxy_pass 127.0.0.1:PORT` (`client_max_body_size 500M`, `proxy_request_buffering off`, `proxy_buffering off`, timeout 3600s). **[Red-team #2] Trust boundary CF:** thêm `set_real_ip_from <dải IP Cloudflare>; real_ip_header CF-Connecting-IP;` + **firewall .65 chỉ mở :80 cho dải IP Cloudflare** (deny khác) — nếu không, attacker connect thẳng `http://103.72.98.65` tự set `CF-Connecting-IP` → bypass rate-limit Phase 5. **KHÔNG cần cert/443** — CF proxied lo TLS ở edge (client↔CF=HTTPS, CF↔.65=HTTP). CF DNS record proxied `scribe.inetdev.io.vn` → 103.72.98.65 (chốt validate). <!-- Updated: Validation Session 1 - domain scribe.inetdev.io.vn -->

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

## Red-Team Fixes (áp 2026-07-11)

- **#5 ffmpeg/ffprobe trên server:** thêm bước cài `ffmpeg`+`ffprobe` trên .65 (apt hoặc static kèm); `ffmpeg_path`/`ffprobe_path` đọc từ config (compile-time hoặc file), KHÔNG hard-code. Deploy verify `which ffmpeg ffprobe` OK trước khi Phase 3.
- **#6 core-split extract:** chữ ký thêm `ffprobe_path`; đưa `probe_file` vào core-split; desktop wrapper tự resolve path sidecar + set `CREATE_NO_WINDOW` (Windows). **"0 regression" = test tự động**: script cắt cùng fixture (mp3/mp4 mẫu) bằng `splitter.rs` cũ vs `core-split` mới, so checksum + số đoạn + duration mỗi đoạn (chạy `cargo test`, không phải mở app nhìn tay). Thêm vào Related Code Files: `crates/core-split/tests/split_regression.rs` + fixture nhỏ.
- **#10 systemd resource cap:** unit thêm `CPUQuota=200%` + `MemoryMax=`; lệnh ffmpeg re-encode (Phase 4) thêm `-threads N`.
- **#13 workspace không vỡ:** `default-members` loại `onescribe-be`.
- **#2 CF trust boundary:** nginx `set_real_ip_from` + firewall :80 chỉ dải CF (cấu hình ở Phase 1, enforce rate-limit ở Phase 5).
- **Docker image:** chốt 1 ref `ghcr.io/rust-cross/rust-musl-cross:x86_64-musl`.

**Cập nhật Success Criteria:** thêm `[ ]` "test đối chiếu tự động core-split vs splitter.rs cũ pass trong `cargo test`"; `[ ]` "`which ffmpeg ffprobe` trên .65 OK"; `[ ]` "firewall :80 chỉ nhận dải CF (verify từ IP ngoài dải → bị chặn)".
