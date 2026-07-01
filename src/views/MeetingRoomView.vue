<script setup lang="ts">
// Tab "Họp": nhập phòng + dev token → kết nối LiveKit → grid video + điều khiển + phụ đề trực tiếp.
// Phase 2 dùng dev token (LiveKit --dev), dán tay; auth theo role để Phase 5.
// Phase 3: bật "host transcribe" → tap audio mọi track → sidecar → caption realtime + publish.
import { ref } from "vue";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import { ViInput, ViButton, ViAlert, ViIcon, ViSwitch } from "../design-system/vi-ui";
import { useLiveKitRoom } from "../composables/use-livekit-room";
import { useAudioTapProbe } from "../composables/use-audio-tap-probe";
import { useLiveCaption } from "../composables/use-live-caption";
import {
  fetchHostToken,
  fetchGuestToken,
  pushCaption,
  type CaptionLine,
  type CaptionMsg,
} from "@onescribe/meeting-shared";
import ParticipantTile from "../components/meeting/ParticipantTile.vue";
import MeetingControls from "../components/meeting/MeetingControls.vue";
import CaptionPanel from "../components/meeting/CaptionPanel.vue";
import CaptionOverlay from "../components/meeting/CaptionOverlay.vue";

// 2 chế độ vào phòng:
//  - Token-service (Phase 5): điền "Máy chủ token" + phòng + (host secret nếu là host) → tự lấy token
//    theo role, host đẩy caption lên store (người vào trễ đọc lịch sử).
//  - Dev token: bỏ trống máy chủ → dán tay token (LiveKit --dev), như Phase 2.
const server = ref(""); // meeting-server base, vd http://localhost:8787 (rỗng = chế độ dev token)
const room = ref("demo");
const name = ref("");
const hostSecret = ref("");
const url = ref("ws://localhost:7880"); // LiveKit ws (chỉ dùng ở chế độ dev token)
const token = ref("");
const asHost = ref(false); // chọn client này là host (transcribe).
const lk = useLiveKitRoom();
const caption = useLiveCaption();

// G6 mini-spike: đo chân audio WebView2→Rust (độc lập LiveKit). Bấm "Đo" để lấy số round-trip/throughput.
const probe = useAudioTapProbe();

async function join() {
  lk.error.value = "";
  let lkUrl = url.value.trim();
  let lkToken = token.value.trim();
  let storeToken = "";

  if (server.value.trim()) {
    // Token-service: lấy token theo role từ server (role gán server-side — C6).
    if (!room.value.trim() || !name.value.trim()) {
      lk.error.value = "Chế độ token-service cần Phòng + Tên.";
      return;
    }
    try {
      const t = asHost.value
        ? await fetchHostToken(server.value.trim(), room.value.trim(), name.value.trim(), hostSecret.value)
        : await fetchGuestToken(server.value.trim(), room.value.trim(), name.value.trim());
      lkUrl = t.url;
      lkToken = t.token;
      storeToken = t.token;
    } catch (e) {
      lk.error.value = e instanceof Error ? e.message : String(e);
      return;
    }
  } else if (!lkToken) {
    lk.error.value = "Cần dev token (sinh bằng script server/dev-token) hoặc điền Máy chủ token.";
    return;
  }

  await lk.connect(lkUrl, lkToken);
  if (lk.state.value === "connected" && lk.room.value) {
    // Host + token-service → đẩy caption final lên store cho người vào trễ.
    const hooks =
      asHost.value && server.value.trim() && storeToken
        ? { onFinal: (c: CaptionMsg) => pushCaption(server.value.trim(), room.value.trim(), storeToken, c) }
        : {};
    await caption.start(lk.room.value, asHost.value, hooks);
  }
}

async function leave() {
  await caption.stop();
  await lk.disconnect();
}

// Strategy A (caption-as-record): ghép caption final theo thời gian → .txt phía client (tùy chọn).
function fmtClock(t: number): string {
  const s = Math.max(0, Math.floor(t));
  return `${String(Math.floor(s / 60)).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}`;
}

async function saveTranscript() {
  const lines = caption.captions.value
    .filter((c: CaptionLine) => c.final)
    .map((c: CaptionLine) => `[${fmtClock(c.t0)}] ${c.name}: ${c.text}`)
    .join("\n");
  if (!lines) return;
  const path = await save({
    defaultPath: `bien-ban-hop-${Date.now()}.txt`,
    filters: [{ name: "Text", extensions: ["txt"] }],
  });
  if (path) await writeTextFile(path, lines);
}
</script>

