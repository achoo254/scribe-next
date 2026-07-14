<script setup lang="ts">
// Player phát lại recording qua `/api/files/:id` (BE stream + HTTP Range → tua được).
// video=true → <video> native (ghi màn hình). false → audio player TÙY BIẾN: volume popup nổi
// PHÍA TRÊN nút loa (không đè lên thanh progress như native controls của trình duyệt).
import { ref, computed } from "vue";
import { ViIcon } from "@hobui/viui";

const props = defineProps<{ fileId: string; video?: boolean }>();

const audioEl = ref<HTMLAudioElement | null>(null);
const playing = ref(false);
const currentTime = ref(0);
const duration = ref(0);
const volume = ref(1);
const muted = ref(false);

const hasDur = computed(() => Number.isFinite(duration.value) && duration.value > 0);
const volIcon = computed(() => (muted.value || volume.value === 0 ? "volume-off" : "volume"));

function fmt(s: number): string {
  const t = Number.isFinite(s) && s > 0 ? Math.floor(s) : 0;
  const m = Math.floor(t / 60);
  const ss = String(t % 60).padStart(2, "0");
  return `${m}:${ss}`;
}

function togglePlay(): void {
  const el = audioEl.value;
  if (!el) return;
  if (el.paused) void el.play();
  else el.pause();
}
function onLoaded(): void {
  const el = audioEl.value;
  if (el) duration.value = el.duration;
}
function onTime(): void {
  const el = audioEl.value;
  if (el) currentTime.value = el.currentTime;
}
function onSeek(e: Event): void {
  const el = audioEl.value;
  if (!el) return;
  el.currentTime = Number((e.target as HTMLInputElement).value);
}
function onVolume(e: Event): void {
  const el = audioEl.value;
  if (!el) return;
  const v = Number((e.target as HTMLInputElement).value);
  el.volume = v;
  el.muted = v === 0; // kéo về 0 = mute; kéo lên = bỏ mute
}
function toggleMute(): void {
  const el = audioEl.value;
  if (!el) return;
  el.muted = !el.muted;
}
function onVolChange(): void {
  const el = audioEl.value;
  if (!el) return;
  volume.value = el.volume;
  muted.value = el.muted;
}
</script>

<template>
  <video
    v-if="props.video"
    :src="`/api/files/${props.fileId}`"
    controls
    preload="metadata"
    class="os-media-video"
  />
  <div v-else class="os-audio">
    <audio
      ref="audioEl"
      :src="`/api/files/${props.fileId}`"
      preload="metadata"
      @loadedmetadata="onLoaded"
      @durationchange="onLoaded"
      @timeupdate="onTime"
      @play="playing = true"
      @pause="playing = false"
      @ended="playing = false"
      @volumechange="onVolChange"
    />

    <button class="os-a-btn" type="button" :aria-label="playing ? 'Tạm dừng' : 'Phát'" @click="togglePlay">
      <ViIcon :icon="playing ? 'player-pause' : 'player-play'" />
    </button>

    <span class="os-a-time">{{ fmt(currentTime) }} / {{ hasDur ? fmt(duration) : "—" }}</span>

    <input
      class="os-a-seek"
      type="range"
      min="0"
      :max="hasDur ? duration : 0"
      step="0.01"
      :value="currentTime"
      :disabled="!hasDur"
      aria-label="Tua"
      @input="onSeek"
    />

    <div class="os-a-vol">
      <button class="os-a-btn" type="button" :aria-label="muted ? 'Bật tiếng' : 'Tắt tiếng'" @click="toggleMute">
        <ViIcon :icon="volIcon" />
      </button>
      <!-- Popup nổi PHÍA TRÊN nút loa → không che progress; hiện khi hover/focus nhóm volume. -->
      <div class="os-a-vol-pop">
        <input
          class="os-a-vol-range"
          type="range"
          min="0"
          max="1"
          step="0.05"
          :value="muted ? 0 : volume"
          aria-label="Âm lượng"
          @input="onVolume"
        />
      </div>
    </div>
  </div>
</template>

<style scoped>
.os-media-video {
  width: 100%;
  max-height: 360px;
  border-radius: 8px;
  background: #000;
  display: block;
}

/* Thanh audio bo tròn, tông tối khớp theme. */
.os-audio {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  width: 100%;
  padding: 0.4rem 0.75rem;
  border-radius: 999px;
  background: rgb(var(--v-theme-surface-bright));
}
.os-a-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  flex: none;
  padding: 0;
  border: none;
  border-radius: 50%;
  background: transparent;
  color: rgb(var(--v-theme-on-surface));
  cursor: pointer;
}
.os-a-btn:hover {
  background: rgba(var(--v-theme-on-surface), 0.1);
}
.os-a-time {
  flex: none;
  font-size: 0.8rem;
  font-variant-numeric: tabular-nums;
  color: rgb(var(--v-theme-on-surface));
  opacity: 0.85;
}
.os-a-seek {
  flex: 1;
  min-width: 0;
}

/* Nhóm volume: popup nổi phía trên, ẩn mặc định. */
.os-a-vol {
  position: relative;
  flex: none;
  display: flex;
  align-items: center;
}
.os-a-vol-pop {
  position: absolute;
  bottom: calc(100% + 6px);
  left: 50%;
  transform: translateX(-50%);
  padding: 0.6rem 0.4rem;
  border-radius: 8px;
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-surface-bright));
  box-shadow: 0 4px 14px rgba(0, 0, 0, 0.4);
  opacity: 0;
  visibility: hidden;
  transition: opacity 120ms ease;
  z-index: 5;
}
.os-a-vol:hover .os-a-vol-pop,
.os-a-vol:focus-within .os-a-vol-pop {
  opacity: 1;
  visibility: visible;
}
/* Slider dọc trong popup (Chrome/Edge — nền tảng chính của app). */
.os-a-vol-range {
  writing-mode: vertical-rl;
  direction: rtl; /* min ở dưới, max ở trên */
  -webkit-appearance: slider-vertical;
  appearance: slider-vertical;
  width: 6px;
  height: 90px;
}

/* Range track/thumb khớp màu primary (Chrome/Edge/Safari). */
.os-a-seek,
.os-a-vol-range {
  accent-color: rgb(var(--v-theme-primary));
  cursor: pointer;
}
.os-a-seek:disabled {
  opacity: 0.5;
  cursor: default;
}
</style>
