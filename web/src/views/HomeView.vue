<script setup lang="ts">
// Shell OneScribe (web) — khớp desktop App.vue: header brand + 3 tab (Ghi âm / Cắt file / Transcript).
// Ghi âm + Cắt file đã có; Transcript khoá kèm nhãn "Sắp có" để bám sát layout desktop.
import { ref, watch } from "vue";
import { ViTabs, ViTab, ViIcon } from "@hobui/viui";
import AppTopBar from "../components/AppTopBar.vue";
import QuotaBar from "../components/QuotaBar.vue";
import RecordView from "./RecordView.vue";
import SplitView from "./SplitView.vue";

const tab = ref("record");

// Chặn chọn tab placeholder (transcript) — snap về record. Record/split đều hoạt động.
watch(tab, (v) => {
  if (v === "transcript") tab.value = "record";
});
</script>

<template>
  <div class="os-shell">
    <AppTopBar />

    <ViTabs v-model="tab" color="primary" class="os-tabs">
      <ViTab value="record"><ViIcon icon="microphone" start />Ghi âm</ViTab>
      <ViTab value="split"><ViIcon icon="scissors" start />Cắt file</ViTab>
      <ViTab value="transcript" disabled>
        <ViIcon icon="file-text" start />Transcript
        <span class="os-soon">Sắp có</span>
      </ViTab>
    </ViTabs>

    <QuotaBar />

    <!-- keep-alive: giữ state khi đổi tab (đang ghi / job cắt đang chạy) — giống desktop App.vue. -->
    <div class="os-view">
      <keep-alive>
        <RecordView v-if="tab === 'record'" />
        <SplitView v-else-if="tab === 'split'" />
      </keep-alive>
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
