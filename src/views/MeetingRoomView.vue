<script setup lang="ts">
// Tab "Họp" (Phase 2): nhập phòng + dev token → kết nối LiveKit → grid video + điều khiển.
// Phase 2 dùng dev token (LiveKit --dev), dán tay; auth theo role để Phase 5.
import { ref } from "vue";
import { ViInput, ViButton, ViAlert, ViIcon } from "../design-system/vi-ui";
import { useLiveKitRoom } from "../composables/use-livekit-room";
import { useAudioTapProbe } from "../composables/use-audio-tap-probe";
import ParticipantTile from "../components/meeting/ParticipantTile.vue";
import MeetingControls from "../components/meeting/MeetingControls.vue";

// LiveKit dev chạy native trên WSL Ubuntu (port 7880, forward sang localhost Windows).
const url = ref("ws://localhost:7880");
const token = ref("");
const lk = useLiveKitRoom();

// G6 mini-spike: đo chân audio WebView2→Rust (độc lập LiveKit). Bấm "Đo" để lấy số round-trip/throughput.
const probe = useAudioTapProbe();

async function join() {
  if (!token.value.trim()) {
    lk.error.value = "Cần dev token (sinh bằng script server/dev-token).";
    return;
  }
  await lk.connect(url.value.trim(), token.value.trim());
}
</script>

<template>
  <div class="meeting">
    <!-- Form vào phòng (trước khi kết nối) -->
    <div v-if="lk.state.value !== 'connected'" class="meeting__join">
      <ViInput v-model="url" label="LiveKit URL" density="compact" :disabled="lk.state.value === 'connecting'" />
      <ViInput
        v-model="token"
        label="Dev token"
        density="compact"
        :disabled="lk.state.value === 'connecting'"
      />
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

    <!-- Trong phòng: grid + điều khiển -->
    <template v-else>
      <div class="meeting__grid">
        <ParticipantTile v-for="p in lk.participants.value" :key="p.identity" :p="p" />
      </div>
      <MeetingControls
        :mic-on="lk.micOn.value"
        :cam-on="lk.camOn.value"
        @toggle-mic="lk.toggleMic"
        @toggle-camera="lk.toggleCamera"
        @leave="lk.disconnect"
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
}
.meeting__join {
  display: flex;
  flex-direction: column;
  gap: var(--viui-space-sm);
  max-width: 420px;
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
