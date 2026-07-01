// Khóa 1-host/phòng [C6] — in-memory với TTL. Request host thứ 2 khi đã có host còn sống → 409.
// Nhả khóa: LiveKit webhook `participant_left` của host, hoặc TTL hết hạn (host chết không nhả).
//
// MVP single-instance. Multi-instance → thay bằng Redis `SET room:{id}:host NX EX` (cùng interface).
export function createHostLock({ ttlSec = 4 * 3600 } = {}) {
  const locks = new Map(); // room → { identity, expiresAt }

  function _live(room) {
    const l = locks.get(room);
    if (!l) return null;
    if (Date.now() / 1000 > l.expiresAt) {
      locks.delete(room);
      return null;
    }
    return l;
  }

  /** Chiếm khóa. Trả true nếu phòng chưa có host (hoặc chính identity này giữ — idempotent/reconnect). */
  function acquire(room, identity) {
    const l = _live(room);
    if (l && l.identity !== identity) return false;
    locks.set(room, { identity, expiresAt: Date.now() / 1000 + ttlSec });
    return true;
  }

  /** Nhả khóa nếu đúng identity đang giữ (tránh guest/kẻ khác nhả hộ). */
  function release(room, identity) {
    const l = locks.get(room);
    if (l && l.identity === identity) locks.delete(room);
  }

  function currentHost(room) {
    return _live(room)?.identity ?? null;
  }

  return { acquire, release, currentHost, _size: () => locks.size };
}
