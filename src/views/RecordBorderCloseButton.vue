<script setup lang="ts">
// Nút ✕ đóng khung vùng ghi — cửa sổ nhỏ riêng (click được) ở góc trên-phải khung. Tách riêng để
// khung viền vẫn click-through. Bấm → xoá vùng (Rust emit về app chính + đóng cả khung lẫn nút).
// Chỉ hiện lúc chưa ghi nên không lọt vào video. Nút lấp đầy cửa sổ (kích thước do Rust quy định)
// + ✕ SVG theo % → không phụ thuộc DPI, không tràn/scroll. Màu hex literal: overlay công cụ hệ thống.
import { invoke } from "@tauri-apps/api/core";
const onClose = () => invoke("clear_record_region").catch(() => {});
</script>

<template>
  <button class="rbc" type="button" aria-label="Đóng khung vùng ghi" title="Đóng khung" @click="onClose">
    <svg class="rbc-x" viewBox="0 0 24 24" aria-hidden="true">
      <path d="M7 7 L17 17 M17 7 L7 17" />
    </svg>
  </button>
</template>

<style scoped>
.rbc {
  position: fixed;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  margin: 0;
  padding: 0;
  box-sizing: border-box;
  border: 2px solid #fff;
  border-radius: 50%;
  background: #e5484d;
  cursor: pointer;
  overflow: hidden;
  transition: background 120ms ease, transform 100ms ease;
}
.rbc:hover {
  background: #f2555a;
}
.rbc:active {
  transform: scale(0.9);
}
.rbc-x {
  width: 52%;
  height: 52%;
  stroke: #fff;
  stroke-width: 2.6;
  stroke-linecap: round;
}
</style>
