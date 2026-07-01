<script setup lang="ts">
// Tile video 1 participant (web): gắn LiveKit video/audio track vào element. Audio CHỈ remote (tránh echo).
import { onBeforeUnmount, ref, watch } from "vue";
import type { Track } from "livekit-client";
import type { ParticipantVM } from "@onescribe/meeting-shared";

const props = defineProps<{ p: ParticipantVM }>();

const videoEl = ref<HTMLVideoElement | null>(null);
const audioEl = ref<HTMLAudioElement | null>(null);

function reattach(el: HTMLMediaElement | null, track: Track | null, prev?: Track | null): void {
  if (prev && el) prev.detach(el);
  if (track && el) track.attach(el);
}

watch(
  [videoEl, () => props.p.videoTrack],
  ([el, t], old) => reattach(el, t, old?.[1]),
  { immediate: true },
);
watch(
  [audioEl, () => props.p.audioTrack],
  ([el, t], old) => reattach(el, props.p.isLocal ? null : t, old?.[1]),
  { immediate: true },
);

onBeforeUnmount(() => {
  if (props.p.videoTrack && videoEl.value) props.p.videoTrack.detach(videoEl.value);
  if (props.p.audioTrack && audioEl.value) props.p.audioTrack.detach(audioEl.value);
});
</script>

<template>
  <div class="tile" :class="{ 'tile--speaking': p.speaking }">
    <video v-show="p.camOn && p.videoTrack" ref="videoEl" autoplay playsinline :muted="p.isLocal" />
    <div v-if="!(p.camOn && p.videoTrack)" class="tile__placeholder">{{ p.name.charAt(0).toUpperCase() }}</div>
    <audio v-if="!p.isLocal" ref="audioEl" autoplay />
    <div class="tile__bar">
      <span class="tile__name">{{ p.name }}{{ p.isLocal ? " (bạn)" : "" }}</span>
      <span v-if="!p.micOn" class="tile__muted">🔇</span>
    </div>
  </div>
</template>

<style scoped>
.tile {
  position: relative;
  aspect-ratio: 16 / 9;
  background: var(--one-bg-card);
  border: 2px solid var(--one-bg-border);
  border-radius: var(--radius);
  overflow: hidden;
}
.tile--speaking {
  border-color: var(--one-green);
}
.tile video {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.tile__placeholder {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 2.5rem;
  font-weight: 700;
  color: var(--one-green);
  background: var(--one-bg-card);
}
.tile__bar {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-xs);
  padding: var(--sp-xs) var(--sp-sm);
  background: linear-gradient(transparent, rgba(0, 0, 0, 0.65));
  font-size: 12px;
}
.tile__name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
