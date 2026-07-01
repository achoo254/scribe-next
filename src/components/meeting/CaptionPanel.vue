<script setup lang="ts">
// Panel caption realtime (Phase 3): danh sách caption kèm tên người nói, tự cuộn cuối, banner trạng
// thái host (transcribe dừng / mic mất / quá tải). Nút lưu transcript (Strategy A — ghép final phía client).
import { nextTick, ref, watch } from "vue";
import { ViButton, ViIcon } from "../../design-system/vi-ui";
import type { CaptionLine, HostStatus } from "@onescribe/meeting-shared";

const props = defineProps<{ captions: CaptionLine[]; hostStatus: HostStatus }>();
const emit = defineEmits<{ save: [] }>();

const scroller = ref<HTMLElement | null>(null);

// Tự cuộn xuống caption mới nhất khi có dòng mới (chỉ khi user đang ở gần đáy → không cướp cuộn).
watch(
  () => props.captions.length,
  async () => {
    const el = scroller.value;
    if (!el) return;
    const nearBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
    await nextTick();
    if (nearBottom) el.scrollTop = el.scrollHeight;
  },
);

const STATUS_TEXT: Record<Exclude<HostStatus, "ok">, string> = {
  transcribe_down: "Transcribe tạm dừng (sidecar khởi động lại)",
  mic_lost: "Mic host bị thu hồi — caption tạm dừng",
  overloaded: "Máy host quá tải — caption có thể trễ/bỏ bớt",
};
</script>

<template>
  <div class="caption-panel">
    <div class="caption-panel__head">
      <span class="caption-panel__title"><ViIcon icon="IconMessageChatbot" start />Phụ đề trực tiếp</span>
      <ViButton
        v-if="captions.length"
        size="small"
        variant="tonal"
        color="secondary"
        @click="emit('save')"
      >
        <ViIcon icon="IconDownload" start />Lưu
      </ViButton>
    </div>

    <div v-if="hostStatus !== 'ok'" class="caption-panel__status" :data-kind="hostStatus">
      <ViIcon icon="IconAlertTriangle" start />{{ STATUS_TEXT[hostStatus] }}
    </div>

    <div ref="scroller" class="caption-panel__list">
      <p v-if="!captions.length" class="caption-panel__empty">Chưa có phụ đề. Bắt đầu nói…</p>
      <div v-for="c in captions" :key="c.id" class="caption-line" :class="{ 'caption-line--partial': !c.final }">
        <span class="caption-line__name">{{ c.name }}</span>
        <span class="caption-line__text">{{ c.text }}</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.caption-panel {
  display: flex;
  flex-direction: column;
  gap: var(--viui-space-xs);
  height: 100%;
  min-height: 0;
}
.caption-panel__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--viui-space-sm);
}
.caption-panel__title {
  font-size: var(--viui-font-size-sm);
  font-weight: 600;
}
.caption-panel__status {
  display: flex;
  align-items: center;
  gap: var(--viui-space-xs);
  font-size: var(--viui-font-size-sm);
  padding: var(--viui-space-xs) var(--viui-space-sm);
  border-radius: var(--viui-radius-md, 8px);
  color: rgb(var(--v-theme-warning));
  background: rgba(var(--v-theme-warning), 0.12);
}
.caption-panel__status[data-kind="transcribe_down"],
.caption-panel__status[data-kind="mic_lost"] {
  color: rgb(var(--v-theme-error));
  background: rgba(var(--v-theme-error), 0.12);
}
.caption-panel__list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: var(--viui-space-xs);
  padding-right: var(--viui-space-xs);
}
.caption-panel__empty {
  opacity: 0.6;
  font-size: var(--viui-font-size-sm);
}
.caption-line {
  font-size: var(--viui-font-size-sm);
  line-height: 1.4;
}
.caption-line--partial {
  opacity: 0.7;
  font-style: italic;
}
.caption-line__name {
  font-weight: 600;
  margin-right: var(--viui-space-xs);
  color: rgb(var(--v-theme-primary));
}
</style>
