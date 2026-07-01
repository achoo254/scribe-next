// Client gọi meeting-server (Phase 5): lấy token theo role + fetch lịch sử caption. Dùng chung web/desktop.
import type { CaptionMsg, TokenResult } from "./types";

async function postJson(url: string, body: unknown): Promise<Response> {
  return fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
}

async function asError(r: Response): Promise<never> {
  const j = (await r.json().catch(() => null)) as { error?: string } | null;
  throw new Error(j?.error || `HTTP ${r.status}`);
}

/** Token guest (consumer). Server hard-code role=guest — client KHÔNG xin được host qua đây (C6). */
export async function fetchGuestToken(baseUrl: string, room: string, name: string): Promise<TokenResult> {
  const r = await postJson(`${baseUrl}/token/guest`, { room, name });
  if (!r.ok) return asError(r);
  return (await r.json()) as TokenResult;
}

/** Token host — cần host secret (pre-shared). 409 nếu phòng đã có host khác. */
export async function fetchHostToken(
  baseUrl: string,
  room: string,
  name: string,
  secret: string,
  identity?: string,
): Promise<TokenResult> {
  const r = await postJson(`${baseUrl}/token/host`, { room, name, secret, identity });
  if (!r.ok) return asError(r);
  return (await r.json()) as TokenResult;
}

/** Host đẩy caption final lên store để người vào trễ đọc lịch sử. Cần host-token (role=host — C8). */
export async function pushCaption(
  baseUrl: string,
  room: string,
  token: string,
  caption: CaptionMsg,
): Promise<void> {
  // best-effort: mất 1 caption store KHÔNG được làm hỏng realtime (data channel vẫn chạy). Nhưng LOG
  // để chẩn đoán store hỏng dai (token sai/403 → late-joiner không có lịch sử mà không ai biết).
  try {
    const r = await fetch(`${baseUrl}/captions/${encodeURIComponent(room)}`, {
      method: "POST",
      headers: { "Content-Type": "application/json", Authorization: `Bearer ${token}` },
      body: JSON.stringify(caption),
    });
    if (!r.ok) console.warn(`[meeting] pushCaption store trả ${r.status}`);
  } catch (e) {
    console.warn("[meeting] pushCaption lỗi mạng:", e);
  }
}

/** Lịch sử caption trong TTL (người vào trễ đọc lại). Cần JWT hợp lệ + room khớp (C8). */
export async function fetchCaptionHistory(
  baseUrl: string,
  room: string,
  token: string,
): Promise<CaptionMsg[]> {
  const r = await fetch(`${baseUrl}/captions/${encodeURIComponent(room)}`, {
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!r.ok) return [];
  const j = (await r.json()) as { captions?: CaptionMsg[] };
  return j.captions ?? [];
}
