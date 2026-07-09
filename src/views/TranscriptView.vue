<script setup lang="ts">
// Mode C — Transcribe offline tiếng Việt (PhoWhisper). UI Vi* (iNET ViUI), token-only, WCAG AA.
import { onMounted, onActivated } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { ask } from "@tauri-apps/plugin-dialog";
import {
  ViCard, ViButton, ViIcon, ViList, ViListItem, ViSelect,
  ViTextarea, ViProgressLinear, ViAlert, ViButtonToggle, ViSwitch,
} from "../design-system/vi-ui";
import TranscriptPlayer from "../components/TranscriptPlayer.vue";
import { useTranscribe, isVideoPath } from "../composables/use-transcribe";
import { useAsrModels, useModelManager, formatBytes, type ModelSize } from "../composables/asr-models";
import { useGlossary } from "../composables/use-glossary";
import { useGpuSetting, useCudaRuntime } from "../composables/use-gpu";

const tr = useTranscribe();
const gloss = useGlossary();
const mm = useModelManager();
const gpu = useGpuSetting();
const cudaRt = useCudaRuntime();
onMounted(async () => {
  tr.init();
  gloss.load();
  mm.refresh();
  await gpu.load(); // chờ nạp use_gpu trước khi áp default (dưới) — mở lại app đã bật GPU cũng đúng
  applyGpuDefault();
  cudaRt.init();
});

// GPU đang bật + user chưa đổi khỏi mặc định "medium" → chọn turbo (nhanh trên GPU + hợp nội dung
// Việt-Anh lẫn). Tôn trọng lựa chọn khác nếu user đã tự chọn size. Áp cả lúc mở app lẫn lúc bật toggle
// (selectedSize không persist → reset "medium" mỗi lần mount, nên phải áp lại mỗi session).
function applyGpuDefault() {
  if (gpu.useGpu.value && tr.selectedSize.value === "medium") {
    tr.selectedSize.value = "large-v3-turbo";
  }
}
function onToggleGpu() {
  gpu.save();
  applyGpuDefault();
}
// keep-alive: quay lại tab → re-sync trạng thái model (có thể đã tải xong từ tab Ghi âm) + model list.
onActivated(() => {
  tr.refreshModel();
  mm.refresh();
});

// Picker model — catalog dùng chung (asr-models); MLX gated theo nền tảng.
const { sizeOptions, sizeHint } = useAsrModels();

const EXPORT_OPTS = [
  { value: "txt", label: "TXT" },
  { value: "srt", label: "SRT (phụ đề)" },
  { value: "vtt", label: "VTT" },
  { value: "json", label: "JSON" },
];

const base = (p: string) => p.split(/[\\/]/).pop() || p;
const locate = (p: string | null) => {
  if (p) revealItemInDir(p).catch(() => {});
};
async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    /* clipboard không khả dụng */
  }
}
async function removeModel(size: ModelSize) {
  const ok = await ask(`Xóa model ${size}? Sẽ phải tải lại khi dùng.`, {
    title: "Xóa model",
    kind: "warning",
  });
  if (ok) mm.remove(size);
}
</script>

