// Host live-transcribe (Phase 3, DESKTOP-only — cần Tauri + sidecar). Tap audio mọi track qua
// AudioWorklet → invoke Rust → sidecar streaming; nhận caption sidecar phát → publish qua LiveKit
// data channel (chỉ host) + render tại chỗ. Viewer (web) KHÔNG dùng file này — chỉ dùng useCaptionFeed (shared).
import { onDeactivated, onUnmounted } from "vue";
import { RoomEvent, Track } from "livekit-client";
import type {
  LocalTrackPublication,
  Participant,
  RemoteTrack,
  Room,
  TrackPublication,
} from "livekit-client";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useCaptionFeed, type CaptionMsg, type HostStatus } from "@onescribe/meeting-shared";

/** Hook host: đẩy caption final lên caption store (Phase 5) để người vào trễ đọc lịch sử. */
export interface LiveCaptionHooks {
  onFinal?: (c: CaptionMsg) => void;
}

const CHUNK_MS = 500;
const encoder = new TextEncoder();

/** Track id gửi xuống sidecar chỉ `[A-Za-z0-9_-]` (Rust cũng chặn) — map lại tên người nói khi có caption. */
function safeTrackId(identity: string): string {
  return identity.replace(/[^A-Za-z0-9_-]/g, "_").slice(0, 64) || "track";
}

interface Tap {
  ctx: AudioContext;
  node: AudioWorkletNode;
  src: MediaStreamAudioSourceNode;
}

interface SidecarCaption {
  type: string;
  track?: string;
  kind?: "partial" | "final";
  text?: string;
  t0?: number;
  t1?: number;
}

