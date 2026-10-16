# Rà bảo mật: đăng nhập hoandat-auth (commit 3030e6b)

Ngày 2026-10-16. Kết quả: 2 High, 5 Medium, 5 Low. Đã sửa trong commit kế tiếp.

| Phát hiện | Sửa |
|---|---|
| Cookie lượt đăng nhập có thể bị subdomain khác ghi đè | `__Host-scribe_login`, `Path=/` khi https |
| Làm mới phiên app không kiểm lại danh tính | Xác minh ID token mới; `sub` phải khớp chủ thiết bị; chạy lại allowlist/email đã xác minh; không đạt ⟹ xoá thiết bị |
| Hai lần làm mới song song cùng refresh token | Khoá thiết bị `Locked/Busy/Missing`; trùng ⟹ 409, mất ⟹ 401 |
| `/api/auth/native/exchange` public gọi IdP không giới hạn | Giới hạn 10/phút/IP + ngân sách chung 45 lần gọi token IdP/phút (IdP cho 60/phút/IP) |
| Back-channel logout bị rate-limit, ghi jti trước khi xoá phiên | Tách khỏi `auth_limit`; kiểm jti ⟹ xoá phiên ⟹ ghi jti |
| Cookie phiên web hết hạn sớm hơn phiên trượt | `Max-Age` = trần 7 ngày; hạn thật do server giữ |
| Allowlist đổi không áp cho phiên đang sống | `require_auth` kiểm allowlist mỗi lần cache phiên hết hạn |
| Đăng xuất app để lại grant ở IdP | Thu hồi refresh token (`revocation_endpoint`) |
| ID token nhiều `aud` không kiểm `azp` | Kiểm `azp` |
| Lỗi JWKS tạm thời làm hỏng mọi đăng nhập | Giữ khoá cũ khi tải lỗi, có cooldown |
| Mọi lỗi grant bị coi là bị từ chối | Chỉ `invalid_grant` là từ chối; còn lại 503 |
| Lỗi IdP lộ chi tiết cho client | Client nhận câu ngắn, chi tiết vào log; callback 429/IdP lỗi ⟹ `/login?error=busy|idp` |

Còn mở: build iPhone cũ gọi `/api/auth/session` sẽ 404 sau deploy (cài app mới); chưa giới hạn IP nguồn của back-channel (dựa vào chữ ký logout token).

Kiểm: `cargo clippy -D warnings` sạch, 44 test + 5 test Mongo thật, web lint/typecheck/tokens/172 test; smoke dev (login 303, callback sai ⟹ `error=expired`, exchange lần 11 ⟹ 429).
