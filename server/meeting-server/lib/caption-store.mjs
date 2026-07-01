// Caption store text-only, TTL 1 ngày [C8-adjacent] — in-memory. Người vào trễ fetch lịch sử trong hạn.
//
// Quyết định Redis-vs-in-memory (plan yêu cầu chốt trước): MVP local-first single-instance → IN-MEMORY.
// Đánh đổi: caption MẤT khi restart server (late-joiner-qua-restart KHÔNG đạt) — chấp nhận cho MVP.
// Interface store-agnostic (push/history) → drop-in Redis (RPUSH+EXPIRE+LRANGE) khi cần multi-instance
// / bền qua restart. KHÔNG lưu media (chỉ text KB-scale) — dung hòa "server không lưu media".
export function createCaptionStore({ ttlSec = 86400, maxPerRoom = 5000 } = {}) {
  const rooms = new Map(); // room → { items: caption[], expiresAt }

  function _live(room) {
    const r = rooms.get(room);
    if (!r) return null;
    if (Date.now() / 1000 > r.expiresAt) {
      rooms.delete(room);
      return null;
    }
    return r;
  }

  function push(room, caption) {
    let r = _live(room);
    if (!r) {
      r = { items: [], expiresAt: 0 };
      rooms.set(room, r);
    }
    r.items.push(caption);
    if (r.items.length > maxPerRoom) r.items.splice(0, r.items.length - maxPerRoom);
    r.expiresAt = Date.now() / 1000 + ttlSec; // gia hạn TTL theo hoạt động mới nhất
  }

  function history(room) {
    return _live(room)?.items ?? [];
  }

  // Purge phòng hết hạn (Redis EXPIRE lo sẵn; in-memory cần sweep chủ động).
  function sweep() {
    const now = Date.now() / 1000;
    for (const [k, r] of rooms) if (now > r.expiresAt) rooms.delete(k);
  }

  return { push, history, sweep, _size: () => rooms.size };
}
