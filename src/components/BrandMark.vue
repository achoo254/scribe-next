<script setup lang="ts">
// Brand mark OneScribe — sóng âm (primary) → dòng transcript (accent). Khớp app icon.
// Màu lấy từ token theme inetOneDark (primary=green, accent=orange) — KHÔNG hardcode hex.
withDefaults(defineProps<{ size?: number }>(), { size: 28 });
</script>

<template>
  <svg
    :width="size"
    :height="size"
    viewBox="0 0 32 32"
    role="img"
    aria-label="OneScribe"
    class="os-brand-mark"
  >
    <!-- sóng âm -->
    <g class="wave">
      <rect x="3.5" y="10" width="3.5" height="12" rx="1.75" />
      <rect x="9" y="6" width="3.5" height="20" rx="1.75" />
      <rect x="14.5" y="9" width="3.5" height="14" rx="1.75" />
    </g>
    <!-- dòng transcript -->
    <g class="lines">
      <rect x="21" y="9" width="8" height="3.5" rx="1.75" />
      <rect x="21" y="14.25" width="6.5" height="3.5" rx="1.75" />
      <rect x="21" y="19.5" width="5" height="3.5" rx="1.75" />
    </g>
  </svg>
</template>

<style scoped>
.os-brand-mark {
  display: block;
  flex: none;
}
.os-brand-mark .wave rect {
  fill: rgb(var(--v-theme-primary));
}
.os-brand-mark .lines rect {
  fill: rgb(var(--v-theme-accent));
}
</style>
