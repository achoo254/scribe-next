<script setup lang="ts">
// Tab "Ghi âm" (Mode A) — chọn nguồn (mic / màn hình+system) → Ghi/Dừng → upload streaming →
// lịch sử + nghe lại. Tùy chọn cắt theo phút sau khi ghi (tái dùng job queue = useSplitter).
import { ref, computed, watch, onBeforeUnmount } from "vue";
import { ViCard, ViButton, ViIcon, ViInput, ViSwitch, ViAlert, ViProgressLinear } from "@hobui/viui";
import { useRecorder, supportsScreenCapture } from "../composables/use-recorder";
import { useSplitter } from "../composables/use-splitter";
import { useUsage } from "../composables/use-usage";
import RecordingHistory from "../components/RecordingHistory.vue";

const rec = useRecorder();
const sp = useSplitter();
const usage = useUsage();

// 3 nguồn ghi bật/tắt độc lập (giống desktop tách mic / tiếng hệ thống / màn hình). Mặc định mic.
const micEnabled = ref(true);
const systemAudioEnabled = ref(false);
const screenEnabled = ref(false);
const splitEnabled = ref(false);
const splitMinutes = ref(10);
const canScreen = supportsScreenCapture();

// Khoá đổi nguồn khi đang ghi/lưu; ≥1 nguồn phải bật; tiếng hệ thống & màn hình cần getDisplayMedia.
const locked = computed(() => rec.recording.value || rec.finalizing.value);
const noSource = computed(() => !micEnabled.value && !systemAudioEnabled.value && !screenEnabled.value);
const needsScreen = computed(() => systemAudioEnabled.value || screenEnabled.value);
const startDisabled = computed(() => noSource.value || (needsScreen.value && !canScreen));

const historyRef = ref<InstanceType<typeof RecordingHistory> | null>(null);

// Giám sát realtime khi đang ghi: preview màn hình + mức âm + thông tin nguồn.
const previewEl = ref<HTMLVideoElement | null>(null);
const sourceInfo = ref("");

const elapsedLabel = computed(() => {
  const t = rec.elapsedS.value;
  const m = Math.floor(t / 60);
  const s = String(t % 60).padStart(2, "0");
  return `${m}:${s}`;
});

// RMS thường thấp (0.05–0.3) → khuếch đại ×250 cho bar dễ đọc, chặn trần 100%.
const meterPct = computed(() => Math.min(100, Math.round(rec.audioLevel.value * 250)));
const meterColor = computed(() => {
  const p = meterPct.value;
  if (p >= 90) return "rgb(var(--v-theme-error))"; // gần clip
  if (p >= 70) return "rgb(var(--v-theme-secondary))"; // cam cảnh báo
  return "rgb(var(--v-theme-primary))"; // xanh bình thường
});

function surfaceLabel(s: string | undefined): string {
  switch (s) {
    case "monitor":
      return "Toàn màn hình";
    case "window":
      return "Cửa sổ";
    case "browser":
      return "Tab trình duyệt";
    default:
      return "";
  }
}

// Gắn stream vào <video> preview + đọc thông tin nguồn khi bắt đầu/kết thúc ghi.
// flush:post → khối .os-live (v-if) đã mount xong nên previewEl có giá trị khi callback chạy.
watch(
  () => rec.previewStream.value,
  (s) => {
    if (previewEl.value) previewEl.value.srcObject = s;
    if (!s) {
      sourceInfo.value = "";
      return;
    }
    const st = s.getVideoTracks()[0]?.getSettings() as
      | (MediaTrackSettings & { displaySurface?: string })
      | undefined;
    if (!st) {
      sourceInfo.value = "";
      return;
    }
    const parts = [surfaceLabel(st.displaySurface)];
    if (st.width && st.height) parts.push(`${st.width}×${st.height}`);
    if (st.frameRate) parts.push(`${Math.round(st.frameRate)}fps`);
    sourceInfo.value = parts.filter(Boolean).join(" · ");
  },
  { flush: "post" },
);

const jobBusy = (): boolean => sp.status.value === "queued" || sp.status.value === "running";