<template>
  <ViCard title="Transcript" class="pa-4">
    <div class="os-form">
      <!-- Picker model: nhanh ↔ chất lượng -->
      <ViSelect
        v-model="tr.selectedSize.value"
        :items="sizeOptions"
        label="Model nhận dạng"
        :disabled="tr.transcribing.value || tr.downloading.value"
        hide-details
      />

      <!-- Toggle GPU (CUDA) — chỉ hiện khi máy có GPU NVIDIA; tải runtime ~1.3GB lần đầu -->
      <template v-if="cudaRt.cudaSupported.value">
        <ViSwitch
          v-model="gpu.useGpu.value"
          label="Dùng GPU (CUDA) — nhanh hơn nhiều, cần tải runtime lần đầu"
          hide-details
          @update:model-value="onToggleGpu"
        />
        <template v-if="gpu.useGpu.value">
          <ViAlert v-if="cudaRt.ready.value" type="success" variant="tonal" density="compact">
            CUDA runtime sẵn sàng.
          </ViAlert>
          <template v-else>
            <ViAlert type="warning" variant="tonal" density="compact">
              Chưa tải CUDA runtime (~1.3GB) — cần tải 1 lần, sau đó dùng offline. Chưa tải thì transcribe vẫn chạy (CPU, chậm hơn).
            </ViAlert>
            <div class="os-row">
              <ViButton color="secondary" :disabled="cudaRt.downloading.value" @click="cudaRt.ensure()">
                <ViIcon icon="IconDownload" start />Tải CUDA runtime
              </ViButton>
              <span v-if="cudaRt.downloading.value" class="os-muted">{{ cudaRt.downloadPct.value }}%</span>
            </div>
            <ViProgressLinear
              v-if="cudaRt.downloading.value"
              :model-value="cudaRt.downloadPct.value"
              :indeterminate="cudaRt.downloadPct.value === 0"
              color="secondary"
              height="6"
              rounded
            />
          </template>
          <ViAlert v-if="cudaRt.error.value" type="error" density="compact" closable>{{ cudaRt.error.value }}</ViAlert>
        </template>
      </template>

      <!-- Glossary: bias model đúng tên riêng/thuật ngữ (dùng chung Mode A + C) -->
      <ViTextarea
        v-model="gloss.glossary.value"
        label="Thuật ngữ / tên riêng (glossary)"
        placeholder="VD: VictoriaLogs, Grafana, tên người, tên sản phẩm… — để trống nếu không cần."
        rows="2"
        auto-grow
        hide-details
        @blur="gloss.save()"
      />

      <!-- Định dạng xuất -->
      <div class="os-row">
        <span class="os-muted">Xuất:</span>
        <ViButtonToggle v-model="tr.exportFormats.value" multiple divided density="comfortable">
          <ViButton v-for="o in EXPORT_OPTS" :key="o.value" :value="o.value" size="small">
            {{ o.label }}
          </ViButton>
        </ViButtonToggle>
      </div>

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
        Model {{ tr.selectedSize.value }} sẵn sàng (chạy offline).
      </ViAlert>
      <template v-else>
        <ViAlert type="warning" variant="tonal" density="compact">
          Chưa có model {{ tr.selectedSize.value }} — cần tải ({{ sizeHint[tr.selectedSize.value] }}) lần đầu, sau đó chạy offline.
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
        <span class="os-muted">
          {{ tr.doneCount.value }}/{{ tr.total.value }} file ·
          <template v-if="tr.indeterminate.value">đang xử lý…</template>
          <template v-else>{{ tr.overallPct.value }}%</template>
          <template v-if="tr.currentFile.value">· {{ base(tr.currentFile.value) }}</template>
        </span>
        <ViProgressLinear
          :model-value="tr.overallPct.value"
          :indeterminate="tr.indeterminate.value"
          color="primary"
          height="6"
          rounded
        />
      </div>

      <ViAlert v-if="tr.error.value" type="error" density="compact" closable>
        {{ tr.error.value }}
      </ViAlert>
      <!-- Lỗi từng file + nút thử lại (Phase 04) -->
      <ViAlert
        v-for="(e, i) in tr.fileErrors.value"
        :key="'fe' + i"
        type="error"
        density="compact"
      >
        <div class="os-row os-err">
          <span>{{ base(e.file) }}: {{ e.message }}</span>
          <ViButton
            variant="text"
            size="small"
            :disabled="tr.transcribing.value"
            @click="tr.retry(e.file)"
          >
            <ViIcon icon="IconRefresh" start />Thử lại
          </ViButton>
        </div>
      </ViAlert>

      <!-- Kết quả per-file: player click-to-seek + editor sửa & lưu (Phase 03) -->
      <div v-for="(r, i) in tr.results.value" :key="'r' + i" class="os-result">
        <div class="os-result-head">
          <ViIcon icon="IconFileText" />
          <span class="os-result-name">{{ base(r.file) }}</span>
          <span v-if="r.saved" class="os-muted">✓ đã lưu</span>
          <ViButton variant="text" size="small" @click="locate(r.txtPath ?? r.file)">
            <ViIcon icon="IconFolder" />
          </ViButton>
        </div>

        <TranscriptPlayer :src="convertFileSrc(r.file)" :is-video="isVideoPath(r.file)" :segments="r.segments" />

        <ViTextarea
          :model-value="r.text"
          label="Nội dung (sửa được)"
          rows="6"
          auto-grow
          hide-details
          @update:model-value="(v: string) => tr.edit(r, v)"
        />
        <div class="os-row">
          <ViButton color="primary" size="small" :disabled="!r.dirty" @click="tr.save(r)">
            <ViIcon icon="IconDeviceFloppy" start />Lưu lại
          </ViButton>
          <ViButton variant="text" size="small" @click="copyText(r.text)">
            <ViIcon icon="IconCopy" start />Copy
          </ViButton>
        </div>
      </div>

      <!-- Quản lý model đã tải (Phase 04) -->
      <details class="os-models">
        <summary class="os-muted">Quản lý model đã tải</summary>
        <ViList density="compact" class="os-box">
          <ViListItem
            v-for="m in mm.models.value"
            :key="m.size"
            :title="m.size"
            :subtitle="m.present ? formatBytes(m.bytes) + ' · đã tải' : 'chưa tải'"
          >
            <template #append>
              <ViButton
                v-if="m.present"
                variant="text"
                size="small"
                color="error"
                :disabled="tr.transcribing.value || tr.downloading.value"
                @click="removeModel(m.size)"
              >
                <ViIcon icon="IconTrash" />
              </ViButton>
            </template>
          </ViListItem>
        </ViList>
      </details>
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
.os-err {
  justify-content: space-between;
  width: 100%;
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
.os-result {
  display: flex;
  flex-direction: column;
  gap: var(--viui-space-sm);
  padding: var(--viui-space-sm);
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: var(--viui-radius-sm, 8px);
}
.os-result-head {
  display: flex;
  align-items: center;
  gap: var(--viui-space-sm);
}
.os-result-name {
  flex: 1 1 auto;
  word-break: break-all;
  font-size: var(--viui-font-size-sm);
}
.os-models > summary {
  cursor: pointer;
}
</style>
