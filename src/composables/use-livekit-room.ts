// Composable Phase 2: phòng họp LiveKit (SFU self-host). Kết nối, publish camera/mic,
// subscribe track người khác, theo dõi participant + active-speaker. UI chỉ render; logic ở đây.
// Auth thật (token theo role) để Phase 5 — Phase 2 dùng dev token (LiveKit --dev).
import { ref, shallowRef, onUnmounted, onDeactivated } from "vue";
import {
  Room,
  RoomEvent,
  Track,
  type Participant,
} from "livekit-client";

export interface ParticipantVM {
  identity: string;
  name: string;
  isLocal: boolean;
  videoTrack: Track | null; // camera track → attach <video>
  audioTrack: Track | null; // mic track → attach <audio> (chỉ remote mới phát, tránh echo)
  micOn: boolean;
  camOn: boolean;
  speaking: boolean;
}

export type RoomState = "disconnected" | "connecting" | "connected";

export function useLiveKitRoom() {
  const room = shallowRef<Room | null>(null);
  const state = ref<RoomState>("disconnected");
  // shallowRef: luôn gán lại CẢ mảng khi rebuild → không cần deep reactivity; tránh Vue UnwrapRef
  // bóc private field của class Track (vỡ type) + tránh proxy MediaStreamTrack (đúng + nhẹ).
  const participants = shallowRef<ParticipantVM[]>([]);
  const error = ref("");
  const micOn = ref(false);
  const camOn = ref(false);

  function vmFor(p: Participant, isLocal: boolean): ParticipantVM {
    let videoTrack: Track | null = null;
    let audioTrack: Track | null = null;
    p.trackPublications.forEach((pub) => {
      if (!pub.track) return;
      if (pub.source === Track.Source.Camera) videoTrack = pub.track;
      else if (pub.source === Track.Source.Microphone) audioTrack = pub.track;
    });
    return {
      identity: p.identity,
      name: p.name || p.identity,
      isLocal,
      videoTrack,
      audioTrack,
      micOn: p.isMicrophoneEnabled,
      camOn: p.isCameraEnabled,
      speaking: p.isSpeaking,
    };
  }

  // Dựng lại danh sách VM từ state phòng (KISS cho ≤8 người: rebuild trên mỗi event track/participant).
  function rebuild() {
    const r = room.value;
    if (!r) {
      participants.value = [];
      return;
    }
    const list: ParticipantVM[] = [vmFor(r.localParticipant, true)];
    r.remoteParticipants.forEach((p) => list.push(vmFor(p, false)));
    participants.value = list;
    micOn.value = r.localParticipant.isMicrophoneEnabled;
    camOn.value = r.localParticipant.isCameraEnabled;
  }

  async function connect(url: string, token: string) {
    if (state.value !== "disconnected") return;
    error.value = "";
    state.value = "connecting";
    // adaptiveStream + dynacast: tự hạ chất lượng/độ phân giải theo băng thông + người xem (simulcast).
    const r = new Room({ adaptiveStream: true, dynacast: true });
    room.value = r;
    r.on(RoomEvent.ParticipantConnected, rebuild)
      .on(RoomEvent.ParticipantDisconnected, rebuild)
      .on(RoomEvent.TrackSubscribed, rebuild)
      .on(RoomEvent.TrackUnsubscribed, rebuild)
      .on(RoomEvent.LocalTrackPublished, rebuild)
      .on(RoomEvent.LocalTrackUnpublished, rebuild)
      .on(RoomEvent.TrackMuted, rebuild)
      .on(RoomEvent.TrackUnmuted, rebuild)
      .on(RoomEvent.ActiveSpeakersChanged, (speakers: Participant[]) => {
        const ids = new Set(speakers.map((s) => s.identity));
        participants.value = participants.value.map((vm) => ({
          ...vm,
          speaking: ids.has(vm.identity),
        }));
      })
      .on(RoomEvent.Disconnected, () => {
        state.value = "disconnected";
        participants.value = [];
      });
    try {
      await r.connect(url, token);
      // Publish camera + mic. getUserMedia ở bước này cần quyền media WebView2 (xem hook Rust H2).
      await r.localParticipant.enableCameraAndMicrophone();
      state.value = "connected";
      rebuild();
    } catch (e) {
      error.value = String(e);
      state.value = "disconnected";
      await r.disconnect().catch(() => {});
      room.value = null;
    }
  }

  async function toggleMic() {
    const r = room.value;
    if (!r) return;
    await r.localParticipant.setMicrophoneEnabled(!r.localParticipant.isMicrophoneEnabled);
    rebuild();
  }

  async function toggleCamera() {
    const r = room.value;
    if (!r) return;
    await r.localParticipant.setCameraEnabled(!r.localParticipant.isCameraEnabled);
    rebuild();
  }

  async function disconnect() {
    const r = room.value;
    if (!r) return;
    await r.disconnect().catch(() => {});
    room.value = null;
    state.value = "disconnected";
    participants.value = [];
  }

  // Rời phòng sạch khi rời view. App.vue bọc <keep-alive> → đổi tab chỉ DEACTIVATE (không unmount)
  // → CHỈ onUnmounted sẽ KHÔNG fire khi chuyển tab → camera/mic kẹt sáng. Phải bắt cả onDeactivated.
  onUnmounted(disconnect);
  onDeactivated(disconnect);

  return { room, state, participants, error, micOn, camOn, connect, disconnect, toggleMic, toggleCamera };
}
