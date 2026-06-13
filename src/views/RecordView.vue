<script setup lang="ts">
// Mode A — Ghi âm micro + split live. UI dùng Vi* (iNET ViUI), token-only, WCAG AA.
import { ref, computed, onMounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  ViCard, ViSelect, ViInput, ViButton, ViButtonToggle,
  ViRadioGroup, ViRadio, ViIcon, ViList, ViListItem, ViAlert,
} from "../design-system/vi-ui";
import LevelMeter from "../components/LevelMeter.vue";
import { useRecorder } from "../composables/use-recorder";

const rec = useRecorder();
const mic = ref<string | null>(null);
const outDir = ref("");
const format = ref("mp3");
const splitValue = ref(10);
const splitUnit = ref("minutes");
const elapsed = ref(0);
let timer: ReturnType<typeof setInterval> | undefined;
let loaded = false;

const isIdle = computed(() => rec.state.value === "idle");
const isRecording = computed(() => rec.state.value === "recording");
const isPaused = computed(() => rec.state.value === "paused");
const micItems = computed(() => rec.devices.value.map((d) => ({ title: d.name, value: d.id })));
const clock = computed(() => {
  const m = Math.floor(elapsed.value / 60).toString().padStart(2, "0");
  const s = (elapsed.value % 60).toString().padStart(2, "0");
  return `${m}:${s}`;
});

onMounted(async () => {
  await rec.init();
  try {
    const s = await invoke<Record<string, unknown>>("get_settings");
    mic.value = (s.mic_id as string) ?? null;
    outDir.value = (s.out_dir as string) ?? "";
    format.value = (s.format as string) ?? "mp3";
    splitValue.value = (s.split_value as number) ?? 10;
    splitUnit.value = (s.split_unit as string) ?? "minutes";
  } catch {
    /* dùng mặc định */
  }
  loaded = true;
});

watch([mic, outDir, format, splitValue, splitUnit], () => {
  if (!loaded) return;
  invoke("set_settings", {
    settings: {
      mic_id: mic.value,
      out_dir: outDir.value || null,
      format: format.value,
      split_value: Number(splitValue.value),
      split_unit: splitUnit.value,
    },
  }).catch(() => {});
});

watch(
  () => rec.state.value,
  (s) => {
    if (s === "recording" && !timer) timer = setInterval(() => elapsed.value++, 1000);
    if (s === "idle") {
      if (timer) clearInterval(timer);
      timer = undefined;
      elapsed.value = 0;
    }
  },
);

async function chooseFolder() {
  const d = await rec.pickFolder();
  if (d) outDir.value = d;
}
async function onStart() {
  if (!outDir.value) {
    rec.error.value = "Hãy chọn thư mục lưu trước.";
    return;
  }
  await rec.start({
    mic: mic.value,
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
      <ViSelect
        v-model="mic"
        :items="micItems"
        label="Micro"
        :disabled="!isIdle"
        clearable
        hint="Bỏ trống = mic mặc định"
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

      <ViList v-if="rec.segments.value.length" density="compact" class="os-segs">
        <ViListItem
          v-for="seg in rec.segments.value"
          :key="seg.name"
          :title="seg.name"
        >
          <template #append>
            <ViButton variant="text" size="small" @click="locate(seg.path)">
              <ViIcon icon="IconFolder" />
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
.os-path {
  font-size: var(--viui-font-size-sm);
  color: rgb(var(--v-theme-on-surface));
  opacity: 0.75;
  word-break: break-all;
}
.os-num {
  max-width: 180px;
}
.os-controls {
  margin-top: var(--viui-space-sm);
}
.os-clock {
  font-size: var(--viui-font-size-lg);
  font-variant-numeric: tabular-nums;
  margin-inline-start: var(--viui-space-sm);
}
.os-segs {
  max-height: 220px;
  overflow-y: auto;
  background: transparent;
}
</style>
