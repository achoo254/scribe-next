<script setup lang="ts">
// Level meter custom (không có Vi* sẵn). Style chỉ bằng token CSS vars (One Series).
// props rms/peak 0..1. WCAG: role=meter + aria.
const props = defineProps<{ rms: number; peak: number }>();
const pct = (v: number) => Math.max(0, Math.min(100, v * 100));
</script>

<template>
  <div
    class="lm"
    role="meter"
    aria-label="Mức âm thanh"
    :aria-valuenow="Math.round(pct(props.rms))"
    aria-valuemin="0"
    aria-valuemax="100"
  >
    <div class="lm__bar" :style="{ width: pct(props.rms) + '%' }" />
    <div class="lm__peak" :style="{ left: pct(props.peak) + '%' }" />
  </div>
</template>

<style scoped>
.lm {
  position: relative;
  height: 14px;
  border-radius: var(--viui-radius-full);
  background: var(--viui-color-products-one-bg-card);
  border: 1px solid var(--viui-color-products-one-bg-border);
  overflow: hidden;
}
.lm__bar {
  height: 100%;
  background: var(--viui-color-products-one-green);
  border-radius: var(--viui-radius-full);
  transition: width var(--viui-motion-fast) linear;
}
.lm__peak {
  position: absolute;
  top: 0;
  width: 2px;
  height: 100%;
  background: var(--viui-color-products-one-orange);
  transition: left var(--viui-motion-fast) linear;
}
</style>
