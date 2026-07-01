#!/usr/bin/env node
// Sinh dev JWT cho LiveKit (Phase 2/3 test tay) — tái dùng signToken của meeting-server (DRY, 1 nguồn ký).
// KHÔNG dùng production (Phase 5 = token-service cấp theo role + host-lock).
// Dùng:  LK_API_KEY=<key> LK_API_SECRET=<secret> node server/dev-token/generate-token.mjs <room> <identity> [name] [host|guest]
//   role mặc định "host" (dev cần publishData để test caption); "guest" = consumer (không publishData).
import { signToken } from "../meeting-server/lib/jwt.mjs";

const apiId = process.env.LK_API_KEY;
const apiSig = process.env.LK_API_SECRET;
if (!apiId || !apiSig) {
  console.error(
    "Thiếu LK_API_KEY / LK_API_SECRET.\n" +
      "Dev: lấy cặp key:secret trong infra/livekit/livekit.yaml rồi:\n" +
      "  LK_API_KEY=<key> LK_API_SECRET=<secret> node server/dev-token/generate-token.mjs <room> <identity> [name] [host|guest]",
  );
  process.exit(1);
}

const [room = "demo", identity = `user-${Date.now()}`, name, roleArg] = process.argv.slice(2);
const role = roleArg === "guest" ? "guest" : "host";
process.stdout.write(
  signToken({
    apiKey: apiId,
    apiSecret: apiSig,
    identity,
    name: name || identity,
    room,
    role,
    ttlSec: 6 * 3600,
  }) + "\n",
);