export function useLiveCaption(opts: { hostIdentity?: string } = {}) {
  const feed = useCaptionFeed({ hostIdentity: opts.hostIdentity });
  const taps = new Map<string, Tap>();
  const trackName = new Map<string, string>(); // trackId → tên người nói (map ngược khi caption về)
  let unlistenCaption: UnlistenFn | null = null;
  let boundRoom: Room | null = null;
  let micTrack: MediaStreamTrack | null = null;
  let isHost = false;
  let hooks: LiveCaptionHooks = {};
  let recovering = false;
  let crashRetries = 0; // reset khi có caption trở lại; bỏ cuộc sau 3 lần crash liên tiếp

  async function addTap(trackId: string, mst: MediaStreamTrack, name: string): Promise<void> {
    if (taps.has(trackId)) return;
    trackName.set(trackId, name);
    try {
      await invoke("meeting_transcribe_start", { track: trackId });
      const ctx = new AudioContext({ sampleRate: 16000 });
      await ctx.audioWorklet.addModule("/meeting-audio-tap-worklet.js");
      const src = ctx.createMediaStreamSource(new MediaStream([mst]));
      const node = new AudioWorkletNode(ctx, "meeting-audio-tap", {
        processorOptions: { chunkMs: CHUNK_MS },
      });
      node.port.onmessage = (ev: MessageEvent<ArrayBuffer>) => {
        // Uint8Array → Vec<u8> raw (Tauri v2 truyền binary, KHÔNG JSON-array — đã chứng minh ở G6).
        void invoke("meeting_transcribe_pcm", {
          track: trackId,
          pcm: new Uint8Array(ev.data),
        }).catch(() => {});
      };
      src.connect(node);
      // KHÔNG connect node → destination: chỉ tap, không phát lại (tránh vọng/echo).
      taps.set(trackId, { ctx, node, src });
    } catch (e) {
      console.error("[caption] addTap lỗi", trackId, e);
      trackName.delete(trackId);
    }
  }

  async function removeTap(trackId: string): Promise<void> {
    const t = taps.get(trackId);
    if (t) {
      t.node.port.onmessage = null;
      try {
        t.src.disconnect();
        t.node.disconnect();
        await t.ctx.close();
      } catch {
        /* ctx có thể đã đóng */
      }
      taps.delete(trackId);
    }
    trackName.delete(trackId);
    await invoke("meeting_transcribe_end", { track: trackId }).catch(() => {});
  }

  function publishStatus(room: Room, status: HostStatus): void {
    feed.setHostStatus(status);
    const data = encoder.encode(JSON.stringify({ type: "host_status", status }));
    void room.localParticipant.publishData(data, { reliable: true }).catch(() => {});
  }

  function attachAudioPublication(pub: TrackPublication, participant: Participant): void {
    if (pub.kind !== Track.Kind.Audio || !pub.track) return;
    const mst = pub.track.mediaStreamTrack;
    if (mst) void addTap(safeTrackId(participant.identity), mst, participant.name || participant.identity);
  }

  function attachLocalMic(room: Room): void {
    const pub = room.localParticipant.getTrackPublication(Track.Source.Microphone);
    const mst = pub?.track?.mediaStreamTrack;
    if (!mst) return;
    micTrack = mst;
    // [Mic revocation] Windows/IT policy thu hồi quyền giữa chừng → track mute im lặng, VAD thấy lặng
    // → caption ngừng không báo. Báo host_mic_lost để mọi người thấy "transcribe tạm dừng".
    mst.onmute = () => boundRoom && publishStatus(boundRoom, "mic_lost");
    mst.onunmute = () => boundRoom && publishStatus(boundRoom, "ok");
    void addTap(safeTrackId(room.localParticipant.identity), mst, room.localParticipant.name || "Bạn");
  }

  function onTrackSubscribed(track: RemoteTrack, _pub: TrackPublication, participant: Participant): void {
    if (track.kind === Track.Kind.Audio && track.mediaStreamTrack) {
      void addTap(safeTrackId(participant.identity), track.mediaStreamTrack, participant.name || participant.identity);
    }
  }

  function onTrackUnsubscribed(track: RemoteTrack, _pub: TrackPublication, participant: Participant): void {
    if (track.kind === Track.Kind.Audio) void removeTap(safeTrackId(participant.identity));
  }

  function onLocalPublished(pub: LocalTrackPublication): void {
    // Mic publish lại sau khi start (host bật lại mic) → gắn tap lại.
    if (pub.source === Track.Source.Microphone && boundRoom) attachLocalMic(boundRoom);
  }

  function onSidecarMessage(m: SidecarCaption, room: Room): void {
    if (m.type === "caption" && m.track && m.text) {
      crashRetries = 0; // caption trở lại → recovery thành công, reset đếm
      const name = trackName.get(m.track) ?? m.track;
      const msg: CaptionMsg = {
        type: "caption",
        track: m.track,
        name,
        kind: m.kind === "partial" ? "partial" : "final",
        text: m.text,
        t0: m.t0 ?? 0,
        t1: m.t1 ?? 0,
      };
      feed.addLocal(msg); // host render caption của chính mình (LiveKit không loopback)
      const data = encoder.encode(JSON.stringify(msg));
      void room.localParticipant.publishData(data, { reliable: msg.kind === "final" }).catch(() => {});
      if (msg.kind === "final") hooks.onFinal?.(msg); // lưu vào caption store (late-joiner history)
    } else if (m.type === "sidecar_crashed") {
      publishStatus(room, "transcribe_down");
      void recoverAfterCrash(room); // [H3] tự khởi động lại: re-start các track đang tap
    } else if (m.type === "backpressure") {
      feed.setHostStatus("overloaded");
    }
  }

  /** [H3] Sidecar chết giữa họp → respawn (qua meeting_transcribe_start) + tạo lại session cho track
   * đang tap. Worklet vẫn chạy nên PCM tiếp tục chảy vào sidecar mới. Bỏ cuộc sau 3 lần liên tiếp. */
  async function recoverAfterCrash(room: Room): Promise<void> {
    if (recovering || !isHost) return;
    if (crashRetries >= 3) return; // giữ trạng thái transcribe_down, không lặp vô hạn
    recovering = true;
    crashRetries += 1;
    try {
      for (const trackId of taps.keys()) {
        await invoke("meeting_transcribe_start", { track: trackId }).catch(() => {});
      }
      publishStatus(room, "ok");
    } finally {
      recovering = false;
    }
  }

  /** Bắt đầu: host = tap + publish; viewer = chỉ subscribe render. Gọi sau khi room connected. */
  async function start(room: Room, host: boolean, startHooks: LiveCaptionHooks = {}): Promise<void> {
    isHost = host;
    hooks = startHooks;
    boundRoom = room;
    feed.bind(room); // viewer render (host cũng bind để đồng nhất; caption của host thêm qua addLocal)
    if (!isHost) return;

    unlistenCaption = await listen<SidecarCaption>("meeting_caption_msg", (ev) =>
      onSidecarMessage(ev.payload, room),
    );
    room.remoteParticipants.forEach((p) =>
      p.trackPublications.forEach((pub) => attachAudioPublication(pub, p)),
    );
    room.on(RoomEvent.TrackSubscribed, onTrackSubscribed);
    room.on(RoomEvent.TrackUnsubscribed, onTrackUnsubscribed);
    room.on(RoomEvent.LocalTrackPublished, onLocalPublished);
    attachLocalMic(room);
  }

  async function stop(): Promise<void> {
    if (boundRoom) {
      feed.unbind(boundRoom);
      boundRoom.off(RoomEvent.TrackSubscribed, onTrackSubscribed);
      boundRoom.off(RoomEvent.TrackUnsubscribed, onTrackUnsubscribed);
      boundRoom.off(RoomEvent.LocalTrackPublished, onLocalPublished);
    }
    if (micTrack) {
      micTrack.onmute = null;
      micTrack.onunmute = null;
      micTrack = null;
    }
    if (unlistenCaption) {
      unlistenCaption();
      unlistenCaption = null;
    }
    await Promise.all([...taps.keys()].map((id) => removeTap(id)));
    if (isHost) await invoke("meeting_transcribe_stop").catch(() => {});
    isHost = false;
    hooks = {};
    recovering = false;
    crashRetries = 0;
    boundRoom = null;
  }

  onUnmounted(() => void stop());
  onDeactivated(() => void stop());

  return { captions: feed.captions, hostStatus: feed.hostStatus, start, stop };
}
