<script setup lang="ts">
// Mode A — Ghi âm micro + split live (+ tùy chọn tự động transcript). UI Vi* (iNET ViUI), token-only, WCAG AA.
import { ref, computed, onMounted, onActivated, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  ViCard, ViSelect, ViInput, ViButton, ViButtonToggle,
  ViRadioGroup, ViRadio, ViIcon, ViAlert, ViSwitch,
} from "../design-system/vi-ui";
import LevelMeter from "../components/LevelMeter.vue";
import SegmentTranscriptList from "../components/SegmentTranscriptList.vue";
import AsrModelControls from "../components/AsrModelControls.vue";
import { useRecorder } from "../composables/use-recorder";
import { useRecordTranscribe } from "../composables/use-record-transcribe";

const rec = useRecorder();
const rt = useRecordTranscribe();
const recSource = ref<"mic" | "system">("mic"); // nguồn ghi: mic | tiếng hệ thống (output)
const mic = ref<string | null>(null);
const outputId = ref<string | null>(null); // render device khi ghi tiếng hệ thống
const outDir = ref("");
const format = ref("mp3");
const splitValue = ref(10);
const splitUnit = ref("minutes");
const elapsed = ref(0);
let timer: ReturnType<typeof setInterval> | undefined;
let loaded = false;

// Picker model transcript: nhanh ↔ chất lượng (mỗi size tải riêng).
const sizeOptions = [
  { title: "Small — nhanh nhất, chất lượng khá", value: "small" },
  { title: "Medium — cân bằng (mặc định)", value: "medium" },
  { title: "Large — chậm nhất, chất lượng cao nhất", value: "large" },
];
const sizeHint: Record<string, string> = { small: "~0.5GB", medium: "~1.5GB", large: "~3GB" };

const isIdle = computed(() => rec.state.value === "idle");
const isRecording = computed(() => rec.state.value === "recording");
const isPaused = computed(() => rec.state.value === "paused");
const micItems = computed(() => rec.devices.value.map((d) => ({ title: d.name, value: d.id })));
const outputItems = computed(() => rec.outputDevices.value.map((d) => ({ title: d.name, value: d.id })));
const isSystem = computed(() => recSource.value === "system");
const deviceItems = computed(() => (isSystem.value ? outputItems.value : micItems.value));
// Select thiết bị dùng chung — đọc/ghi mic hoặc outputId tùy nguồn.
const selectedDevice = computed({
  get: () => (isSystem.value ? outputId.value : mic.value),
  set: (v: string | null) => {
    if (isSystem.value) outputId.value = v;
    else mic.value = v;
  },
});
const deviceLabel = computed(() => (isSystem.value ? "Thiết bị output" : "Micro"));
const deviceHint = computed(() => (isSystem.value ? "Bỏ trống = thiết bị mặc định" : "Bỏ trống = mic mặc định"));
const clock = computed(() => {
  const m = Math.floor(elapsed.value / 60).toString().padStart(2, "0");
  const s = (elapsed.value % 60).toString().padStart(2, "0");
  return `${m}:${s}`;
});

onMounted(async () => {
  await rec.init();
  await rt.init();
  try {
    const s = await invoke<Record<string, unknown>>("get_settings");
    mic.value = (s.mic_id as string) ?? null;
    outDir.value = (s.out_dir as string) ?? "";
    format.value = (s.format as string) ?? "mp3";
    splitValue.value = (s.split_value as number) ?? 10;
    splitUnit.value = (s.split_unit as string) ?? "minutes";
    recSource.value = (s.source as "mic" | "system") ?? "mic";
    outputId.value = (s.output_id as string) ?? null;
  } catch {
    /* dùng mặc định */
  }
  loaded = true;
});

// keep-alive: quay lại tab → re-sync trạng thái model (có thể đã tải xong từ tab Transcript).
onActivated(() => rt.refreshModel());

watch([mic, outputId, recSource, outDir, format, splitValue, splitUnit], () => {
  if (!loaded) return;
  invoke("set_settings", {
    settings: {
      mic_id: mic.value,
      out_dir: outDir.value || null,
      format: format.value,
      split_value: Number(splitValue.value),
      split_unit: splitUnit.value,
      source: recSource.value,
      output_id: outputId.value || null,
    },
  }).catch(() => {});
});

watch(
  () => rec.state.value,
  (s) => {
    // Đồng hồ chỉ chạy khi đang ghi; tạm dừng → đóng băng (giữ giá trị); dừng hẳn → reset.
    if (s === "recording") {
      if (!timer) timer = setInterval(() => elapsed.value++, 1000);
    } else {
      if (timer) {
        clearInterval(timer);
        timer = undefined;
      }
      if (s === "idle") elapsed.value = 0;
    }
  },
);

// Đẩy các đoạn ĐÃ finalize vào transcribe. Khi đang ghi, đoạn cuối còn ghi dở → bỏ qua;
// khi đã dừng (idle) thì đoạn cuối cũng xong → enqueue hết. enqueue tự dedup + gate enabled/model.
function flushFinalized() {
  if (!rt.enabled.value || !rt.modelReady.value) return;
  const segs = rec.segments.value;
  const upto = isIdle.value ? segs.length : segs.length - 1;
  for (let i = 0; i < upto; i++) rt.enqueue(segs[i].path);
}
watch(() => rec.segments.value.length, flushFinalized);
watch(() => rec.state.value, (s) => { if (s === "idle") flushFinalized(); });
watch(() => rt.modelReady.value, flushFinalized);
watch(() => rt.enabled.value, flushFinalized);

