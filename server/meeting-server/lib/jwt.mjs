// JWT LiveKit (HS256) — sign + verify, zero-dep (chỉ node:crypto). Dùng chung: token-service cấp
// token, caption-store verify token (C8), dev-token script tái dùng sign.
import crypto from "node:crypto";

const b64url = (buf) => Buffer.from(buf).toString("base64url");
const b64urlJson = (obj) => b64url(JSON.stringify(obj));

/**
 * Ký JWT LiveKit.
 * @param {{apiKey:string, apiSecret:string, identity:string, name?:string, ttlSec?:number,
 *          room:string, role:"host"|"guest", canPublish?:boolean, canPublishData?:boolean}} o
 * @returns {string} JWT
 */
export function signToken(o) {
  const now = Math.floor(Date.now() / 1000);
  const grant = {
    room: o.room,
    roomJoin: true,
    canSubscribe: true,
    canPublish: o.canPublish !== false, // guest vẫn publish camera/mic (consumer nhưng có mặt)
    // [C7] chỉ HOST được publish data (caption). Guest = false → không bơm caption giả qua data channel.
    canPublishData: o.role === "host" && o.canPublishData !== false,
  };
  const payload = {
    iss: o.apiKey,
    sub: o.identity,
    name: o.name || o.identity,
    nbf: now - 10,
    exp: now + (o.ttlSec || 6 * 3600),
    // metadata do SERVER gán (role) → client KHÔNG tự khai; viewer đọc để lọc caption của host (C7).
    metadata: JSON.stringify({ role: o.role }),
    video: grant,
  };
  const data = `${b64urlJson({ alg: "HS256", typ: "JWT" })}.${b64urlJson(payload)}`;
  const sig = crypto.createHmac("sha256", o.apiSecret).update(data).digest("base64url");
  return `${data}.${sig}`;
}

/**
 * Verify chữ ký + hạn + (tùy chọn) room khớp. KHÔNG tin payload nếu chữ ký sai/hết hạn.
 * @returns {{ok:true, payload:object} | {ok:false, error:string}}
 */
export function verifyToken(token, apiSecret, expectRoom) {
  if (typeof token !== "string" || token.split(".").length !== 3) {
    return { ok: false, error: "token sai định dạng" };
  }
  const [h, p, sig] = token.split(".");
  const expect = crypto.createHmac("sha256", apiSecret).update(`${h}.${p}`).digest("base64url");
  // So sánh hằng thời gian — chống timing attack đoán chữ ký.
  const a = Buffer.from(sig);
  const b = Buffer.from(expect);
  if (a.length !== b.length || !crypto.timingSafeEqual(a, b)) {
    return { ok: false, error: "chữ ký không hợp lệ" };
  }
  let payload;
  try {
    payload = JSON.parse(Buffer.from(p, "base64url").toString("utf8"));
  } catch {
    return { ok: false, error: "payload không phải JSON" };
  }
  const now = Math.floor(Date.now() / 1000);
  if (payload.exp && now > payload.exp) return { ok: false, error: "token hết hạn" };
  if (payload.nbf && now < payload.nbf) return { ok: false, error: "token chưa hiệu lực" };
  if (expectRoom && payload.video?.room !== expectRoom) {
    return { ok: false, error: "room không khớp token" };
  }
  return { ok: true, payload };
}

/** Role đã ký trong metadata (server gán). Trả "host" | "guest" | null. */
export function roleOf(payload) {
  try {
    return JSON.parse(payload.metadata || "{}").role || null;
  } catch {
    return null;
  }
}
