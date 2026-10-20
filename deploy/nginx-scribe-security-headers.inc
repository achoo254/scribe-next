# /etc/nginx/scribe/security-headers.inc trên .65 — KHÔNG đặt trong conf.d/ (conf.d/*.conf nạp ở
# mức http → áp cho MỌI vhost của box shared). Chỉ nginx-scribe.conf include file này.
#
# nginx: một block có add_header riêng thì KHÔNG thừa kế add_header của block cha, nên
# nginx-scribe.conf include file này ở mức server (cho /api/) VÀ trong từng location có
# add_header (/assets/, /). `always`: cả trang lỗi (404, 413, 502) cũng mang header.
#
# Đây là nguồn duy nhất của bộ header: web/e2e/csp/ đọc thẳng file này để phục vụ dist và chạy
# e2e với CSP thật (pnpm run test:e2e:csp), nên sửa ở đây là kiểm được ở đó.

# HSTS 1 năm, gồm subdomain; KHÔNG preload (preload khó rút lại).
add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
add_header X-Content-Type-Options "nosniff" always;
add_header Referrer-Policy "strict-origin-when-cross-origin" always;
# Ghi âm trên web cần micro (getUserMedia) và chia sẻ màn hình/âm thanh hệ thống
# (getDisplayMedia, web/src/features/recorder/media-capture.ts) — chỉ cho chính origin.
add_header Permissions-Policy "camera=(), geolocation=(), microphone=(self), display-capture=(self)" always;
# CSP — lập từ những gì app thực sự dùng (web/src + web/dist):
# - script-src: bundle Vite từ 'self' + hash của script inline áp theme trước first paint
#   (web/index.html). Sửa script đó thì hash đổi: pnpm run test:e2e:csp báo vi phạm, cập nhật hash.
# - style-src 'unsafe-inline': Radix (Dialog/DropdownMenu, qua react-remove-scroll) chèn thẻ
#   <style> lúc chạy; style={{…}} của React đi qua CSSOM nên không cần. Không có nonce trên site tĩnh.
# - connect-src/media-src/font-src/img-src 'self': API, stream media /api/files/…, font tự host,
#   icon — đều cùng origin; IdP (auth.hoandat.com) chỉ là điều hướng, không phải fetch.
# - form-action 'self': form của app đều xử lý bằng JS (onSubmit), không post ra ngoài.
# - frame-ancestors 'none': không cho trang nào nhúng Scribe trong iframe (clickjacking).
add_header Content-Security-Policy "default-src 'self'; script-src 'self' 'sha256-xm6mYjbQROrwta3dXJ1qt/VXzqssPzi4affxRPKBh3g='; style-src 'self' 'unsafe-inline'; img-src 'self'; font-src 'self'; connect-src 'self'; media-src 'self'; manifest-src 'self'; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'" always;
