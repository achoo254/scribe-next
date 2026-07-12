<script setup lang="ts">
// Tab "Ghi âm" (Mode A) — chọn nguồn (mic / màn hình+system) → Ghi/Dừng → upload streaming →
// lịch sử + nghe lại. Tùy chọn cắt theo phút sau khi ghi (tái dùng job queue = useSplitter).
import { ref, computed, onBeforeUnmount } from "vue";
import { ViCard, ViButton, ViIcon, ViRadioGroup, ViInput, ViSwitch, ViAlert, ViProgressLinear } from "@hobui/viui";
import { useRecorder, supportsScreenCapture, type RecordSource } from "../composables/use-recorder";
import { useSplitter } from "../composables/use-splitter";
import RecordingHistory from "../components/RecordingHistory.vue";

const rec = useRecorder();
const sp = useSplitter();

const source = ref<RecordSource>("mic");
const splitEnabled = ref(false);
const splitMinutes = ref(10);
const canScreen = supportsScreenCapture();

const sourceOptions = [
  { title: "Mic", value: "mic" },
  { title: "Màn hình + tiếng hệ thống", value: "screen" },
];

const historyRef = ref<InstanceType<typeof RecordingHistory> | null>(null);

const elapsedLabel = computed(() => {
  const t = rec.elapsedS.value;
  const m = Math.floor(t / 60);
  const s = String(t % 60).padStart(2, "0");
  return `${m}:${s}`;
});

const jobBusy = (): boolean => sp.status.value === "queued" || sp.status.value === "running";

async function toggle(): Promise<void> {
  if (rec.recording.value) {
    const result = await rec.stop();
    if (result) {
      await historyRef.value?.refresh();
      if (splitEnabled.value) {
        await sp.createJob(result.fileId, "minutes", splitMinutes.value);
      }
    }
  } else {
    sp.reset(); // xoá job/segments của lần cắt trước để UI không hiện kết quả cũ khi đang ghi mới
    await rec.start(source.value); // lỗi đã set rec.error (start không throw)
  }
}

function onDownload(): void {
  if (sp.jobId.value) sp.download(sp.jobId.value);
}

// Dọn mic/màn hình + timer nếu component bị huỷ giữa lúc đang ghi (phòng thủ; keep-alive chỉ deactivate).
onBeforeUnmount(() => rec.dispose());
</script>

<template>
  <div class="os-record">
    <ViCard title="Ghi âm" class="os-rec-card">
      <div class="os-form">
        <!-- Nguồn ghi (khóa khi đang ghi) -->
        <ViRadioGroup
          v-model="source"
          inline
          label="Nguồn"
          :items="sourceOptions"
          :disabled="rec.recording.value || rec.finalizing.value"
        />

        <ViAlert type="info" variant="tonal" density="compact">
          Ghi màn hình + tiếng hệ thống hoạt động tốt nhất trên <strong>Chrome/Edge</strong> — chọn
          “Toàn màn hình” để có tiếng hệ thống. Firefox/Safari chỉ ghi mic.
          <template v-if="!canScreen"><br />Trình duyệt này không hỗ trợ ghi màn hình.</template>
        </ViAlert>

        <!-- Cắt theo phút (tùy chọn) -->
        <div class="os-row">
          <ViSwitch
            v-model="splitEnabled"
            label="Cắt thành đoạn theo phút"
            :disabled="rec.recording.value || rec.finalizing.value"
            hide-details
          />
          <ViInput
            v-if="splitEnabled"
            v-model.number="splitMinutes"
            type="number"
            min="1"
            label="Số phút mỗi đoạn"
            density="comfortable"
            class="os-num"
            :disabled="rec.recording.value || rec.finalizing.value"
          />
        </div>

        <!-- Đồng hồ khi đang ghi -->
        <div v-if="rec.recording.value" class="os-timer">
          <span class="os-dot" /> Đang ghi — {{ elapsedLabel }}
        </div>

        <!-- Nút Ghi/Dừng -->
        <ViButton
          :color="rec.recording.value ? 'error' : 'primary'"
          size="large"
          :loading="rec.finalizing.value || rec.starting.value"
          :disabled="!rec.recording.value && source === 'screen' && !canScreen"
          @click="toggle"
        >
          <ViIcon :icon="rec.recording.value ? 'player-stop' : 'microphone'" start />
          {{ rec.recording.value ? "Dừng ghi" : "Bắt đầu ghi" }}
        </ViButton>

        <p v-if="rec.finalizing.value" class="os-muted">Đang lưu bản ghi…</p>
        <ViAlert v-if="rec.error.value" type="error" variant="tonal" density="compact" :text="rec.error.value" />

        <!-- Tiến độ/kết quả cắt (nếu bật) -->
        <template v-if="splitEnabled">
          <ViProgressLinear v-if="jobBusy()" indeterminate color="primary" height="8" rounded />
          <p v-if="jobBusy()" class="os-muted">Đang cắt bản ghi…</p>
          <ViAlert v-if="sp.error.value" type="error" variant="tonal" density="compact" :text="sp.error.value" />
          <template v-if="sp.status.value === 'done'">
            <p class="os-muted">Đã cắt {{ sp.segments.value.length }} đoạn.</p>
            <ViButton color="primary" variant="flat" @click="onDownload">
              <ViIcon icon="download" start />Tải zip
            </ViButton>
          </template>
        </template>
      </div>
    </ViCard>

    <RecordingHistory ref="historyRef" class="os-hist-block" />
  </div>
</template>

<style scoped>
.os-record {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}
.os-rec-card {
  padding: 1rem;
}
.os-form {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}
.os-row {
  display: flex;
  align-items: center;
  gap: 1rem;
  flex-wrap: wrap;
}
.os-num {
  max-width: 200px;
}
.os-muted {
  font-size: 0.875rem;
  color: rgb(var(--v-theme-on-surface));
  opacity: 0.75;
  margin: 0;
}
.os-timer {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-weight: 600;
  color: rgb(var(--v-theme-error));
}
.os-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: rgb(var(--v-theme-error));
  animation: os-blink 1s steps(2, start) infinite;
}
@keyframes os-blink {
  to {
    visibility: hidden;
  }
}
</style>
