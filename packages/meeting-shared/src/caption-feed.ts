// Render caption feed dùng chung (web viewer + desktop). Nhận caption qua LiveKit data channel +
// seed lịch sử (người vào trễ). [C7] chỉ nhận caption từ HOST (identity LiveKit ký + role trong metadata).
import { ref, shallowRef } from "vue";
import { RoomEvent, type Participant, type Room } from "livekit-client";
import type { CaptionLine, CaptionMsg, HostStatus, MeetingDataMsg } from "./types";

const MAX_LINES = 200;
const decoder = new TextDecoder();

export function useCaptionFeed(opts: { hostIdentity?: string } = {}) {
  const captions = shallowRef<CaptionLine[]>([]);
  const hostStatus = ref<HostStatus>("ok");

  function isHostSender(p?: Participant): boolean {
    if (!p) return false;
    if (opts.hostIdentity && p.identity === opts.hostIdentity) return true;
    try {
      const meta = p.metadata ? (JSON.parse(p.metadata) as { role?: string }) : null;
      return meta?.role === "host";
    } catch {
      return false;
    }
  }

  function addCaption(m: CaptionMsg): void {
    const id = `${m.track}:${m.t0.toFixed(3)}`;
    const line: CaptionLine = {
      id,
      track: m.track,
      name: m.name,
      text: m.text,
      final: m.kind === "final",
      t0: m.t0,
      t1: m.t1,
    };
    const list = captions.value.slice();
    const idx = list.findIndex((l) => l.id === id);
    if (idx >= 0) list[idx] = line;
    else list.push(line);
    if (list.length > MAX_LINES) list.splice(0, list.length - MAX_LINES);
    captions.value = list;
  }

  function onData(payload: Uint8Array, participant?: Participant): void {
    if (!isHostSender(participant)) return;
    try {
      const msg = JSON.parse(decoder.decode(payload)) as MeetingDataMsg;
      if (msg.type === "caption") addCaption(msg);
      else if (msg.type === "host_status") hostStatus.value = msg.status;
    } catch {
      /* không phải caption của mình */
    }
  }

  /** Nạp lịch sử fetch từ caption store khi vừa join (trước realtime). */
  function seedHistory(items: CaptionMsg[]): void {
    for (const m of items) addCaption(m);
  }

  function bind(room: Room): void {
    room.on(RoomEvent.DataReceived, onData);
  }
  function unbind(room: Room): void {
    room.off(RoomEvent.DataReceived, onData);
  }

  // Host render caption CỦA CHÍNH MÌNH: LiveKit không loopback publishData → thêm trực tiếp.
  function addLocal(m: CaptionMsg): void {
    addCaption(m);
  }
  function setHostStatus(s: HostStatus): void {
    hostStatus.value = s;
  }
  function clear(): void {
    captions.value = [];
    hostStatus.value = "ok";
  }

  return { captions, hostStatus, bind, unbind, seedHistory, addLocal, setHostStatus, clear };
}
