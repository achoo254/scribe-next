// Composable phòng LiveKit cho VIEWER (web guest) — connect + publish camera/mic + theo dõi track.
// KHÔNG host/transcribe (đó là việc desktop). Nhận TURN ephemeral (Phase 5) qua rtcConfig.
// (Trùng nhẹ với desktop use-livekit-room theo chủ ý "sibling"; hợp nhất về đây khi lên monorepo đầy đủ.)
import { ref, shallowRef, onUnmounted } from "vue";
import { Room, RoomEvent, Track, type Participant } from "livekit-client";
import type { TurnCredential } from "./types";

export interface ParticipantVM {
  identity: string;
  name: string;
  isLocal: boolean;
  videoTrack: Track | null;
  audioTrack: Track | null;
  micOn: boolean;
  camOn: boolean;
  speaking: boolean;
}

export type RoomState = "disconnected" | "connecting" | "connected";

export function useLiveKitViewer() {
  const room = shallowRef<Room | null>(null);
  const state = ref<RoomState>("disconnected");
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

  function rebuild(): void {
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

  async function connect(url: string, token: string, turn?: TurnCredential | null): Promise<Room | null> {
    if (state.value !== "disconnected") return room.value;
    error.value = "";
    state.value = "connecting";
    const rtcConfig: RTCConfiguration | undefined = turn
      ? { iceServers: [{ urls: turn.urls, username: turn.username, credential: turn.credential }] }
      : undefined;
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
        participants.value = participants.value.map((vm) => ({ ...vm, speaking: ids.has(vm.identity) }));
      })
      .on(RoomEvent.Disconnected, () => {
        state.value = "disconnected";
        participants.value = [];
      });
    try {
      // TURN (ephemeral, Phase 5) qua rtcConfig của RoomConnectOptions — cho participant sau NAT chặt.
      await r.connect(url, token, rtcConfig ? { rtcConfig } : undefined);
      // Publish camera + mic cần user gesture (Safari/iOS) — gọi từ nút "Tham gia".
      await r.localParticipant.enableCameraAndMicrophone();
      state.value = "connected";
      rebuild();
      return r;
    } catch (e) {
      error.value = String(e);
      state.value = "disconnected";
      await r.disconnect().catch(() => {});
      room.value = null;
      return null;
    }
  }

  async function toggleMic(): Promise<void> {
    const r = room.value;
    if (!r) return;
    await r.localParticipant.setMicrophoneEnabled(!r.localParticipant.isMicrophoneEnabled);
    rebuild();
  }

  async function toggleCamera(): Promise<void> {
    const r = room.value;
    if (!r) return;
    await r.localParticipant.setCameraEnabled(!r.localParticipant.isCameraEnabled);
    rebuild();
  }

  async function disconnect(): Promise<void> {
    const r = room.value;
    if (!r) return;
    await r.disconnect().catch(() => {});
    room.value = null;
    state.value = "disconnected";
    participants.value = [];
  }

  onUnmounted(disconnect);

  return { room, state, participants, error, micOn, camOn, connect, disconnect, toggleMic, toggleCamera };
}
