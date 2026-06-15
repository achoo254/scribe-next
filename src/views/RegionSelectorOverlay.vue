<script setup lang="ts">
// Overlay chọn vùng ghi màn hình (Bandicam-style). Cửa sổ trong suốt phủ trọn 1 monitor.
// Kéo chuột → khung chọn (CSS px). Khi thả: nhân devicePixelRatio → px VẬT LÝ theo monitor
// (xử lý DPI scaling 125/150%), làm tròn chẵn (yuv420p) → gửi finish_region_selection.
// Màu khung dùng hex literal: overlay là công cụ hệ thống tạm thời, không nằm trong design surface.
import { ref, computed, onMounted, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";

const dragging = ref(false);
const sx = ref(0);
const sy = ref(0);
const ex = ref(0);
const ey = ref(0);
const dpr = ref(window.devicePixelRatio || 1);

// Khung chọn theo CSS px (hiển thị).
const sel = computed(() => ({
  x: Math.min(sx.value, ex.value),
  y: Math.min(sy.value, ey.value),
  w: Math.abs(ex.value - sx.value),
  h: Math.abs(ey.value - sy.value),
}));

// Kích thước vật lý hiển thị cho người dùng (đúng với px sẽ ghi).
const physW = computed(() => Math.round(sel.value.w * dpr.value));
const physH = computed(() => Math.round(sel.value.h * dpr.value));

function onDown(e: MouseEvent) {
  dragging.value = true;
  sx.value = ex.value = e.clientX;
  sy.value = ey.value = e.clientY;
}
function onMove(e: MouseEvent) {
  if (!dragging.value) return;
  ex.value = e.clientX;
  ey.value = e.clientY;
}
function onUp() {
  if (!dragging.value) return;
  dragging.value = false;
  const s = sel.value;
  if (s.w < 8 || s.h < 8) {
    invoke("cancel_region_selection"); // click lỡ → huỷ
    return;
  }
  const ratio = dpr.value;
  let w = Math.round(s.w * ratio);
  let h = Math.round(s.h * ratio);
  w -= w % 2; // H.264 yuv420p cần kích thước chẵn
  h -= h % 2;
  invoke("finish_region_selection", {
    x: Math.round(s.x * ratio),
    y: Math.round(s.y * ratio),
    w,
    h,
  });
}
function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") invoke("cancel_region_selection");
}

onMounted(() => window.addEventListener("keydown", onKey));
onUnmounted(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <div class="rs-root" @mousedown="onDown" @mousemove="onMove" @mouseup="onUp">
    <div v-if="!dragging" class="rs-hint">Kéo để chọn vùng ghi · Esc để huỷ</div>
    <div
      v-if="dragging"
      class="rs-box"
      :style="{ left: sel.x + 'px', top: sel.y + 'px', width: sel.w + 'px', height: sel.h + 'px' }"
    >
      <span class="rs-dim">{{ physW }} × {{ physH }}</span>
    </div>
  </div>
</template>

<style scoped>
.rs-root {
  position: fixed;
  inset: 0;
  cursor: crosshair;
  background: rgba(0, 0, 0, 0.35);
  user-select: none;
  overflow: hidden;
}
.rs-hint {
  position: fixed;
  top: 16px;
  left: 50%;
  transform: translateX(-50%);
  color: #fff;
  background: rgba(0, 0, 0, 0.65);
  padding: 6px 14px;
  border-radius: 8px;
  font: 14px system-ui, sans-serif;
  pointer-events: none;
}
.rs-box {
  position: fixed;
  border: 2px solid #4aa3ff;
  background: rgba(74, 163, 255, 0.12);
  box-sizing: border-box;
  pointer-events: none;
}
.rs-dim {
  position: absolute;
  top: -26px;
  left: 0;
  color: #fff;
  background: rgba(0, 0, 0, 0.75);
  padding: 2px 8px;
  border-radius: 4px;
  font: 12px system-ui, sans-serif;
  white-space: nowrap;
}
</style>
