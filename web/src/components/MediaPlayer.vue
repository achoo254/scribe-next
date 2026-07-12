<script setup lang="ts">
// Player phát lại recording qua `/api/files/:id` (BE stream + HTTP Range → tua được).
// video=true → <video> (ghi màn hình); false → <audio> (ghi mic).
defineProps<{ fileId: string; video?: boolean }>();
</script>

<template>
  <video
    v-if="video"
    :src="`/api/files/${fileId}`"
    controls
    preload="metadata"
    class="os-media-video"
  />
  <audio v-else :src="`/api/files/${fileId}`" controls preload="metadata" class="os-media-audio" />
</template>

<style scoped>
.os-media-video {
  width: 100%;
  max-height: 360px;
  border-radius: 8px;
  background: #000;
  display: block;
}
.os-media-audio {
  width: 100%;
  display: block;
}
</style>