async function chooseFolder() {
  const d = await rec.pickFolder();
  if (d) outDir.value = d;
}
async function onStart() {
  if (!outDir.value) {
    rec.error.value = "Hãy chọn thư mục lưu trước.";
    return;
  }
  rt.reset();
  await rec.start({
    source: recSource.value,
    device: selectedDevice.value,
    outDir: outDir.value,
    format: format.value,
    splitValue: Number(splitValue.value),
    splitUnit: splitUnit.value,
  });
}
const locate = (path: string) => revealItemInDir(path).catch(() => {});
</script>

<template>
  <ViCard title="Ghi âm" class="pa-4">
    <div class="os-form">
      <ViButtonToggle v-model="recSource" mandatory :disabled="!isIdle" density="comfortable">
        <ViButton value="mic"><ViIcon icon="IconMicrophone" start />Mic</ViButton>
        <ViButton value="system"><ViIcon icon="IconVolume" start />Tiếng hệ thống</ViButton>
      </ViButtonToggle>

      <ViSelect
        v-model="selectedDevice"
        :items="deviceItems"
        :label="deviceLabel"
        :disabled="!isIdle"
        clearable
        :hint="deviceHint"
        persistent-hint
      />

      <div class="os-row">
        <ViButton variant="outlined" :disabled="!isIdle" @click="chooseFolder">
          <ViIcon icon="IconFolder" start />Chọn thư mục
        </ViButton>
        <span class="os-path">{{ outDir || "(chưa chọn)" }}</span>
      </div>

      <div class="os-row">
        <ViInput
          v-model.number="splitValue"
          type="number"
          label="Độ dài mỗi đoạn"
          :disabled="!isIdle"
          min="0.1"
          class="os-num"
        />
        <ViButtonToggle v-model="splitUnit" mandatory :disabled="!isIdle" density="comfortable">
          <ViButton value="minutes">Phút</ViButton>
          <ViButton value="megabytes">MB</ViButton>
        </ViButtonToggle>
      </div>

      <ViRadioGroup v-model="format" inline label="Định dạng" :disabled="!isIdle">
        <ViRadio label="MP3 (khuyến nghị)" value="mp3" />
        <ViRadio label="M4A-AAC" value="m4a" />
      </ViRadioGroup>

      <!-- Tùy chọn tự động transcript khi ghi -->
      <div class="os-asr">
        <ViSwitch v-model="rt.enabled.value" label="Tự động transcript khi ghi" />
        <AsrModelControls
          v-if="rt.enabled.value"
          v-model:size="rt.selectedSize.value"
          :size-options="sizeOptions"
          :size-hint="sizeHint"
          :model-ready="rt.modelReady.value"
          :downloading="rt.downloading.value"
          :download-pct="rt.downloadPct.value"
          :error="rt.error.value"
          :disabled="!isIdle"
          @download="rt.ensureModel()"
        />
      </div>

      <div class="os-row os-controls">
        <ViButton v-if="!isRecording && !isPaused" color="primary" size="large" @click="onStart">
          <ViIcon icon="IconMicrophone" start />Ghi
        </ViButton>
        <ViButton v-if="isRecording" color="secondary" @click="rec.pause()">
          <ViIcon icon="IconPlayerPause" start />Tạm dừng
        </ViButton>
        <ViButton v-if="isPaused" color="primary" @click="rec.resume()">
          <ViIcon icon="IconPlayerPlay" start />Tiếp tục
        </ViButton>
        <ViButton v-if="!isIdle" color="error" variant="outlined" @click="rec.stop()">
          <ViIcon icon="IconPlayerStop" start />Dừng
        </ViButton>
        <span class="os-clock">{{ clock }} · {{ rec.segments.value.length }} đoạn</span>
      </div>

      <LevelMeter :rms="rec.level.value.rms" :peak="rec.level.value.peak" />

      <ViAlert v-if="rec.error.value" type="error" density="compact" closable>
        {{ rec.error.value }}
      </ViAlert>

      <SegmentTranscriptList
        v-if="rec.segments.value.length"
        :segments="rec.segments.value"
        :transcripts="rt.transcripts.value"
        :show-transcript="rt.enabled.value"
        @locate="locate"
      />
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
.os-path {
  font-size: var(--viui-font-size-sm);
  color: rgb(var(--v-theme-on-surface));
  opacity: 0.75;
  word-break: break-all;
}
.os-muted {
  font-size: var(--viui-font-size-sm);
  color: rgb(var(--v-theme-on-surface));
  opacity: 0.75;
}
.os-num {
  max-width: 180px;
}
.os-asr {
  display: flex;
  flex-direction: column;
  gap: var(--viui-space-sm);
  padding: var(--viui-space-sm);
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: var(--viui-radius-sm, 8px);
}
.os-controls {
  margin-top: var(--viui-space-sm);
}
.os-clock {
  font-size: var(--viui-font-size-lg);
  font-variant-numeric: tabular-nums;
  margin-inline-start: var(--viui-space-sm);
}
</style>
