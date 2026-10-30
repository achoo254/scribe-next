# Location của server block Scribe (scribe.hoandat.com ở nginx-scribe.conf) — cài: /etc/nginx/scribe/locations.inc trên
# .65 (ngoài conf.d/: conf.d/*.conf nạp ở mức http). Cần security-headers.inc cùng thư mục.

# API → BE axum localhost. Tắt buffering cho upload/stream (chunked + progress).
location /api/ {
    proxy_pass http://127.0.0.1:8787;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto https;
    proxy_request_buffering off;
    proxy_buffering off;
    proxy_read_timeout 3600s;
    proxy_send_timeout 3600s;
}

# MCP cho công cụ AI (docs/decisions/0002-mcp-endpoint.md): POST JSON nhỏ (≤ 1 MiB, BE tự chặn), trả JSON; câu hỏi
# `scribe_ask` có thể chờ model vài chục giây. BE tự đặt Cache-Control và kiểm Origin/token.
location = /mcp {
    proxy_pass http://127.0.0.1:8787;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto https;
    client_max_body_size 2m;
    proxy_read_timeout 180s;
}

# Protected Resource Metadata (RFC 9728) của /mcp: client MCP đọc trước khi đăng nhập ở IdP. Không có thì SPA
# fallback trả index.html và client không tìm được máy chủ đăng nhập.
location ^~ /.well-known/oauth-protected-resource {
    proxy_pass http://127.0.0.1:8787;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto https;
}

# FE static (React dist) — SPA fallback về index.html.
root /var/www/scribe/dist;

# Asset Vite có hash trong tên (index-<hash>.js/css) → nội dung bất biến → cache vĩnh viễn.
location /assets/ {
    add_header Cache-Control "public, max-age=31536000, immutable";
    include /etc/nginx/scribe/security-headers.inc;
    try_files $uri =404;
}

# Web app manifest: mime.types của nginx không có webmanifest (trước đây trả application/octet-stream),
# trình duyệt cần application/manifest+json. Không cache như index.html; bộ header bảo mật giữ nguyên.
location = /manifest.webmanifest {
    types { }
    default_type application/manifest+json;
    add_header Cache-Control "no-cache";
    include /etc/nginx/scribe/security-headers.inc;
    try_files $uri =404;
}

# SPA: index.html + fallback → KHÔNG cache (luôn revalidate) nên deploy mới hiện ngay,
# không kẹt bundle cũ trong browser cache (nguyên nhân UI hiện lệch sau khi deploy).
location / {
    try_files $uri $uri/ /index.html;
    add_header Cache-Control "no-cache";
    include /etc/nginx/scribe/security-headers.inc;
}
