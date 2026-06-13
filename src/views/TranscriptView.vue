<script setup lang="ts">
// Mode C — Transcribe offline tiếng Việt (PhoWhisper). UI Vi* (iNET ViUI), token-only, WCAG AA.
import { onMounted } from "vue";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  ViCard, ViButton, ViIcon, ViList, ViListItem,
  ViTextarea, ViProgressLinear, ViAlert,
} from "../design-system/vi-ui";
import { useTranscribe } from "../composables/use-transcribe";

const tr = useTranscribe();
onMounted(() => tr.init());

const base = (p: string) => p.split(/[\\/]/).pop() || p;
const locate = (p: string | null) => {
  if (p) revealItemInDir(p).catch(() => {});
};
async function copyText() {
  try {
    await navigator.clipboard.writeText(tr.panelText.value);
  } catch {
    /* clipboard không khả dụng */
  }
}
</script>

<template>
  <ViCard title="Transcript" class="pa-4">
    <div class="os-form">
      <div class="os-row">
        <ViButton variant="outlined" :disabled="tr.transcribing.value" @click="tr.pickFiles()">
          <ViIcon icon="IconFolder" start />Chọn file
        </ViButton>
        <span class="os-muted">{{ tr.files.value.length }} file</span>
      </div>
      <ViList v-if="tr.files.value.length" density="compact" class="os-box">
        <ViListItem v-for="(p, i) in tr.files.value" :key="i" :title="base(p)" />
      </ViList>

      <!-- Trạng thái model -->
      <ViAlert v-if="tr.modelReady.value" type="success" variant="tonal" density="compact">
        Model PhoWhisper sẵn sàng (chạy offline).
      </ViAlert>
      <template v-else>
        <ViAlert type="warning" variant="tonal" density="compact">
          Chưa có model — cần tải PhoWhisper-large (~3GB) lần đầu, sau đó chạy offline.
        </ViAlert>
        <div class="os-row">
          <ViButton color="secondary" :disabled="tr.downloading.value" @click="tr.ensureModel()">
            <ViIcon icon="IconDownload" start />Tải model
          </ViButton>
          <span v-if="tr.downloading.value" class="os-muted">{{ tr.downloadPct.value }}%</span>
        </div>
        <ViProgressLinear
          v-if="tr.downloading.value"
          :model-value="tr.downloadPct.value"
          :indeterminate="tr.downloadPct.value === 0"
          color="secondary"
          height="6"
          rounded
        />
      </template>
      <!-- Đường dẫn model: tách riêng để KHÔNG phá chuỗi v-if/v-else ở trên -->
      <p v-if="tr.modelPath.value" class="os-muted os-path">
        <ViIcon icon="IconFolder" />&nbsp;Model lưu tại: {{ tr.modelPath.value }}
      </p>

      <!-- Điều khiển transcribe -->
      <div class="os-row">
        <ViButton
          color="primary"
          size="large"
          :disabled="!tr.files.value.length || tr.transcribing.value || !tr.modelReady.value"
          @click="tr.transcribe()"
        >
          <ViIcon icon="IconFileText" start />Transcribe
        </ViButton>
        <ViButton v-if="tr.transcribing.value" color="error" variant="outlined" @click="tr.cancel()">
          Hủy
        </ViButton>
      </div>

      <span v-if="tr.deviceInfo.value" class="os-muted">⚙ {{ tr.deviceInfo.value }}</span>

      <div v-if="tr.transcribing.value" class="os-prog">
        <span class="os-muted">{{ base(tr.currentFile.value) }} · {{ tr.progressPct.value }}%</span>
        <ViProgressLinear :model-value="tr.progressPct.value" color="primary" height="6" rounded />
      </div>

      <ViAlert v-if="tr.error.value" type="error" density="compact" closable>
        {{ tr.error.value }}
      </ViAlert>
      <ViAlert
        v-for="(e, i) in tr.fileErrors.value"
        :key="'fe' + i"
        type="error"
        density="compact"
        closable
      >
        {{ base(e.file) }}: {{ e.message }}
      </ViAlert>

      <!-- Panel transcript -->
      <ViTextarea
        v-if="tr.panelText.value"
        :model-value="tr.panelText.value"
        label="Nội dung"
        readonly
        rows="8"
        auto-grow
      />
      <div v-if="tr.panelText.value" class="os-row">
        <ViButton variant="text" size="small" @click="copyText">
          <ViIcon icon="IconCopy" start />Copy
        </ViButton>
      </div>

      <!-- Kết quả (file .txt/.srt cạnh file gốc) -->
      <ViList v-if="tr.results.value.length" density="compact" class="os-box">
        <ViListItem
          v-for="(r, i) in tr.results.value"
          :key="i"
          :title="base(r.file)"
          subtitle=".txt + .srt cạnh file gốc"
        >
          <template #append>
            <ViButton variant="text" size="small" @click="locate(r.txtPath)">
              <ViIcon icon="IconFileText" />
            </ViButton>
          </template>
        </ViListItem>
      </ViList>
    </div>
  </ViCard>
</template>

<style scoped>
.os-form {
  display: flex;
  flex-direction: column;
  gap: var(--viui-space-md);
}
.os-row {
  display: flex;
  align-items: center;
  gap: var(--viui-space-sm);
  flex-wrap: wrap;
}
.os-muted {
  font-size: var(--viui-font-size-sm);
  color: rgb(var(--v-theme-on-surface));
  opacity: 0.75;
}
.os-path {
  word-break: break-all;
  margin: 0;
}
.os-box {
  max-height: 160px;
  overflow-y: auto;
  background: transparent;
}
.os-prog {
  display: flex;
  flex-direction: column;
  gap: var(--viui-space-xs);
}
</style>