async function toggle(): Promise<void> {
  if (rec.recording.value) {
    const result = await rec.stop();
    if (result) {
      await historyRef.value?.refresh();
      void usage.refresh(); // dung lượng tăng sau khi lưu bản ghi
      if (splitEnabled.value) {
        await sp.createJob(result.fileId, "minutes", splitMinutes.value);
      }
    }
  } else {
    sp.reset(); // xoá job/segments của lần cắt trước để UI không hiện kết quả cũ khi đang ghi mới
    await rec.start({
      mic: micEnabled.value,
      systemAudio: systemAudioEnabled.value,
      screen: screenEnabled.value,
    }); // lỗi đã set rec.error (start không throw)
  }
}

function onDownload(): void {
  if (sp.jobId.value) sp.download(sp.jobId.value);
}

// Đoạn cắt tính vào quota → refresh khi job cắt XONG (createJob resolve lúc queued, chưa có output).
watch(() => sp.status.value, (s) => { if (s === "done") void usage.refresh(); });

// Dọn mic/màn hình + timer nếu component bị huỷ giữa lúc đang ghi (phòng thủ; keep-alive chỉ deactivate).
onBeforeUnmount(() => rec.dispose());
</script>

<template>
  <div class="os-record">
    <ViCard title="Ghi âm" class="os-rec-card">
      <div class="os-form">
        <!-- Nguồn ghi — 3 toggle độc lập (khóa khi đang ghi); chọn nhiều → trộn chung 1 bản ghi -->
        <div class="os-sources">
          <span class="os-sources-label">Nguồn</span>
          <ViSwitch v-model="micEnabled" label="Micro" :disabled="locked" hide-details />
          <ViSwitch v-model="systemAudioEnabled" label="Tiếng hệ thống" :disabled="locked" hide-details />
          <ViSwitch v-model="screenEnabled" label="Ghi kèm màn hình (video)" :disabled="locked" hide-details />
        </div>

        <ViAlert v-if="noSource" type="warning" variant="tonal" density="compact">
          Hãy bật ít nhất một nguồn ghi.
        </ViAlert>

        <ViAlert type="info" variant="tonal" density="compact">
          Tiếng hệ thống và ghi màn hình cần chia sẻ màn hình — tốt nhất trên <strong>Chrome/Edge</strong>:
          chọn “Toàn màn hình” và bật “Chia sẻ âm thanh” để có tiếng hệ thống. Firefox/Safari có thể
          không hỗ trợ tiếng hệ thống. Chọn nhiều nguồn → trộn chung vào 1 bản ghi.
          <template v-if="!canScreen"><br />Trình duyệt này không hỗ trợ chia sẻ màn hình (chỉ ghi được mic).</template>
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

        <!-- Giám sát realtime: preview (chỉ màn hình) + mức âm thanh + thông tin nguồn -->
        <div v-if="rec.recording.value" class="os-live">
          <video
            v-show="screenEnabled"
            ref="previewEl"
            class="os-live-video"
            muted
            autoplay
            playsinline
          ></video>
          <div class="os-meter">
            <ViIcon icon="microphone" />
            <div class="os-meter-track">
              <div class="os-meter-fill" :style="{ width: meterPct + '%', background: meterColor }" />
            </div>
          </div>
          <p v-if="sourceInfo" class="os-muted os-live-info">{{ sourceInfo }}</p>
        </div>

        <!-- Nút Ghi/Dừng -->
        <ViButton
          :color="rec.recording.value ? 'error' : 'primary'"
          size="large"
          :loading="rec.finalizing.value || rec.starting.value"
          :disabled="!rec.recording.value && startDisabled"
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
.os-sources {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}
.os-sources-label {
  font-size: 0.875rem;
  color: rgb(var(--v-theme-on-surface));
  opacity: 0.75;
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
.os-live {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  padding: 0.75rem;
  border: 1px solid rgb(var(--v-theme-surface-bright));
  border-radius: 8px;
  background: rgba(var(--v-theme-surface), 0.5);
}
.os-live-video {
  width: 100%;
  max-height: 280px;
  object-fit: contain;
  background: #000;
  border-radius: 6px;
}
.os-meter {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}
.os-meter-track {
  flex: 1;
  height: 10px;
  border-radius: 999px;
  background: rgb(var(--v-theme-surface-bright));
  overflow: hidden;
}
.os-meter-fill {
  height: 100%;
  border-radius: 999px;
  transition: width 80ms linear;
}
.os-live-info {
  margin: 0;
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
