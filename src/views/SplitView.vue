<script setup lang="ts">
// Mode B — Cắt file sẵn có (phút / MB / chia-K) + batch. UI Vi* (iNET ViUI), token-only, WCAG AA.
import { ref, computed, onMounted } from "vue";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  ViCard, ViButton, ViIcon, ViList, ViListItem, ViRadioGroup,
  ViRadio, ViInput, ViProgressLinear, ViAlert,
} from "../design-system/vi-ui";
import { useSplitter, type FileResult } from "../composables/use-splitter";

const sp = useSplitter();
const criterion = ref("minutes");
const value = ref(10);

onMounted(() => sp.init());

const valueLabel = computed(() =>
  criterion.value === "minutes"
    ? "Số phút mỗi đoạn"
    : criterion.value === "megabytes"
      ? "MB mỗi đoạn"
      : "Chia thành (số đoạn)",
);
const base = (p: string) => p.split(/[\\/]/).pop() || p;
const VIDEO_RE = /\.(mp4|mkv|mov|avi|webm|m4v|ts|flv|wmv|mpe?g)$/i;
const hasVideo = computed(() => sp.files.value.some((p) => VIDEO_RE.test(p)));
const canSplit = computed(() => sp.files.value.length > 0 && !!sp.outDir.value && !sp.running.value);
function fmtDur(s?: number): string {
  if (!s || s <= 0) return "";
  const t = Math.round(s);
  const h = Math.floor(t / 3600);
  const m = Math.floor((t % 3600) / 60);
  const ss = (t % 60).toString().padStart(2, "0");
  return h > 0 ? `${h}:${m.toString().padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}

const onSplit = () => sp.split(criterion.value, Number(value.value));
const locate = (path: string) => revealItemInDir(path).catch(() => {});
const openResult = (r: FileResult) => {
  if (r.segments.length) locate(r.segments[0].path);
};
</script>

<template>
  <ViCard title="Cắt file" class="pa-4">
    <div class="os-form">
      <div class="os-row">
        <ViButton variant="outlined" :disabled="sp.running.value" @click="sp.pickFiles()">
          <ViIcon icon="IconFolder" start />Chọn file
        </ViButton>
        <span class="os-muted">{{ sp.files.value.length }} file đã chọn</span>
      </div>
      <ViList v-if="sp.files.value.length" density="compact" class="os-box">
        <ViListItem
          v-for="(p, i) in sp.files.value"
          :key="i"
          :title="base(p)"
          :subtitle="fmtDur(sp.durations.value[p])"
        />
      </ViList>

      <ViRadioGroup v-model="criterion" inline label="Tiêu chí" :disabled="sp.running.value">
        <ViRadio label="Phút" value="minutes" />
        <ViRadio label="MB" value="megabytes" />
        <ViRadio label="Chia đều K" value="parts" />
      </ViRadioGroup>

      <ViInput
        v-model.number="value"
        type="number"
        :label="valueLabel"
        :disabled="sp.running.value"
        min="0.1"
        class="os-num"
      />

      <ViAlert v-if="criterion === 'megabytes'" type="info" variant="tonal" density="compact">
        Mỗi đoạn xấp xỉ kích thước đặt (cắt tại keyframe để giữ nguyên chất lượng, nên có thể chênh nhẹ /
        nhỉnh hơn một chút).
      </ViAlert>

      <ViAlert v-if="hasVideo" type="info" variant="tonal" density="compact">
        File video cắt không re-encode (giữ nguyên chất lượng, nhanh) nên điểm cắt bám theo keyframe —
        độ dài mỗi đoạn có thể lệch nhẹ so với giá trị đặt.
      </ViAlert>

      <div class="os-row">
        <ViButton variant="outlined" :disabled="sp.running.value" @click="sp.pickOutDir()">
          <ViIcon icon="IconFolder" start />Thư mục lưu
        </ViButton>
        <span class="os-muted os-path">{{ sp.outDir.value || "(chưa chọn)" }}</span>
      </div>

      <ViButton color="primary" size="large" :disabled="!canSplit" @click="onSplit">
        <ViIcon icon="IconScissors" start />Cắt
      </ViButton>

      <div v-if="sp.running.value && sp.progress.value" class="os-prog">
        <span class="os-muted">
          {{ sp.progress.value.file_name }} · file {{ sp.progress.value.file_index + 1 }}/{{ sp.progress.value.total }}
        </span>
        <ViProgressLinear :model-value="sp.progress.value.file_pct" color="primary" height="6" rounded />
        <span class="os-muted">Tổng: {{ Math.round(sp.progress.value.overall_pct) }}%</span>
        <ViProgressLinear :model-value="sp.progress.value.overall_pct" color="secondary" height="6" rounded />
      </div>

      <ViAlert v-for="(e, i) in sp.errors.value" :key="i" type="error" density="compact" closable>
        {{ e.file ? base(e.file) + ": " : "" }}{{ e.error }}
      </ViAlert>

      <ViList v-if="sp.results.value.length" density="compact" class="os-box">
        <ViListItem
          v-for="(r, i) in sp.results.value"
          :key="i"
          :title="base(r.file)"
          :subtitle="r.segments.length + ' đoạn'"
        >
          <template #append>
            <ViButton variant="text" size="small" @click="openResult(r)">
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
.os-muted {
  font-size: var(--viui-font-size-sm);
  color: rgb(var(--v-theme-on-surface));
  opacity: 0.75;
}
.os-path {
  word-break: break-all;
}
.os-num {
  max-width: 220px;
}
.os-box {
  max-height: 180px;
  overflow-y: auto;
  background: transparent;
}
.os-prog {
  display: flex;
  flex-direction: column;
  gap: var(--viui-space-xs);
}
</style>
