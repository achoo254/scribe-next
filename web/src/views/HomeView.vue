<script setup lang="ts">
// Shell OneScribe (web) — khớp desktop App.vue: header brand + 3 tab (Ghi âm / Cắt file / Transcript).
// Web hiện chỉ có Cắt file; Ghi âm + Transcript khoá kèm nhãn "Sắp có" để bám sát layout desktop.
import { ref, watch } from "vue";
import { ViTabs, ViTab, ViIcon } from "@hobui/viui";
import AppTopBar from "../components/AppTopBar.vue";
import SplitView from "./SplitView.vue";

const tab = ref("split");

// Chốt tab luôn ở "split" — 2 tab kia mới là placeholder (disabled), tránh nội dung trống nếu bị chọn.
watch(tab, (v) => {
  if (v !== "split") tab.value = "split";
});
</script>

<template>
  <div class="os-shell">
    <AppTopBar />

    <ViTabs v-model="tab" color="primary" class="os-tabs">
      <ViTab value="record" disabled>
        <ViIcon icon="microphone" start />Ghi âm
        <span class="os-soon">Sắp có</span>
      </ViTab>
      <ViTab value="split"><ViIcon icon="scissors" start />Cắt file</ViTab>
      <ViTab value="transcript" disabled>
        <ViIcon icon="file-text" start />Transcript
        <span class="os-soon">Sắp có</span>
      </ViTab>
    </ViTabs>

    <div class="os-view">
      <SplitView />
    </div>
  </div>
</template>

<style scoped>
.os-shell {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  padding: 1.5rem;
  max-width: 920px;
  margin-inline: auto;
}
.os-soon {
  margin-left: 0.4rem;
  font-size: 0.625rem;
  font-weight: 600;
  letter-spacing: 0.02em;
  text-transform: uppercase;
  padding: 0.1rem 0.35rem;
  border-radius: 999px;
  color: rgb(var(--v-theme-secondary));
  border: 1px solid rgb(var(--v-theme-secondary));
  opacity: 0.85;
  line-height: 1;
}
.os-view {
  flex: 1;
}
</style>
