// OneScribe meeting-server (Phase 5): cấp JWT LiveKit theo role + caption store TTL + TURN ephemeral.
// 1 process, zero-dep (node builtins). Local-first NATIVE (KHÔNG Docker) — chạy sau LiveKit/coturn.
//
// Bảo mật (red-team Phase 5):
//  [C6] SERVER gán role. Guest endpoint hard-code role=guest (bỏ field client). Host qua HOST_SECRET
//       + khóa 1-host/phòng (409 nếu đã có host khác).
//  [C7] Chỉ host-token có canPublishData → chỉ host bơm caption qua data channel.
//  [C8] Caption store bắt buộc JWT LiveKit hợp lệ + room khớp; push yêu cầu role=host.
//  [TURN] credential ephemeral (HMAC hết hạn ~5') — KHÔNG tĩnh.
import http from "node:http";
import crypto from "node:crypto";
import { loadConfig, validateConfig } from "./lib/config.mjs";
import { signToken, verifyToken, roleOf } from "./lib/jwt.mjs";
import { makeTurnCredential } from "./lib/turn.mjs";
import { createRateLimiter } from "./lib/rate-limit.mjs";
import { createHostLock } from "./lib/host-lock.mjs";
import { createCaptionStore } from "./lib/caption-store.mjs";

const ROOM_RE = /^[A-Za-z0-9_.:-]{1,128}$/; // room id opaque; khuyến nghị UUID (không slug đoán được — C8)

function setCors(cfg, req, res) {
  const origin = req.headers.origin;
  if (cfg.allowOrigins.includes("*")) res.setHeader("Access-Control-Allow-Origin", "*");
  else if (origin && cfg.allowOrigins.includes(origin)) res.setHeader("Access-Control-Allow-Origin", origin);
  res.setHeader("Vary", "Origin");
  res.setHeader("Access-Control-Allow-Methods", "GET,POST,OPTIONS");
  res.setHeader("Access-Control-Allow-Headers", "Content-Type,Authorization");
}

function sendJson(res, status, obj) {
  const body = JSON.stringify(obj);
  res.writeHead(status, { "Content-Type": "application/json; charset=utf-8" });
  res.end(body);
}

function readBody(req, limit = 1 << 20) {
  return new Promise((resolve, reject) => {
    let size = 0;
    const chunks = [];
    req.on("data", (c) => {
      size += c.length;
      if (size > limit) {
        reject(new Error("body quá lớn"));
        req.destroy();
      } else chunks.push(c);
    });
    req.on("end", () => resolve(Buffer.concat(chunks)));
    req.on("error", reject);
  });
}

async function readJson(req) {
  const buf = await readBody(req);
  if (!buf.length) return {};
  return JSON.parse(buf.toString("utf8"));
}

function bearer(req) {
  const h = req.headers.authorization || "";
  return h.startsWith("Bearer ") ? h.slice(7) : null;
}

function clientIp(req, trustProxy) {
  // Chỉ đọc XFF khi tin proxy — nếu không, attacker gửi XFF khác nhau mỗi request để lấy bucket mới
  // (bypass rate-limit → brute-force HOST_SECRET). Deploy trực tiếp phải dùng socket IP.
  if (trustProxy) {
    const xf = req.headers["x-forwarded-for"];
    if (xf) return String(xf).split(",")[0].trim();
  }
  return req.socket.remoteAddress || "?";
}

function safeEq(a, b) {
  const ab = Buffer.from(String(a));
  const bb = Buffer.from(String(b));
  return ab.length === bb.length && crypto.timingSafeEqual(ab, bb);
}

function turnFor(cfg, name) {
  if (!cfg.turnSecret || !cfg.turnUrls.length) return null;
  return makeTurnCredential(cfg.turnSecret, name, cfg.turnUrls, cfg.turnTtlSec);
}

