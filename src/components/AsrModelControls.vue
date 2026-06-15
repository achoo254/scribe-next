<script setup lang="ts">
// Điều khiển model ASR (PhoWhisper): chọn size + trạng thái sẵn sàng/tải + tiến độ + lỗi.
// Presentational — state do parent giữ (composable). Dùng cho tự-động-transcript (Mode A).
import { ViSelect, ViAlert, ViButton, ViIcon, ViProgressLinear } from "../design-system/vi-ui";

const size = defineModel<string>("size", { required: true });
const props = defineProps<{
  sizeOptions: { title: string; value: string }[];
  sizeHint: Record<string, string>;
  modelReady: boolean;
  downloading: boolean;
  downloadPct: number;
  error: string;
  disabled?: boolean;
}>();
defineEmits<{ download: [] }>();
</script>

<template>
  <ViSelect
    v-model="size"
    :items="props.sizeOptions"
    label="Model nhận dạng"
    :disabled="props.downloading || props.disabled"
    hide-details
  />
  <ViAlert v-if="props.modelReady" type="success" variant="tonal" density="compact">
    Model {{ size }} sẵn sàng — transcript chạy ngay sau mỗi đoạn ghi.
  </ViAlert>
  <template v-else>
    <ViAlert type="warning" variant="tonal" density="compact">
      Chưa có model {{ size }} — cần tải ({{ props.sizeHint[size] }}) lần đầu, sau đó chạy offline.
    </ViAlert>
    <div class="os-row">
      <ViButton color="secondary" :disabled="props.downloading" @click="$emit('download')">
        <ViIcon icon="IconDownload" start />Tải model
      </ViButton>
      <span v-if="props.downloading" class="os-muted">{{ props.downloadPct }}%</span>
    </div>
    <ViProgressLinear
      v-if="props.downloading"
      :model-value="props.downloadPct"
      :indeterminate="props.downloadPct === 0"
      color="secondary"
      height="6"
      rounded
    />
  </template>
  <ViAlert v-if="props.error" type="error" density="compact" closable>
    {{ props.error }}
  </ViAlert>
</template>

<style scoped>
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
</style>
