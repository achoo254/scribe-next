// Test meeting-server qua HTTP thật (ephemeral port) — verify role gán server (C6), guest không
// publishData (C7), caption store auth JWT + room khớp + chỉ host ghi (C8), host-lock 409, TURN.
// Chạy: node server/meeting-server/test/run-tests.mjs   (zero-dep)
import http from "node:http";
import assert from "node:assert/strict";
import { createApp } from "../index.mjs";
import { createRateLimiter } from "../lib/rate-limit.mjs";
import { verifyToken, roleOf } from "../lib/jwt.mjs";

const CFG = {
  apiKey: "devkey",
  apiSecret: "devsecret-abcdefghijklmnopqrstuv",
  livekitUrl: "ws://localhost:7880",
  hostSecret: "s3cr3t-host",
  turnSecret: "turnsecret",
  turnUrls: ["turn:localhost:3478?transport=udp"],
  turnTtlSec: 300,
  tokenTtlSec: 3600,
  captionTtlSec: 86400,
  allowOrigins: ["*"],
  webhookApiKeys: ["devsecret-abcdefghijklmnopqrstuv"],
};

function startServer(deps) {
  const app = createApp(CFG, deps);
  const srv = http.createServer(app.handler);
  return new Promise((resolve) => srv.listen(0, () => resolve({ srv, port: srv.address().port, app })));
}

function req(port, method, path, { body, token, xff } = {}) {
  return new Promise((resolve, reject) => {
    const data = body ? Buffer.from(JSON.stringify(body)) : null;
    const headers = { "Content-Type": "application/json" };
    if (token) headers.Authorization = `Bearer ${token}`;
    if (xff) headers["X-Forwarded-For"] = xff;
    if (data) headers["Content-Length"] = data.length;
    const r = http.request({ host: "127.0.0.1", port, method, path, headers }, (res) => {
      const chunks = [];
      res.on("data", (c) => chunks.push(c));
      res.on("end", () => {
        const txt = Buffer.concat(chunks).toString("utf8");
        let json = null;
        try {
          json = JSON.parse(txt);
        } catch {
          /* non-json */
        }
        resolve({ status: res.statusCode, json });
      });
    });
    r.on("error", reject);
    if (data) r.write(data);
    r.end();
  });
}

const TESTS = [];
const test = (name, fn) => TESTS.push([name, fn]);

test("guest token: role gán server, KHÔNG publishData (C6/C7)", async () => {
  const { srv, port } = await startServer();
  try {
    const r = await req(port, "POST", "/token/guest", { body: { room: "r1", name: "Khách", role: "host" } });
    assert.equal(r.status, 200);
    assert.equal(r.json.role, "guest"); // bỏ qua role=host client tự khai
    const v = verifyToken(r.json.token, CFG.apiSecret, "r1");
    assert.ok(v.ok, v.error);
    assert.equal(roleOf(v.payload), "guest");
    assert.equal(v.payload.video.canPublishData, false); // C7
    assert.equal(v.payload.video.canPublish, true);
    assert.ok(r.json.turn && r.json.turn.username && r.json.turn.credential); // TURN ephemeral
  } finally {
    srv.close();
  }
});

test("host token: sai secret → 403; đúng → role host + publishData", async () => {
  const { srv, port } = await startServer();
  try {
    const bad = await req(port, "POST", "/token/host", { body: { room: "r1", name: "Chủ", secret: "sai" } });
    assert.equal(bad.status, 403);
    const ok = await req(port, "POST", "/token/host", { body: { room: "r1", identity: "host-1", secret: CFG.hostSecret } });
    assert.equal(ok.status, 200);
    assert.equal(ok.json.role, "host");
    const v = verifyToken(ok.json.token, CFG.apiSecret, "r1");
    assert.equal(roleOf(v.payload), "host");
    assert.equal(v.payload.video.canPublishData, true);
  } finally {
    srv.close();
  }
});

test("host-lock: host thứ 2 (identity khác) cùng phòng → 409; cùng identity → idempotent", async () => {
  const { srv, port } = await startServer();
  try {
    const a = await req(port, "POST", "/token/host", { body: { room: "rL", identity: "host-A", secret: CFG.hostSecret } });
    assert.equal(a.status, 200);
    const b = await req(port, "POST", "/token/host", { body: { room: "rL", identity: "host-B", secret: CFG.hostSecret } });
    assert.equal(b.status, 409);
    const again = await req(port, "POST", "/token/host", { body: { room: "rL", identity: "host-A", secret: CFG.hostSecret } });
    assert.equal(again.status, 200); // reconnect cùng identity OK
  } finally {
    srv.close();
  }
});

