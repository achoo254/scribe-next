// TURN credential EPHEMERAL (RFC 5766 REST API / coturn use-auth-secret). KHÔNG credential tĩnh:
// lộ credential tĩnh → TURN bị lạm dụng làm open proxy (tốn băng thông + attribution risk).
// username = "<expiryUnix>:<name>", password = base64(HMAC-SHA1(turnSecret, username)). Hết hạn ngắn.
import crypto from "node:crypto";

/**
 * @param {string} turnSecret shared secret khớp coturn `static-auth-secret`
 * @param {string} name gợi nhớ (identity) — không bí mật
 * @param {string[]} urls danh sách turn: uris (vd ["turn:host:3478?transport=udp"])
 * @param {number} ttlSec hạn credential (khuyến nghị ~300s)
 */
export function makeTurnCredential(turnSecret, name, urls, ttlSec = 300) {
  const expiry = Math.floor(Date.now() / 1000) + ttlSec;
  const username = `${expiry}:${name}`;
  const credential = crypto.createHmac("sha1", turnSecret).update(username).digest("base64");
  return { username, credential, ttl: ttlSec, urls };
}
