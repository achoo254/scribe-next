#!/usr/bin/env node
// Sinh dev JWT cho LiveKit (Phase 2) — zero-dep (chỉ node:crypto). KHÔNG dùng production (Phase 5 = auth thật).
// Dùng:  LK_API_KEY=<key> LK_API_SECRET=<secret> node server/dev-token/generate-token.mjs <room> <identity> [name]
//   Giá trị dev khớp `keys:` trong infra/livekit/livekit.yaml (mặc định của `livekit-server --dev`).
import crypto from "node:crypto";

const apiId = process.env.LK_API_KEY;
const apiSig = process.env.LK_API_SECRET;
if (!apiId || !apiSig) {
  console.error(
    "Thiếu LK_API_KEY / LK_API_SECRET.\n" +
      "Dev: lấy cặp key:secret trong infra/livekit/livekit.yaml rồi:\n" +
      "  LK_API_KEY=<key> LK_API_SECRET=<secret> node server/dev-token/generate-token.mjs <room> <identity> [name]",
  );
  process.exit(1);
}

const [room = "demo", identity = `user-${Date.now()}`, name = identity] = process.argv.slice(2);
const b64 = (o) => Buffer.from(JSON.stringify(o)).toString("base64url");
const now = Math.floor(Date.now() / 1000);
const header = { alg: "HS256", typ: "JWT" };
const payload = {
  iss: apiId,
  sub: identity,
  name,
  nbf: now - 10,
  exp: now + 6 * 3600, // 6h cho phiên dev
  video: { room, roomJoin: true, canPublish: true, canSubscribe: true, canPublishData: true },
};
const data = `${b64(header)}.${b64(payload)}`;
const sig = crypto.createHmac("sha256", apiSig).update(data).digest("base64url");
process.stdout.write(`${data}.${sig}\n`);
