<script setup lang="ts">
// Danh sách đoạn ghi (Mode A) + transcript inline khi bật "Tự động transcript".
// Thuần presentational: nhận segments + map transcript theo path; phát sự kiện mở thư mục/copy.
import { ViButton, ViIcon, ViTextarea, ViProgressLinear, ViAlert } from "../design-system/vi-ui";
import type { SegmentFile } from "../composables/use-recorder";
import type { SegTranscript } from "../composables/use-record-transcribe";

const props = defineProps<{
  segments: SegmentFile[];
  transcripts: Record<string, SegTranscript>;
  showTranscript: boolean;
}>();
defineEmits<{ locate: [path: string] }>();

const statusLabel = (t: SegTranscript): string => {
  switch (t.status) {
    case "pending": return "Chờ transcript…";
    case "transcribing": return `Đang nhận dạng ${t.pct}%`;
    case "done": return "Transcript xong";
    case "error": return "Lỗi transcript";
    default: return "";
  }
};

async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    /* clipboard không khả dụng */
  }
}
</script>

<template>
  <div class="os-segs">
    <div v-for="(seg, i) in props.segments" :key="seg.name" class="os-seg">
      <div class="os-seg-head">
        <span class="os-seg-idx">{{ i + 1 }}</span>
        <span class="os-seg-name">{{ seg.name }}</span>
        <span v-if="props.showTranscript && props.transcripts[seg.path]" class="os-seg-status">
          {{ statusLabel(props.transcripts[seg.path]) }}
        </span>
        <ViButton variant="text" size="small" @click="$emit('locate', seg.path)">
          <ViIcon icon="IconFolder" />
        </ViButton>
      </div>

      <template v-if="props.showTranscript && props.transcripts[seg.path]">
        <ViProgressLinear
          v-if="props.transcripts[seg.path].status === 'transcribing'"
          :model-value="props.transcripts[seg.path].pct"
          color="primary"
          height="4"
          rounded
        />
        <ViAlert
          v-else-if="props.transcripts[seg.path].status === 'error'"
          type="error"
          density="compact"
          variant="tonal"
        >
          {{ props.transcripts[seg.path].error }}
        </ViAlert>
        <template v-else-if="props.transcripts[seg.path].text">
          <ViTextarea
            :model-value="props.transcripts[seg.path].text"
            label="Nội dung transcript"
            readonly
            rows="4"
            auto-grow
            hide-details
          />
          <div class="os-seg-actions">
            <ViButton variant="text" size="small" @click="copyText(props.transcripts[seg.path].text)">
              <ViIcon icon="IconCopy" start />Copy
            </ViButton>
          </div>
        </template>
      </template>
    </div>
  </div>
</template>

<style scoped>
.os-segs {
  display: flex;
  flex-direction: column;
  gap: var(--viui-space-sm);
  max-height: 420px;
  overflow-y: auto;
}
.os-seg {
  display: flex;
  flex-direction: column;
  gap: var(--viui-space-xs);
  padding: var(--viui-space-sm);
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: var(--viui-radius-sm, 8px);
}
.os-seg-head {
  display: flex;
  align-items: center;
  gap: var(--viui-space-sm);
}
.os-seg-idx {
  flex: 0 0 auto;
  min-width: 1.5rem;
  height: 1.5rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  font-size: var(--viui-font-size-sm);
  background: rgb(var(--v-theme-primary));
  color: rgb(var(--v-theme-on-primary));
}
.os-seg-name {
  flex: 1 1 auto;
  word-break: break-all;
  font-size: var(--viui-font-size-sm);
}
.os-seg-status {
  flex: 0 0 auto;
  font-size: var(--viui-font-size-sm);
  color: rgb(var(--v-theme-on-surface));
  opacity: 0.75;
}
.os-seg-actions {
  display: flex;
  justify-content: flex-end;
}
</style>
