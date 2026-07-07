<script setup lang="ts">
// OneScribe shell — 3 tab (Ghi âm / Cắt file / Transcript). UI dùng Vi* (iNET ViUI).
import { ref, computed, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { ViTabs, ViTab, ViIcon, ViAlert } from "./design-system/vi-ui";
import BrandMark from "./components/BrandMark.vue";
import RecordView from "./views/RecordView.vue";
import SplitView from "./views/SplitView.vue";
import TranscriptView from "./views/TranscriptView.vue";

const tab = ref<"record" | "split" | "transcript">("record");
const views = { record: RecordView, split: SplitView, transcript: TranscriptView };
const currentView = computed(() => views[tab.value]);

// Smoke wiring: xác nhận ffmpeg sidecar gọi được khi mở app.
const ffmpegInfo = ref("");
const ffmpegError = ref("");
onMounted(async () => {
  try {
    ffmpegInfo.value = await invoke<string>("ffmpeg_version");
  } catch (e) {
    ffmpegError.value = String(e);
  }
});
</script>

<template>
  <v-app>
    <v-main>
      <div class="os-shell">
        <header class="os-header">
          <BrandMark class="os-brand-icon" />
          <span class="os-brand-name">OneScribe</span>
        </header>
        <ViTabs v-model="tab" color="primary" class="os-tabs">
          <ViTab value="record"><ViIcon icon="IconMicrophone" start />Ghi âm</ViTab>
          <ViTab value="split"><ViIcon icon="IconScissors" start />Cắt file</ViTab>
          <ViTab value="transcript"><ViIcon icon="IconFileText" start />Transcript</ViTab>
        </ViTabs>

        <!-- keep-alive: giữ state mỗi tab (toggle, danh sách đoạn + transcript, file đã chọn) khi đổi tab. -->
        <keep-alive>
          <component :is="currentView" class="os-view" />
        </keep-alive>

        <ViAlert
          v-if="ffmpegError"
          type="error"
          density="compact"
          class="os-status"
        >
          ffmpeg lỗi: {{ ffmpegError }}
        </ViAlert>
        <p v-else class="os-status">{{ ffmpegInfo || "Đang kiểm tra ffmpeg…" }}</p>
      </div>
    </v-main>
  </v-app>
</template>

<style scoped>
.os-header {
  display: flex;
  align-items: center;
  gap: var(--viui-space-sm);
}
.os-brand-name {
  font-size: var(--viui-font-size-lg);
  font-weight: 600;
  color: rgb(var(--v-theme-primary));
}
.os-shell {
  display: flex;
  flex-direction: column;
  gap: var(--viui-space-md);
  padding: var(--viui-space-lg);
  max-width: var(--viui-layout-max);
  margin-inline: auto;
}
.os-view {
  flex: 1;
}
.os-status {
  font-size: var(--viui-font-size-sm);
  color: rgb(var(--v-theme-on-surface));
  opacity: 0.7;
  margin: 0;
}
</style>