test("caption store: không token → 401; guest ghi → 403; host ghi → 200; GET trả lịch sử (C8)", async () => {
  const { srv, port } = await startServer();
  try {
    const noAuth = await req(port, "POST", "/captions/rC", { body: { track: "t", text: "hi" } });
    assert.equal(noAuth.status, 401);

    const guest = await req(port, "POST", "/token/guest", { body: { room: "rC", name: "G" } });
    const host = await req(port, "POST", "/token/host", { body: { room: "rC", identity: "host-1", secret: CFG.hostSecret } });

    const guestWrite = await req(port, "POST", "/captions/rC", { token: guest.json.token, body: { track: "t", text: "giả" } });
    assert.equal(guestWrite.status, 403); // guest KHÔNG ghi caption

    const hostWrite = await req(port, "POST", "/captions/rC", { token: host.json.token, body: { track: "spk1", name: "Chủ", kind: "final", text: "xin chào", t0: 1, t1: 2 } });
    assert.equal(hostWrite.status, 200);

    const hist = await req(port, "GET", "/captions/rC", { token: guest.json.token }); // guest ĐỌC được
    assert.equal(hist.status, 200);
    assert.equal(hist.json.captions.length, 1);
    assert.equal(hist.json.captions[0].text, "xin chào");
  } finally {
    srv.close();
  }
});

test("caption store: token sai room → 401 (không đọc trộm phòng khác)", async () => {
  const { srv, port } = await startServer();
  try {
    const g = await req(port, "POST", "/token/guest", { body: { room: "roomA", name: "G" } });
    const cross = await req(port, "GET", "/captions/roomB", { token: g.json.token });
    assert.equal(cross.status, 401);
  } finally {
    srv.close();
  }
});

test("room không hợp lệ → 400", async () => {
  const { srv, port } = await startServer();
  try {
    const r = await req(port, "POST", "/token/guest", { body: { room: "bad room!", name: "x" } });
    assert.equal(r.status, 400);
  } finally {
    srv.close();
  }
});

test("rate-limit → 429 khi vượt capacity", async () => {
  const { srv, port } = await startServer({ limiter: createRateLimiter({ capacity: 3, refillPerSec: 0 }) });
  try {
    const codes = [];
    for (let i = 0; i < 5; i++) {
      const r = await req(port, "POST", "/token/guest", { body: { room: "rr", name: "x" } });
      codes.push(r.status);
    }
    assert.ok(codes.filter((c) => c === 429).length >= 1, codes.join(","));
  } finally {
    srv.close();
  }
});

test("H1: XFF giả KHÔNG bypass rate-limit khi trustProxy=false (mặc định)", async () => {
  const { srv, port } = await startServer({ limiter: createRateLimiter({ capacity: 3, refillPerSec: 0 }) });
  try {
    const codes = [];
    for (let i = 0; i < 6; i++) {
      const r = await req(port, "POST", "/token/guest", { xff: `1.2.3.${i}`, body: { room: "rr", name: "x" } });
      codes.push(r.status);
    }
    // Keyed theo socket IP (không XFF) → XFF khác nhau vẫn chung 1 bucket → phải có 429.
    assert.ok(codes.filter((c) => c === 429).length >= 1, `XFF giả vẫn phải bị limit: ${codes}`);
  } finally {
    srv.close();
  }
});

test("M3: guest KHÔNG chiếm được identity namespace host-", async () => {
  const { srv, port } = await startServer();
  try {
    const r = await req(port, "POST", "/token/guest", { body: { room: "r1", name: "x", identity: "host-evil" } });
    assert.equal(r.status, 200);
    assert.ok(r.json.identity.startsWith("guest-"), r.json.identity); // server tự sinh, bỏ field client
  } finally {
    srv.close();
  }
});

test("verifyToken: token bị sửa → fail", async () => {
  const { srv, port } = await startServer();
  try {
    const g = await req(port, "POST", "/token/guest", { body: { room: "r1", name: "x" } });
    const tampered = g.json.token.slice(0, -3) + "AAA";
    assert.equal(verifyToken(tampered, CFG.apiSecret, "r1").ok, false);
  } finally {
    srv.close();
  }
});

(async () => {
  let failed = 0;
  for (const [name, fn] of TESTS) {
    try {
      await fn();
      console.log(`PASS ${name}`);
    } catch (e) {
      failed++;
      console.log(`FAIL ${name}: ${e.message}`);
    }
  }
  console.log(`\n${TESTS.length - failed}/${TESTS.length} passed`);
  process.exit(failed ? 1 : 0);
})();
