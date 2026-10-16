# Research — Rust BE stack (musl static, MongoDB, OAuth, nginx)

**Date:** 2026-07-11 · Confidence ~85% (docs + GitHub issues). Snippet minh hoạ — verify version thật lúc cook.

## Crate stack (musl-safe)

| Thành phần | Crate | Version | Ghi chú |
|---|---|---|---|
| Web/async | `axum` + `tokio` | 0.8 + 1.40 | axum 0.8 đổi path syntax `/{id}` |
| Body limit/static | `tower-http` | 0.5+ | `RequestBodyLimitLayer::max(...)` (mặc định 2MB → phải nâng) |
| MongoDB | `mongodb` | 3.x | **feature `rustls-tls`** (tránh OpenSSL) |
| HTTP client | `reqwest` | 0.12 | **`rustls-tls`** / `.use_rustls_tls()` (fetch JWKS) |
| JWT verify | `jsonwebtoken` | 9.3+ | RS256 + kid match |
| OAuth | `oauth2` | 4.4+ | auth-code flow |

## Điểm mấu chốt

- **Upload lớn:** stream thẳng disk (multipart `next_field` → write từng chunk), KHÔNG buffer cả file trong RAM. `RequestBodyLimitLayer` per-route.
- **MongoDB:** `rustls-tls` bắt buộc để musl static không link OpenSSL. Atomic job claim = `find_one_and_update({status:queued}→{running}, ReturnDocument::After)`. TTL index qua `IndexOptions::expire_after`.
- **OAuth Google:** oauth2 auth-code → nhận `id_token` → `decode_header` lấy `kid` → fetch JWKS `googleapis.com/oauth2/v3/certs` (cache theo Cache-Control) → `DecodingKey::from_rsa_components(n,e)` → `Validation` RS256 + `set_audience(client_id)` + `set_issuer("https://accounts.google.com")` + leeway 60s. Refetch JWKS khi kid không thấy (key rotation).
- **musl cross-compile từ Windows:** **Docker `messense/rust-musl-cross:x86_64-musl`** (khuyến nghị) — WSL2+rustup FAIL do ring/aws-lc-rs không build được trên musl; `cross` là fallback. Verify static: `ldd binary` → "not a dynamic executable".
- **Secret compile-time:** `build.rs` đọc env build-time → `println!("cargo:rustc-env=MONGO_URI={}", ...)` → code dùng `env!("MONGO_URI")`. Pass qua env vars của shell build. `env!` fail build nếu thiếu; `option_env!` cho optional (allowlist). Đổi secret = rebuild. Secret lộ qua `strings` (chấp nhận — trade-off server sạch).
- **Process manager:** **systemd** (user chốt bỏ PM2 — PM2 kéo Node runtime ~80MB thừa). Unit `Type=simple`, `Restart=on-failure`, `User` riêng, log `journald`. Rust graceful shutdown SIGTERM: `axum::serve(...).with_graceful_shutdown(...)`.
- **nginx (listen :80, CF proxied lo TLS edge):** `client_max_body_size 500M`, `proxy_request_buffering off` (stream upload), `proxy_buffering off` + `proxy_http_version 1.1` (SSE), `proxy_read/send_timeout 3600s`. `location /` → FE dist; `location /api/` → `proxy_pass 127.0.0.1:PORT`.

## Unresolved
- Mongo pool size cho box CPU-only (~5-10 connection, test).
- JWKS cache in-process (đủ cho 1 server) vs Redis (không cần).
- Job retry/backoff (đề xuất 3 lần, 2^n s).

## Sources
- axum 0.8 release (tokio.rs) · MongoDB Rust v3.0 TLS docs · messense/rust-musl-cross · ring #1414 / aws-lc-rs musl issues · Auth0 RS256+JWKS · Leapcell compile-time secrets · systemd vs PM2 (Patrick Niyo) · nginx reverse-proxy + SSE docs.
