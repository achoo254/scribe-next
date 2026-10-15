---
phase: 1
title: "Vỏ Capacitor + transport + auth + parity không-ghi"
status: pending
priority: P1
effort: "L"
dependencies: []
---

# Phase 1: Vỏ Capacitor + transport + auth + parity không-ghi

## Overview

Dựng app iOS bọc UI Vue, giải quyết **tầng transport** (điểm sập của bản 1), đăng nhập Google native, và chứng minh mọi tính năng KHÔNG liên quan ghi âm chạy đầy đủ trên iPhone — Cắt file, Lịch sử ghi, phát lại, quota, xoá.

Phase này mở đầu bằng **ba gate xếp theo thứ tự rủi ro giảm dần**. Mỗi gate rẻ, và mỗi gate có thể huỷ phần còn lại của plan. Chạy hết ba gate trước khi viết bất kỳ dòng code sản phẩm nào.

## Requirements

**Gate (bước 0–3, làm trước)**
- Chứng minh `CapacitorHttp` gọi được backend từ thiết bị thật
- Chứng minh đăng nhập Google lấy được Firebase ID token mà BE chấp nhận
- Chứng minh `/api/recordings/finalize` nhận m4a

**Chức năng**
- Đăng nhập Google → `HomeView` 3 tab
- Tab **Cắt file**: chọn file từ Files/Photos → upload → job cắt → **tải kết quả về máy** (share sheet)
- Tab **Ghi âm**: **ẩn hoàn toàn** trên iOS cho tới Phase 3 (không dùng nhãn tạm — nhãn có số phase là artifact không ổn định)
- **Lịch sử ghi**: liệt kê, phát lại có tua, xoá, **hiển thị ngày hết hạn 14 ngày**, **nút "Lưu vào Files"** để export trước hạn <!-- Updated: Validation Session 1 - V2 export ra Files -->
- **QuotaBar** đúng
- Toggle "Tiếng hệ thống" / "Ghi kèm màn hình" không tồn tại trên iOS (D8)

**Phi chức năng**
- Không sửa file nào trong `server/**/*.rs` — **trừ khi Gate 0 fail**, khi đó đi đường CORS + Bearer đã chốt ở D3 (không hỏi lại) <!-- Updated: Validation Session 1 - V1 fallback chốt sẵn -->
- `vue-tsc` + `vite build` EXIT 0; `xcodebuild build` EXIT 0, 0 warning
- Mọi nhánh iOS nằm sau `isNative` — web không hồi quy

<!-- Updated: Validation Session 1 - V4 bỏ điều kiện tiên quyết ALLOWED_EMAILS, chủ sở hữu chọn giữ allowlist mở -->
**Không có điều kiện tiên quyết ngoài repo.** Bản trước đặt việc khoá `ALLOWED_EMAILS` làm tiên quyết; chủ sở hữu đã quyết **giữ allowlist mở** (Validation Log V4), rủi ro ghi nhận ở bảng rủi ro của `plan.md`.

## Architecture

```
┌─ iOS App (Capacitor) ────────────────────────────────────┐
│  WKWebView ← web/dist  (origin capacitor://localhost)    │
│                                                           │
│  fetch/XHR ──▶ CapacitorHttp ──▶ URLSession ──┐          │
│                (không CORS, không SameSite)    │          │
│  tải file  ──▶ DownloadPlugin ─▶ share sheet ──┤          │
│  phát media ─▶ DownloadPlugin ─▶ file tạm ─────┤          │
│                                                 │          │
│  SessionAuth: Google → Firebase ID token ──────┤          │
│               POST /api/auth/session            │          │
│               cookie → HTTPCookieStorage        │          │
└─────────────────────────────────────────────────┼──────────┘
                                                  ▼
                            https://scribe.inetdev.io.vn (BE không đổi)
```

Cookie chỉ tồn tại ở **một** kho (`HTTPCookieStorage`) vì mọi request đều phát từ native. Việc tiêm cookie sang `WKHTTPCookieStore` của bản 1 bị **xoá bỏ** — không cần thiết.

**Vị trí project**: `web/capacitor.config.ts` + `web/ios/`. Dùng chung `web/package.json`, một lệnh `npm run build && npx cap sync ios`.

## Related Code Files

- Create: `web/capacitor.config.ts` — bật plugin `CapacitorHttp`
- Create: `web/ios/App/App/Info.plist` — `UIBackgroundModes: [audio]`, `NSMicrophoneUsageDescription`, URL scheme OAuth. **Không** bật `UIFileSharingEnabled`
- Create: `web/ios/App/App/SessionAuth.swift`
- Create: `web/ios/App/App/DownloadPlugin.swift` + `.m` — tải file qua `URLSession` (dùng cookie sẵn có) → trả file URL cục bộ hoặc mở `UIActivityViewController`
- Create: `web/src/composables/use-platform.ts` — bọc `Capacitor.isNativePlatform()`
- Create: `web/src/composables/use-download.ts` — gom **5** chỗ `window.location.href`, nhánh `isNative`
- Modify: `web/src/composables/use-recordings.ts:49,53,57` · `use-splitter.ts:113` · `use-managed-files.ts:56` — gọi `use-download`
- Modify: `web/src/components/MediaPlayer.vue:72,80` — nhánh `isNative` dùng file cục bộ
- Modify: `web/src/components/RecordingHistory.vue` — cột ngày hết hạn (`expiresAt` đã có trong `files`) + nút **"Lưu vào Files"** khi `isNative` (tái dùng `DownloadPlugin` → `UIActivityViewController`) <!-- Updated: Validation Session 1 - V2 -->
- Modify: `web/src/composables/use-download.ts` — thêm `saveToFiles(fileId)` bên cạnh `download(fileId)` <!-- Updated: Validation Session 1 - V2 -->
- Modify: `web/src/views/HomeView.vue` — ẩn tab Ghi âm khi `isNative`
- Modify: `web/src/composables/use-auth.ts` — nhánh `isNative`; **lưu ý 3 consumer**: `router.ts:18` (navigation guard), `AppTopBar.vue:5,8`, `LoginView.vue:5,8`
- Modify: `web/package.json`, `web/.gitignore`
- **Commit** `web/ios/App/App/GoogleService-Info.plist` — nhất quán với cách repo đã xử lý config Firebase web (`web/.env.example` ghi rõ "PUBLIC — không phải secret"). Gitignore nó sẽ làm tiêu chí clone-sạch của Phase 4 bất khả thi

