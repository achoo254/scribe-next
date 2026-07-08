<script setup lang="ts">
// Player audio/video đồng bộ transcript: click 1 segment → nhảy tới mốc + phát; highlight segment
// đang phát theo timeupdate. Custom component (không có Vi* sẵn cho media) — token-only, không hex.
import { ref } from "vue";
import type { Segment } from "../composables/use-transcribe";

const props = defineProps<{
  src: string; // đã qua convertFileSrc
  isVideo: boolean;
  segments: Segment[];
}>();

const mediaEl = ref<HTMLMediaElement | null>(null);
const activeIdx = ref(-1);

function seek(s: Segment, i: number) {
  const el = mediaEl.value;
  if (!el) return;
  el.currentTime = s.start;
  el.play().catch(() => {});
  activeIdx.value = i;
}

function onTimeUpdate() {
  const t = mediaEl.value?.currentTime ?? 0;
  activeIdx.value = props.segments.findIndex((s) => t >= s.start && t < s.end);
}

function fmt(sec: number): string {
  const m = Math.floor(sec / 60);
  const s = Math.floor(sec % 60);
  return `${m}:${String(s).padStart(2, "0")}`;
}
</script>

<template>
  <div class="os-player">
    <video
      v-if="props.isVideo"
      ref="mediaEl"
      :src="props.src"
      controls
      class="os-media os-video"
      @timeupdate="onTimeUpdate"
    />
    <audio
      v-else
      ref="mediaEl"
      :src="props.src"
      controls
      class="os-media"
      @timeupdate="onTimeUpdate"
    />
    <div v-if="props.segments.length" class="os-seglist">
      <button
        v-for="(s, i) in props.segments"
        :key="i"
        type="button"
        class="os-segrow"
        :class="{ active: i === activeIdx }"
        @click="seek(s, i)"
      >
        <span class="os-segt">{{ fmt(s.start) }}</span>
        <span class="os-segx">{{ s.text.trim() }}</span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.os-player {
  display: flex;
  flex-direction: column;
  gap: var(--viui-space-sm);
}
.os-media {
  width: 100%;
}
.os-video {
  max-height: 320px;
  background: rgb(var(--v-theme-surface));
  border-radius: var(--viui-radius-sm, 8px);
}
.os-seglist {
  display: flex;
  flex-direction: column;
  max-height: 220px;
  overflow-y: auto;
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: var(--viui-radius-sm, 8px);
}
.os-segrow {
  display: flex;
  gap: var(--viui-space-sm);
  align-items: baseline;
  padding: var(--viui-space-xs) var(--viui-space-sm);
  background: transparent;
  border: none;
  border-bottom: 1px solid rgb(var(--v-theme-outline));
  text-align: left;
  cursor: pointer;
  color: rgb(var(--v-theme-on-surface));
  font-size: var(--viui-font-size-sm);
}
.os-segrow:last-child {
  border-bottom: none;
}
.os-segrow:hover {
  background: rgb(var(--v-theme-surface));
}
.os-segrow.active {
  background: rgb(var(--v-theme-surface));
  color: rgb(var(--v-theme-primary));
}
.os-segt {
  flex: 0 0 auto;
  font-variant-numeric: tabular-nums;
  opacity: 0.75;
  min-width: 3rem;
}
.os-segx {
  flex: 1 1 auto;
  white-space: pre-wrap;
  word-break: break-word;
}
</style>
