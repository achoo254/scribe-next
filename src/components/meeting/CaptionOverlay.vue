<script setup lang="ts">
// Overlay caption gọn (Phase 3): hiện 2 dòng final mới nhất đè lên grid video (kiểu phụ đề TV).
// Dùng khi muốn caption nổi trên video thay vì panel bên cạnh.
import { computed } from "vue";
import type { CaptionLine } from "@onescribe/meeting-shared";

const props = defineProps<{ captions: CaptionLine[] }>();
const recent = computed(() => props.captions.filter((c) => c.final).slice(-2));
</script>

<template>
  <div v-if="recent.length" class="caption-overlay">
    <div v-for="c in recent" :key="c.id" class="caption-overlay__line">
      <span class="caption-overlay__name">{{ c.name }}:</span>
      <span>{{ c.text }}</span>
    </div>
  </div>
</template>

<style scoped>
.caption-overlay {
  position: absolute;
  left: 50%;
  bottom: var(--viui-space-md);
  transform: translateX(-50%);
  max-width: min(90%, 720px);
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: var(--viui-space-xs) var(--viui-space-sm);
  border-radius: var(--viui-radius-md, 8px);
  background: rgba(0, 0, 0, 0.62);
  color: #fff;
  text-align: center;
  pointer-events: none;
}
.caption-overlay__line {
  font-size: var(--viui-font-size-md, 15px);
  line-height: 1.4;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.9);
}
.caption-overlay__name {
  font-weight: 700;
  margin-right: var(--viui-space-xs);
}
</style>
