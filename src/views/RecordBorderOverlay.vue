<script setup lang="ts">
// Viền đỏ quanh vùng đang ghi (kiểu Bandicam). Cửa sổ trong suốt + click-through, đã được Rust nở
// ra ngoài vùng ghi vài px. Viền vẽ ở rìa (box-sizing: border-box) nên nằm NGOÀI vùng ghi thực
// → ddagrab không chụp viền vào video. Độ dày quy về px VẬT LÝ (chia devicePixelRatio) để khớp
// phần nở ở Rust. Màu đỏ dùng hex literal: overlay công cụ hệ thống, không thuộc design surface.
const BORDER_PHYS = 3; // px vật lý — phải ≤ BORDER_INFLATE trong screen.rs (=4) để không lọt vào video
const borderWidth = BORDER_PHYS / (window.devicePixelRatio || 1) + "px";
</script>

<template>
  <div class="rb" :style="{ borderWidth }"></div>
</template>

<style scoped>
.rb {
  position: fixed;
  inset: 0;
  box-sizing: border-box;
  border-style: solid;
  border-color: #ff3b30;
  pointer-events: none;
}
</style>
