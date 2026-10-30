# ADR 0002: Endpoint MCP chỉ đọc cho công cụ AI

- Trạng thái: Đã chấp nhận
- Ngày: 2026-10-30
- Theo hợp đồng của Chat MCP (`chat-next/docs/adr/0012-mcp-endpoint.md`) và mục 9 của `auth-next/docs/integration-guide.md`.

## Bối cảnh

Người dùng muốn cho công cụ AI (Claude Code và các client MCP khác) đọc bản ghi của mình trên Scribe: tìm, đọc bản
chép lời, biên bản, bản dịch, hỏi đáp, như Chat MCP và Localman MCP đã làm cho app của chúng. Scribe chỉ có phiên
riêng (cookie web, token thiết bị), không nhận access token của IdP. Chép lời tốn tiền thật (ElevenLabs/OpenAI) nên
vòng này không có tool nào bắt đầu việc trả tiền theo phút (quyết định 30/10/2026).

## Quyết định

### Endpoint và xác thực

- `https://scribe.<domain>/mcp` (`PUBLIC_URL` + `/mcp`), MCP Streamable HTTP không trạng thái, trả JSON (không SSE:
  GET và DELETE trả 405), mỗi POST đúng một message JSON-RPC (batch 400), body tối đa 1 MiB. Phiên bản giao thức:
  `2025-11-25`, `2025-06-18`, `2025-03-26`, `2024-11-05`.
- Chỉ nhận `Authorization: Bearer` với access token của IdP: JWT ES256, `typ: at+jwt`, `iss` là `OIDC_ISSUER`, `aud`
  chứa `PUBLIC_URL/mcp`, còn hạn, có `sub` và `client_id` (`Oidc::verify_access_token`, `server/scribe-be/src/oidc.rs`).
  Xác minh offline bằng cùng bộ JWKS đã cache cho đăng nhập; không nhận cookie phiên.
- Protected Resource Metadata (RFC 9728) ở `/.well-known/oauth-protected-resource` và
  `/.well-known/oauth-protected-resource/mcp`: `authorization_servers` là IdP, `scopes_supported` là `["scribe:read"]`.
- Thiếu token: 401 kèm `WWW-Authenticate: Bearer scope="scribe:read", resource_metadata="…"`. Token sai: 401
  `invalid_token`. Token không có `scribe:read`: 403 `insufficient_scope`. IdP chưa tải được JWKS: 503 `Retry-After: 5`.
  Request có `Origin` khác `PUBLIC_URL`: 403. Mỗi tài khoản IdP tối đa 120 request một phút.
- `sub` của token là `idpSub` của user (`users`, index `tenantId_idpSub_unique`). Tài khoản chưa từng đăng nhập Scribe
  nhận lỗi tool `scribe_account_required`; tài khoản bị khoá, đang xoá hoặc ngoài allowlist email nhận
  `account_blocked`, `account_pending_deletion`, `account_not_allowed`, đúng các điều kiện mà phiên web kiểm.

### Scope và tool

Một scope `scribe:read`, mọi tool chỉ đọc (`readOnlyHint: true`):

| Tool | Gọi handler |
|---|---|
| `scribe_list_transcripts` | `GET /api/transcript-docs` |
| `scribe_list_files` | `GET /api/files` (`source=history`, thêm `transcriptKey`) hoặc `GET /api/files/manage` (`source=split`) |
| `scribe_get_file` | lọc từ hai danh sách trên |
| `scribe_list_jobs` | `job::list_owned` (không có route danh sách; cùng điều kiện sở hữu với `GET /api/jobs/{id}`) |
| `scribe_get_job` | `GET /api/jobs/{id}` |
| `scribe_get_transcript_status` | `GET /api/transcript-docs/{key}/status` |
| `scribe_get_transcript` | `GET /api/transcript-docs/{key}`, phân trang theo câu |
| `scribe_export_transcript` | `GET /api/transcripts/export` (txt, srt, vtt, md, docx; `lang`) |
| `scribe_get_summary`, `scribe_get_speakers`, `scribe_get_bookmarks` | `GET /api/transcript-docs/{key}` |
| `scribe_get_translation` | `GET /api/transcript-docs/{key}/translation`, phân trang theo câu |
| `scribe_search` | `GET /api/search` |
| `scribe_ask` | `POST /api/ask`: **tốn hạn mức** (một lượt hỏi trong ngày, cộng vào trần chi phí tháng), mô tả tool nói rõ |
| `scribe_get_usage` | `GET /api/me` (gói, phút chép lời) và `GET /api/me/usage` (dung lượng) |

- Tool gọi thẳng hàm handler của `/api` trong cùng tiến trình với `UserId` của token (`server/scribe-be/src/mcp/calls.rs`),
  nên kiểm sở hữu, giới hạn tần suất, quyền chép lời và hạn mức giống hệt app.
- Không có tool tải lên, ghi âm, chép lời, tóm tắt, dịch, sửa, chia sẻ, xoá, key riêng hay token chép trực tiếp.
- Kết quả có `structuredContent` và bản chữ JSON; danh sách và câu được phân trang (`offset`, `limit`, `hasMore`,
  `nextOffset`) với trần khoảng 25.000 ký tự một trang; bản xuất dạng chữ cắt ở 100.000 ký tự, docx trả dạng resource
  nhúng tối đa 4 MiB. Lỗi là kết quả tool `isError: true`, `{"error":{"code","message"}}`, mã lấy từ handler (vd
  `DAILY_LIMIT`) hoặc theo HTTP status (`not_found`, `invalid_argument`, `rate_limited`…).

### Tự viết JSON-RPC thay vì SDK `rmcp`

Endpoint chỉ cần `initialize`, `ping`, `tools/list`, `tools/call` và trả JSON không trạng thái; phần khó (xác thực,
Origin, giới hạn, ánh xạ user) phải tự viết quanh SDK dù có dùng SDK. Tự viết khoảng 100 dòng (`mcp/rpc.rs`) giữ binary
musl không thêm cây phụ thuộc (SDK kéo theo `schemars`, macro, transport SSE) và không buộc nâng toolchain theo SDK.

## Hệ quả

- Công cụ AI được cấp `scribe:read` đọc được nội dung bản ghi và gửi chúng tới nhà cung cấp AI do người dùng chọn;
  trang consent của IdP nói rõ quyền này, người dùng thu hồi ở `/account` của IdP (token đang có sống tối đa 15 phút).
- `scribe_ask` gọi model bằng key của server (hoặc key riêng của người dùng) và trừ hạn mức như hỏi trong app; prompt
  injection có thể khiến công cụ AI hỏi nhiều lần, giới hạn bởi hạn mức ngày, 10 câu một phút và trần chi phí tháng.
- Hạn mức 120 request một phút nằm trong bộ nhớ tiến trình: hợp với một tiến trình `scribe-be`.

## Cách thay đổi

- Thêm tool đọc: khai ở `mcp/tools.rs`, chạy ở `mcp/calls.rs` bằng handler sẵn có.
- Thêm tool ghi hoặc tool tốn phút chép lời: cần scope mới khai ở IdP trước (`auth-next/src/oidc/resources.ts`,
  `SCRIBE_MCP_SCOPES`, câu consent), deploy IdP, rồi mới dùng ở Scribe; ADR mới cho phần tiền.
- Đổi đường dẫn `/mcp` hay domain: đổi resource ở IdP (`tenant-resources.ts`) cùng lúc; client MCP phải kết nối lại.

Nguồn: `server/scribe-be/src/mcp/`, `server/scribe-be/src/oidc.rs` (`verify_access_token`), test `mcp/tests.rs`.
