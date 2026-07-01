// Rate-limit token-bucket per key (IP) — in-memory, zero-dep. Chặn spam endpoint cấp token.
export function createRateLimiter({ capacity = 20, refillPerSec = 1 } = {}) {
  const buckets = new Map(); // key → { tokens, last }

  function allow(key) {
    const now = Date.now() / 1000;
    let b = buckets.get(key);
    if (!b) {
      b = { tokens: capacity, last: now };
      buckets.set(key, b);
    }
    b.tokens = Math.min(capacity, b.tokens + (now - b.last) * refillPerSec);
    b.last = now;
    if (b.tokens < 1) return false;
    b.tokens -= 1;
    return true;
  }

  // Dọn bucket cũ (đầy lại + lâu không dùng) → không phình bộ nhớ theo số IP.
  function sweep() {
    const now = Date.now() / 1000;
    for (const [k, b] of buckets) {
      if (b.tokens >= capacity && now - b.last > 300) buckets.delete(k);
    }
  }

  return { allow, sweep, _size: () => buckets.size };
}