<template>
  <div class="meeting">
    <!-- Form vào phòng (trước khi kết nối) -->
    <div v-if="lk.state.value !== 'connected'" class="meeting__join">
      <ViInput v-model="server" label="Máy chủ token (để trống = dev token)" density="compact" placeholder="http://localhost:8787" :disabled="lk.state.value === 'connecting'" />
      <ViInput v-model="room" label="Phòng" density="compact" :disabled="lk.state.value === 'connecting'" />
      <ViInput v-model="name" label="Tên hiển thị" density="compact" :disabled="lk.state.value === 'connecting'" />
      <ViSwitch v-model="asHost" label="Tôi là host (transcribe phụ đề)" :disabled="lk.state.value === 'connecting'" />
      <ViInput v-if="asHost && server.trim()" v-model="hostSecret" label="Host secret" density="compact" :disabled="lk.state.value === 'connecting'" />

      <template v-if="!server.trim()">
        <ViInput v-model="url" label="LiveKit URL (dev)" density="compact" :disabled="lk.state.value === 'connecting'" />
        <ViInput v-model="token" label="Dev token" density="compact" :disabled="lk.state.value === 'connecting'" />
      </template>

      <ViButton color="primary" :loading="lk.state.value === 'connecting'" @click="join">
        <ViIcon icon="IconVideoPlus" start />Vào phòng
      </ViButton>
      <ViAlert v-if="lk.error.value" type="error" density="compact">{{ lk.error.value }}</ViAlert>

      <!-- G6 mini-spike: đo chân audio WebView2 → Rust (không cần LiveKit). -->
      <div class="probe">
        <div class="probe__head">
          <span class="probe__title">G6 — Đo chân audio (WebView2 → Rust)</span>
          <ViButton
            size="small"
            :color="probe.running.value ? 'error' : 'secondary'"
            variant="tonal"
            @click="probe.running.value ? probe.stop() : probe.start()"
          >
            {{ probe.running.value ? "Dừng" : "Đo" }}
          </ViButton>
        </div>
        <div class="probe__grid">
          <span>Sample rate thực: <b>{{ probe.actualSampleRate.value || "—" }} Hz</b></span>
          <span>SharedArrayBuffer: <b>{{ probe.sabSupported.value ? "có" : "không" }}</b></span>
          <span>crossOriginIsolated: <b>{{ probe.crossOriginIsolated.value ? "có" : "không" }}</b></span>
          <span>Chunk gửi: <b>{{ probe.chunksSent.value }}</b></span>
          <span>Tổng byte: <b>{{ (probe.bytesSent.value / 1024).toFixed(0) }} KB</b></span>
          <span>Round-trip last/p50/p95: <b>{{ probe.rttLast.value }}/{{ probe.rttP50.value }}/{{ probe.rttP95.value }} ms</b></span>
        </div>
        <ViAlert v-if="probe.error.value" type="error" density="compact">{{ probe.error.value }}</ViAlert>
      </div>
    </div>

    <!-- Trong phòng: grid + phụ đề + điều khiển -->
    <template v-else>
      <div class="meeting__stage">
        <div class="meeting__grid-wrap">
          <div class="meeting__grid">
            <ParticipantTile v-for="p in lk.participants.value" :key="p.identity" :p="p" />
          </div>
          <CaptionOverlay :captions="caption.captions.value" />
        </div>
        <aside class="meeting__captions">
          <CaptionPanel
            :captions="caption.captions.value"
            :host-status="caption.hostStatus.value"
            @save="saveTranscript"
          />
        </aside>
      </div>
      <MeetingControls
        :mic-on="lk.micOn.value"
        :cam-on="lk.camOn.value"
        @toggle-mic="lk.toggleMic"
        @toggle-camera="lk.toggleCamera"
        @leave="leave"
      />
      <ViAlert v-if="lk.error.value" type="warning" density="compact">{{ lk.error.value }}</ViAlert>
    </template>
  </div>
</template>

<style scoped>
.meeting {
  display: flex;
  flex-direction: column;
  gap: var(--viui-space-md);
  height: 100%;
  min-height: 0;
}
.meeting__join {
  display: flex;
  flex-direction: column;
  gap: var(--viui-space-sm);
  max-width: 420px;
}
/* Sân khấu: grid video (co giãn) + cột phụ đề cố định; hẹp thì xuống dòng. */
.meeting__stage {
  display: flex;
  gap: var(--viui-space-md);
  flex: 1;
  min-height: 0;
}
.meeting__grid-wrap {
  position: relative;
  flex: 1;
  min-width: 0;
}
.meeting__captions {
  width: 320px;
  flex-shrink: 0;
  border-left: 1px solid rgba(var(--v-theme-on-surface), 0.12);
  padding-left: var(--viui-space-sm);
}
@media (max-width: 720px) {
  .meeting__stage {
    flex-direction: column;
  }
  .meeting__captions {
    width: auto;
    border-left: none;
    border-top: 1px solid rgba(var(--v-theme-on-surface), 0.12);
    padding-left: 0;
    padding-top: var(--viui-space-sm);
    max-height: 200px;
  }
}
/* Grid responsive ≤8 tile: auto-fit theo bề rộng, không cần Vuetify row/col (né H5 ViRow/ViCol). */
.meeting__grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  gap: var(--viui-space-sm);
}
.probe {
  display: flex;
  flex-direction: column;
  gap: var(--viui-space-xs);
  margin-top: var(--viui-space-md);
  padding: var(--viui-space-sm);
  border: 1px solid rgb(var(--v-theme-surface));
  border-radius: var(--viui-radius-md, 8px);
}
.probe__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--viui-space-sm);
}
.probe__title {
  font-size: var(--viui-font-size-sm);
  font-weight: 600;
}
.probe__grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: var(--viui-space-xs);
  font-size: var(--viui-font-size-sm);
  opacity: 0.85;
}
</style>
