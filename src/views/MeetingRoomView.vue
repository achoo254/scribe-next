<script setup lang="ts">
// Tab "Họp" (Phase 2): nhập phòng + dev token → kết nối LiveKit → grid video + điều khiển.
// Phase 2 dùng dev token (LiveKit --dev), dán tay; auth theo role để Phase 5.
import { ref } from "vue";
import { ViInput, ViButton, ViAlert, ViIcon } from "../design-system/vi-ui";
import { useLiveKitRoom } from "../composables/use-livekit-room";
import ParticipantTile from "../components/meeting/ParticipantTile.vue";
import MeetingControls from "../components/meeting/MeetingControls.vue";

// LiveKit dev chạy native trên WSL Ubuntu (port 7880, forward sang localhost Windows).
const url = ref("ws://localhost:7880");
const token = ref("");
const lk = useLiveKitRoom();

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
</style>