export function createApp(cfg, deps = {}) {
  const limiter = deps.limiter || createRateLimiter({ capacity: 20, refillPerSec: 1 });
  const hostLock = deps.hostLock || createHostLock();
  const captions = deps.captions || createCaptionStore({ ttlSec: cfg.captionTtlSec });

  async function handler(req, res) {
    setCors(cfg, req, res);
    if (req.method === "OPTIONS") return void res.writeHead(204).end();

    const parsed = new URL(req.url, "http://localhost");
    const path = parsed.pathname;

    try {
      if (path === "/healthz") return sendJson(res, 200, { ok: true });

      // ── Token guest: role gán cứng server, bỏ mọi field role của client (C6) ──────────────
      if (req.method === "POST" && path === "/token/guest") {
        if (!limiter.allow(clientIp(req, cfg.trustProxy))) return sendJson(res, 429, { error: "quá nhiều yêu cầu" });
        const b = await readJson(req);
        if (!ROOM_RE.test(b.room || "")) return sendJson(res, 400, { error: "room không hợp lệ" });
        // Identity guest LUÔN server-generate: chặn guest chiếm namespace "host-" (M3) → không thể
        // giả identity host để kích hoạt nhả host-lock qua webhook. Guest KHÔNG cần identity ổn định.
        const identity = `guest-${crypto.randomUUID()}`;
        const token = signToken({
          apiKey: cfg.apiKey, apiSecret: cfg.apiSecret, identity,
          name: String(b.name || identity).slice(0, 80), room: b.room, role: "guest",
          ttlSec: cfg.tokenTtlSec,
        });
        return sendJson(res, 200, { token, url: cfg.livekitUrl, identity, role: "guest", turn: turnFor(cfg, identity) });
      }

      // ── Token host: cần HOST_SECRET + khóa 1-host/phòng (C6) ──────────────────────────────
      if (req.method === "POST" && path === "/token/host") {
        if (!limiter.allow(clientIp(req, cfg.trustProxy))) return sendJson(res, 429, { error: "quá nhiều yêu cầu" });
        const b = await readJson(req);
        if (!ROOM_RE.test(b.room || "")) return sendJson(res, 400, { error: "room không hợp lệ" });
        if (!cfg.hostSecret || !safeEq(b.secret || "", cfg.hostSecret)) {
          return sendJson(res, 403, { error: "sai host secret" });
        }
        const identity = String(b.identity || `host-${crypto.randomUUID()}`);
        if (!hostLock.acquire(b.room, identity)) {
          return sendJson(res, 409, { error: "phòng đã có host khác" });
        }
        const token = signToken({
          apiKey: cfg.apiKey, apiSecret: cfg.apiSecret, identity,
          name: String(b.name || identity).slice(0, 80), room: b.room, role: "host",
          canPublishData: true, ttlSec: cfg.tokenTtlSec,
        });
        return sendJson(res, 200, { token, url: cfg.livekitUrl, identity, role: "host", turn: turnFor(cfg, identity) });
      }

      // ── Caption store: /captions/:room — bắt buộc JWT hợp lệ + room khớp (C8) ──────────────
      const capMatch = path.match(/^\/captions\/([^/]+)$/);
      if (capMatch) {
        const room = decodeURIComponent(capMatch[1]);
        const tok = bearer(req);
        const v = tok ? verifyToken(tok, cfg.apiSecret, room) : { ok: false, error: "thiếu token" };
        if (!v.ok) return sendJson(res, 401, { error: v.error });

        if (req.method === "GET") {
          return sendJson(res, 200, { captions: captions.history(room) });
        }
        if (req.method === "POST") {
          if (roleOf(v.payload) !== "host") return sendJson(res, 403, { error: "chỉ host được ghi caption" });
          const c = await readJson(req);
          if (c && typeof c.text === "string" && typeof c.track === "string") {
            captions.push(room, {
              track: c.track, name: String(c.name || c.track), kind: c.kind === "partial" ? "partial" : "final",
              text: c.text, t0: Number(c.t0) || 0, t1: Number(c.t1) || 0,
            });
          }
          return sendJson(res, 200, { ok: true });
        }
      }

      // ── LiveKit webhook: nhả host-lock khi host rời (participant_left) ─────────────────────
      if (req.method === "POST" && path === "/webhook") {
        const raw = await readBody(req);
        if (!verifyWebhook(req, raw, cfg)) return sendJson(res, 401, { error: "webhook không hợp lệ" });
        let ev;
        try {
          ev = JSON.parse(raw.toString("utf8"));
        } catch {
          return sendJson(res, 400, { error: "payload lỗi" });
        }
        if (ev.event === "participant_left" && ev.room?.name && ev.participant?.identity) {
          // Chỉ nhả theo ROLE đã ký trong metadata (server gán, không spoof được) — KHÔNG dựa
          // heuristic prefix "host-" (guest có thể tự đặt identity đó, M3). release() vẫn khớp identity.
          if (roleOf(ev.participant) === "host") {
            hostLock.release(ev.room.name, ev.participant.identity);
          }
        }
        return sendJson(res, 200, { ok: true });
      }

      return sendJson(res, 404, { error: "not found" });
    } catch (e) {
      return sendJson(res, 400, { error: String(e.message || e) });
    }
  }

  return { handler, limiter, hostLock, captions };
}

// Webhook LiveKit: Authorization = JWT có claim sha256 = base64(sha256(body)), ký HS256 bằng apiSecret.
function verifyWebhook(req, raw, cfg) {
  const tok = bearer(req);
  if (!tok || tok.split(".").length !== 3) return false;
  const [h, p, sig] = tok.split(".");
  // Chấp nhận nếu KHỚP với bất kỳ webhook key nào (thường = LK_API_SECRET).
  const okSig = cfg.webhookApiKeys.some((secret) => {
    const expect = crypto.createHmac("sha256", secret).update(`${h}.${p}`).digest("base64url");
    const a = Buffer.from(sig);
    const b = Buffer.from(expect);
    return a.length === b.length && crypto.timingSafeEqual(a, b);
  });
  if (!okSig && cfg.apiSecret) {
    const expect = crypto.createHmac("sha256", cfg.apiSecret).update(`${h}.${p}`).digest("base64url");
    const a = Buffer.from(sig);
    const b = Buffer.from(expect);
    if (a.length !== b.length || !crypto.timingSafeEqual(a, b)) return false;
  } else if (!okSig) {
    return false;
  }
  try {
    const claim = JSON.parse(Buffer.from(p, "base64url").toString("utf8"));
    const digest = crypto.createHash("sha256").update(raw).digest("base64");
    return !claim.sha256 || claim.sha256 === digest; // sha256 khớp body (chống replay body khác)
  } catch {
    return false;
  }
}

function main() {
  const cfg = loadConfig();
  const errs = validateConfig(cfg);
  if (errs.length) {
    console.error("[meeting-server] cấu hình thiếu:\n  - " + errs.join("\n  - "));
    console.error("Xem server/meeting-server/.env.example");
    process.exit(1);
  }
  const app = createApp(cfg);
  setInterval(() => {
    app.captions.sweep();
    app.limiter.sweep();
  }, 60_000).unref();
  http.createServer(app.handler).listen(cfg.port, () => {
    console.log(`[meeting-server] nghe :${cfg.port} (LiveKit ${cfg.livekitUrl || "?"})`);
  });
}

// Chạy trực tiếp = khởi động server; import (test) = chỉ lấy createApp.
if (process.argv[1] && import.meta.url === `file://${process.argv[1]}`) main();
