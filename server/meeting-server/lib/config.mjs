// Cấu hình từ env — KHÔNG commit secret. Xem .env.example. Thiếu secret bắt buộc → thoát sớm (fail-fast).
export function loadConfig(env = process.env) {
  const cfg = {
    port: Number(env.PORT || 8787),
    apiKey: env.LK_API_KEY || "",
    apiSecret: env.LK_API_SECRET || "",
    livekitUrl: env.LIVEKIT_URL || "", // wss:// cho client (trả kèm token cho tiện)
    hostSecret: env.HOST_SECRET || "", // pre-shared: cấp host-token (C6)
    turnSecret: env.TURN_SECRET || "", // coturn static-auth-secret (rỗng = không cấp TURN)
    turnUrls: (env.TURN_URLS || "").split(",").map((s) => s.trim()).filter(Boolean),
    turnTtlSec: Number(env.TURN_TTL_SEC || 300),
    tokenTtlSec: Number(env.TOKEN_TTL_SEC || 6 * 3600),
    captionTtlSec: Number(env.CAPTION_TTL_SEC || 86400),
    // Chỉ tin X-Forwarded-For khi ĐỨNG SAU reverse proxy tin cậy (nginx/Caddy set header này).
    // Mặc định false → dùng socket IP → chống bypass rate-limit bằng XFF giả (brute-force HOST_SECRET).
    trustProxy: env.TRUST_PROXY === "true" || env.TRUST_PROXY === "1",
    // Origin cho phép gọi (CORS). "*" cho dev; production nên liệt kê domain web client.
    allowOrigins: (env.ALLOW_ORIGINS || "*").split(",").map((s) => s.trim()).filter(Boolean),
    webhookApiKeys: (env.LK_WEBHOOK_KEYS || env.LK_API_KEY || "").split(",").map((s) => s.trim()).filter(Boolean),
  };
  return cfg;
}

/** @returns {string[]} danh sách lỗi cấu hình bắt buộc (rỗng = OK). */
export function validateConfig(cfg) {
  const errs = [];
  if (!cfg.apiKey) errs.push("LK_API_KEY thiếu");
  if (!cfg.apiSecret) errs.push("LK_API_SECRET thiếu");
  if (!cfg.hostSecret) errs.push("HOST_SECRET thiếu (cần để cấp host-token an toàn — C6)");
  return errs;
}
