<script setup lang="ts">
// Thanh điều khiển họp: mute mic / tắt camera / rời phòng. Chỉ dùng Vi* (ViButton/ViIcon).
import { ViButton, ViIcon } from "../../design-system/vi-ui";

defineProps<{ micOn: boolean; camOn: boolean }>();
const emit = defineEmits<{
  (e: "toggle-mic"): void;
  (e: "toggle-camera"): void;
  (e: "leave"): void;
}>();
</script>

<template>
  <div class="controls">
    <ViButton :color="micOn ? 'primary' : 'error'" variant="tonal" @click="emit('toggle-mic')">
      <ViIcon :icon="micOn ? 'IconMicrophone' : 'IconMicrophoneOff'" start />
      {{ micOn ? "Tắt mic" : "Bật mic" }}
    </ViButton>
    <ViButton :color="camOn ? 'primary' : 'error'" variant="tonal" @click="emit('toggle-camera')">
      <ViIcon :icon="camOn ? 'IconVideo' : 'IconVideoOff'" start />
      {{ camOn ? "Tắt camera" : "Bật camera" }}
    </ViButton>
    <ViButton color="error" @click="emit('leave')">
      <ViIcon icon="IconDoorExit" start />
      Rời phòng
    </ViButton>
  </div>
</template>

<style scoped>
.controls {
  display: flex;
  flex-wrap: wrap;
  gap: var(--viui-space-sm);
  justify-content: center;
}
</style>
