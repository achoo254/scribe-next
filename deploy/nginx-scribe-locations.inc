# Phần chung của mọi server block Scribe (scribe.hoandat.com ở nginx-scribe.conf, scribe.<domain> của tenant sổ ở
# nginx-scribe-tenants.conf) — cài: /etc/nginx/scribe/locations.inc trên .65 (ngoài conf.d/: conf.d/*.conf nạp ở mức
# http). Backend chọn tenant theo Host nên /api/ giữ nguyên Host của request. Cần security-headers.inc cùng thư mục.

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

# FE static (React dist) — SPA fallback về index.html. Một bản build cho mọi domain: tenant đọc lúc chạy (/api/idp).
root /var/www/scribe/dist;

# Asset Vite có hash trong tên (index-<hash>.js/css) → nội dung bất biến → cache vĩnh viễn.
location /assets/ {
    add_header Cache-Control "public, max-age=31536000, immutable";
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
