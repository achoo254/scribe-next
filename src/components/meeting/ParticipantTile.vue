<script setup lang="ts">
// 1 ô video/audio cho 1 participant. Gắn (attach) Track LiveKit vào <video>/<audio>;
// detach khi đổi track/unmount để KHÔNG leak MediaStream. Render tên + trạng thái mic/cam (Vi*).
import { ref, watch, onMounted, onBeforeUnmount } from "vue";
import type { Track } from "livekit-client";
import { ViIcon } from "../../design-system/vi-ui";
import type { ParticipantVM } from "../../composables/use-livekit-room";

const props = defineProps<{ p: ParticipantVM }>();

const videoEl = ref<HTMLVideoElement | null>(null);
const audioEl = ref<HTMLAudioElement | null>(null);
let attachedVideo: Track | null = null;
let attachedAudio: Track | null = null;

function syncVideo() {
  const t = props.p.camOn ? props.p.videoTrack : null;
  if (t === attachedVideo) return;
  if (attachedVideo && videoEl.value) attachedVideo.detach(videoEl.value);
  attachedVideo = null;
  if (t && videoEl.value) {
    t.attach(videoEl.value);
    attachedVideo = t;
  }
}

function syncAudio() {
  // KHÔNG phát lại tiếng của chính mình (echo) — chỉ attach audio remote.
  const t = props.p.isLocal ? null : props.p.audioTrack;
  if (t === attachedAudio) return;
  if (attachedAudio && audioEl.value) attachedAudio.detach(audioEl.value);
  attachedAudio = null;
  if (t && audioEl.value) {
    t.attach(audioEl.value);
    attachedAudio = t;
  }
}

onMounted(() => {
  syncVideo();
  syncAudio();
});
watch(() => [props.p.videoTrack, props.p.camOn], syncVideo, { flush: "post" });
watch(() => props.p.audioTrack, syncAudio, { flush: "post" });

onBeforeUnmount(() => {
  if (attachedVideo && videoEl.value) attachedVideo.detach(videoEl.value);
  if (attachedAudio && audioEl.value) attachedAudio.detach(audioEl.value);
});
</script>

<template>
  <div class="tile" :class="{ 'tile--speaking': p.speaking }">
    <!-- muted cho local: tránh nghe lại tiếng mình; remote audio đi qua <audio> riêng. -->
    <video
      v-show="p.camOn"
      ref="videoEl"
      class="tile__video"
      autoplay
      playsinline
      :muted="p.isLocal"
    />
    <div v-show="!p.camOn" class="tile__novideo">
      <ViIcon icon="IconVideoOff" />
    </div>
    <audio v-if="!p.isLocal" ref="audioEl" autoplay />
    <div class="tile__bar">
      <span class="tile__name">{{ p.name }}<span v-if="p.isLocal"> (bạn)</span></span>
      <ViIcon
        :icon="p.micOn ? 'IconMicrophone' : 'IconMicrophoneOff'"
        :class="{ 'tile__mic--off': !p.micOn }"
      />
    </div>
  </div>
</template>

<style scoped>
.tile {
  position: relative;
  aspect-ratio: 16 / 9;
  background: rgb(var(--v-theme-surface));
  border: 2px solid transparent;
  border-radius: var(--viui-radius-md, 8px);
  overflow: hidden;
}
.tile--speaking {
  border-color: rgb(var(--v-theme-primary));
}
.tile__video {
  width: 100%;
  height: 100%;
  object-fit: cover;
  background: #000;
}
.tile__novideo {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  color: rgb(var(--v-theme-on-surface));
  opacity: 0.4;
  font-size: 2rem;
}
.tile__bar {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--viui-space-sm);
  padding: var(--viui-space-xs) var(--viui-space-sm);
  background: rgba(0, 0, 0, 0.45);
}
.tile__name {
  font-size: var(--viui-font-size-sm);
  color: #fff;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.tile__mic--off {
  color: rgb(var(--v-theme-error));
}
</style>