## Implementation Steps

1. **GATE 0 — transport.** Capacitor rỗng + `CapacitorHttp` + một nút gọi `GET /api/health` rồi `GET /api/me` từ thiết bị thật. Fail → dừng plan, quay lại D3 chọn phương án khác. Đây là gate rẻ nhất và huỷ nhiều nhất.
2. `npx cap init` appId `vn.inet.onescribe` (khác `vn.inet.onescribe.spike`), `npx cap add ios`, chọn Personal Team `AM5C55N339`.
3. **GATE 1 — auth.** Thử `@capacitor-firebase/authentication`. Vướng Keychain (`-34018`) → chuyển ngay sang `ASWebAuthenticationSession` → Google credential → `accounts:signInWithIdp` (REST, web API key ở `web/.env.staging`) → Firebase ID token. **Không** thử gửi Google ID token thẳng cho BE: `jwks.rs:73-75` pin `aud`/`iss` theo securetoken, chắc chắn 401.
4. **GATE 2 — m4a.** Có cookie rồi thì đẩy một file `.m4a` 5 giây (tạo bằng `afconvert -f m4af -d aac`) qua `init`/`chunk`/`finalize`. Kỳ vọng 200, file hiện trong `/api/files` với đuôi **`.m4a`** (không phải `.mp4`), `/api/files/:id` phát được. Fail → dừng, báo cáo. Gate này ở đây chứ không phải cuối Phase 2 vì nó bảo vệ toàn bộ chi phí của Phase 2.
5. `SessionAuth.swift`: sign-in + POST session + refresh chủ động T-30 phút + `logout()` (drain queue → POST logout → xoá cookie → Firebase signOut).
6. `DownloadPlugin.swift` + `use-download.ts` + nhánh `isNative` của `MediaPlayer.vue`.
7. `use-platform.ts`; ẩn tab Ghi âm; cột hết hạn trong Lịch sử ghi.
8. QA thủ công theo Success Criteria.

## Success Criteria

- [ ] **Gate 0**: `GET /api/me` trả **200** từ trong WebView qua `CapacitorHttp`, trên thiết bị thật
- [ ] **Gate 1**: đăng nhập Google → BE cấp session cookie
- [ ] **Gate 2**: m4a qua `finalize` → 200, lưu đuôi `.m4a`, phát lại được
- [ ] Cắt file: chọn file → upload 100% → job `done` → **zip nằm trong Files.app của máy** qua share sheet
- [ ] Lịch sử ghi: phát lại một bản ghi cũ tạo từ web, **tua được**
- [ ] Lịch sử ghi hiển thị ngày hết hạn; xoá file → QuotaBar giảm
- [ ] **"Lưu vào Files"** trên một bản ghi → file nằm trong Files.app của máy, phát được bằng app khác (V2)
- [ ] Tab Ghi âm không xuất hiện trên iOS; không có toggle "Tiếng hệ thống"/"Ghi kèm màn hình"
- [ ] `git diff --stat server/` trống
- [ ] `vue-tsc --noEmit` + `vite build` + `xcodebuild build` đều EXIT 0, 0 warning
- [ ] Web không hồi quy: dev local → login + cắt file + phát lại trên Chrome desktop vẫn chạy

## Risk Assessment

<!-- Updated: Validation Session 1 - V1 fallback đã chốt, không còn là quyết định treo -->
**Gate 0 fail — `CapacitorHttp` không phủ hết (CAO).** Nó patch `fetch`/`XHR` nhưng có thể vướng redirect, response streaming, hoặc header lạ. **Nếu fail: đi thẳng đường đã chốt ở D3** — thêm `CorsLayer` vào BE (`Cargo.toml` bật feature `cors` + `routes.rs`) và native đẩy session token qua bridge cho JS dùng `Authorization: Bearer`, **giữ nguyên cookie `SameSite=Lax`**. Không dừng hỏi lại. Ghi rõ trong báo cáo phase rằng cam kết zero-code-change đã bị phá và phạm vi cụ thể là gì.

**Gate 1 fail — Keychain (TRUNG BÌNH).** Lối lui `signInWithIdp` đã xác định cụ thể ở D5.3 và không cần Firebase iOS SDK, nên né được hoàn toàn rủi ro Keychain. Không đụng backend.

**Phát media phải tải trọn file (TRUNG BÌNH).** Đoạn ghi 3.73 MB không đáng kể; file Mode B tới 2GB thì cần tiến độ. Không mất chức năng, chỉ đổi trải nghiệm.

**Rollback (đã sửa so với bản 1).** Thư mục `web/ios/` xoá được sạch. Nhưng **`use-download.ts` và `MediaPlayer.vue` là thay đổi có thật cho web** — chúng refactor 5 chỗ `window.location.href` dùng chung. Rollback phải revert cả những chỗ đó, không chỉ xoá thư mục iOS. Bản 1 nói rollback là "xoá thư mục + revert 4 file" — không đúng với phạm vi này.
