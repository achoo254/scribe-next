<script setup lang="ts">
// Thanh công cụ nhỏ phía trên khung vùng ghi (CHỈ hiện lúc idle): tay nắm KÉO ⠿ để di chuyển vùng
// + nút ✕ đóng khung. Cửa sổ riêng (click được) để khung viền giữ click-through; vì chỉ hiện khi
// chưa ghi nên không lọt vào video. Kéo: gửi TỔNG độ dời (px vật lý) so với lúc bấm xuống về Rust —
// Rust dời cửa sổ trực tiếp (mượt) + kẹp trong màn hình, đồng bộ vùng về app chính khi thả.
// Màu hex literal: overlay công cụ hệ thống, ngoài design surface.
import { invoke } from "@tauri-apps/api/core";

const dpr = window.devicePixelRatio || 1; // cửa sổ nằm trên monitor của vùng → đúng scale để quy px vật lý
let startX = 0;
let startY = 0;
let dragging = false;
let raf = 0;
let dx = 0;
let dy = 0;

function flush() {
  raf = 0;
  invoke("update_region_drag", { dx, dy }).catch(() => {});
}

function onPointerDown(e: PointerEvent) {
  if (e.button !== 0) return;
  dragging = true;
  startX = e.screenX;
  startY = e.screenY;
  dx = dy = 0;
  (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  invoke("begin_region_drag").catch(() => {});
  e.preventDefault();
}
function onPointerMove(e: PointerEvent) {
  if (!dragging) return;
  // screenX/Y theo CSS px (screen-absolute) → delta × dpr = px vật lý; gộp theo frame cho mượt.
  dx = Math.round((e.screenX - startX) * dpr);
  dy = Math.round((e.screenY - startY) * dpr);
  if (!raf) raf = requestAnimationFrame(flush);
}
function onPointerUp(e: PointerEvent) {
  if (!dragging) return;
  dragging = false;
  if (raf) {
    cancelAnimationFrame(raf);
    raf = 0;
  }
  // Gửi delta cuối rồi chốt phiên (báo vùng mới về app chính để lưu).
  invoke("update_region_drag", { dx, dy })
    .catch(() => {})
    .finally(() => invoke("end_region_drag").catch(() => {}));
  try {
    (e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId);
  } catch {
    /* pointer đã nhả */
  }
}

const onClose = () => invoke("clear_record_region").catch(() => {});
</script>

<template>
  <div class="rbt">
    <div
      class="rbt-grip"
      title="Kéo để di chuyển vùng ghi"
      @pointerdown="onPointerDown"
      @pointermove="onPointerMove"
      @pointerup="onPointerUp"
    >
      <svg class="rbt-ico" viewBox="0 0 24 24" aria-hidden="true">
        <circle cx="9" cy="6" r="1.7" /><circle cx="15" cy="6" r="1.7" />
        <circle cx="9" cy="12" r="1.7" /><circle cx="15" cy="12" r="1.7" />
        <circle cx="9" cy="18" r="1.7" /><circle cx="15" cy="18" r="1.7" />
      </svg>
    </div>
    <button class="rbt-close" type="button" aria-label="Đóng khung vùng ghi" title="Đóng khung" @click="onClose">
      <svg class="rbt-x" viewBox="0 0 24 24" aria-hidden="true">
        <path d="M7 7 L17 17 M17 7 L7 17" />
      </svg>
    </button>
  </div>
</template>

<style scoped>
.rbt {
  position: fixed;
  inset: 0;
  display: flex;
  align-items: stretch;
  margin: 0;
  box-sizing: border-box;
  overflow: hidden;
  border: 1px solid #ff3b30; /* đồng bộ viền khung */
  border-radius: 6px;
  background: #1f2530;
  user-select: none;
}
.rbt-grip {
  flex: 1 1 auto;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: move;
  touch-action: none; /* pointer events không bị cuộn/zoom can thiệp */
}
.rbt-grip:hover {
  background: rgba(255, 255, 255, 0.08);
}
.rbt-ico {
  width: 56%;
  height: 56%;
  fill: #cfd6e0;
}
.rbt-close {
  flex: 0 0 44%;
  display: flex;
  align-items: center;
  justify-content: center;
  margin: 0;
  padding: 0;
  border: none;
  border-left: 1px solid rgba(255, 255, 255, 0.18);
  background: #e5484d;
  cursor: pointer;
  transition: background 120ms ease;
}
.rbt-close:hover {
  background: #f2555a;
}
.rbt-close:active {
  transform: scale(0.94);
}
.rbt-x {
  width: 50%;
  height: 50%;
  stroke: #fff;
  stroke-width: 2.6;
  stroke-linecap: round;
}
</style>
