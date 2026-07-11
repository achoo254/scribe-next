<script setup lang="ts">
import { onUnmounted, ref } from "vue";
import { useRouter } from "vue-router";
import { ViCard, ViButton, ViSelect, ViInput, ViProgressLinear, ViAlert } from "@hobui/viui";
import { useUpload } from "../composables/use-upload";
import { useSplitter, type JobCriterion } from "../composables/use-splitter";
import AppTopBar from "../components/AppTopBar.vue";

const router = useRouter();

const {
  uploading,
  progress: uploadProgress,
  error: uploadError,
  result: uploadResult,
  upload,
} = useUpload();

const {
  jobId,
  status: jobStatus,
  segments,
  error: jobError,
  creating,
  createJob,
  download,
  stopPolling,
} = useSplitter();

const fileInput = ref<HTMLInputElement | null>(null);
const fileName = ref("");
const criterion = ref<JobCriterion>("minutes");
const value = ref(10);

const criterionOptions: { value: JobCriterion; title: string }[] = [
  { value: "minutes", title: "Phút / đoạn" },
  { value: "megabytes", title: "MB / đoạn" },
  { value: "parts", title: "Số phần" },
];

const jobBusy = (): boolean => jobStatus.value === "queued" || jobStatus.value === "running";

function pickFile(): void {
  fileInput.value?.click();
}

async function onFileChange(e: Event): Promise<void> {
  const input = e.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  fileName.value = file.name;
  stopPolling();
  await upload(file);
}

async function onSplit(): Promise<void> {
  if (!uploadResult.value) return;
  await createJob(uploadResult.value.fileId, criterion.value, value.value);
}

function onDownload(): void {
  if (jobId.value) download(jobId.value);
}

function formatDuration(seconds: number): string {
  const total = Math.round(seconds);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const pad = (n: number): string => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
}

onUnmounted(() => {
  stopPolling();
});
</script>

<template>
  <div>
    <AppTopBar />
    <main class="content">
      <ViButton
        variant="text"
        color="primary"
        size="small"
        label="← Về trang chủ"
        class="mb-2"
        @click="router.push({ name: 'home' })"
      />
      <h1 class="text-h5 font-weight-bold mb-4">Cắt file</h1>

      <!-- Bước 1: chọn + upload file -->
      <ViCard rounded="lg" class="mb-4 section">
        <input
          ref="fileInput"
          type="file"
          accept="audio/*,video/*"
          class="d-none"
          @change="onFileChange"
        />
        <ViButton
          color="primary"
          variant="outlined"
          :disabled="uploading"
          label="Chọn file audio/video"
          @click="pickFile"
        />
        <p v-if="fileName" class="text-body-2 mt-3 mb-0">Tệp: <strong>{{ fileName }}</strong></p>
        <ViProgressLinear
          v-if="uploading"
          :model-value="uploadProgress"
          color="primary"
          height="8"
          rounded
          class="mt-3"
        />
        <p v-if="uploading" class="text-caption text-medium-emphasis mt-1 mb-0">
          Đang tải lên… {{ uploadProgress }}%
        </p>
        <ViAlert
          v-if="uploadError"
          type="error"
          variant="tonal"
          density="compact"
          class="mt-3"
          :text="uploadError"
        />
        <ViAlert
          v-if="uploadResult"
          type="success"
          variant="tonal"
          density="compact"
          class="mt-3"
          :text="`Đã tải lên — thời lượng ${formatDuration(uploadResult.durationS)}`"
        />
      </ViCard>

      <!-- Bước 2: tiêu chí + cắt -->
      <ViCard v-if="uploadResult" rounded="lg" class="mb-4 section">
        <h2 class="text-subtitle-1 font-weight-bold mb-3">Tiêu chí cắt</h2>
        <ViSelect
          v-model="criterion"
          :items="criterionOptions"
          item-title="title"
          item-value="value"
          label="Cắt theo"
          density="comfortable"
        />
        <ViInput
          v-model.number="value"
          type="number"
          min="1"
          label="Giá trị"
          density="comfortable"
          class="mt-2"
        />
        <ViButton
          color="primary"
          variant="flat"
          :loading="creating || jobBusy()"
          :disabled="uploading"
          label="Cắt"
          class="mt-2"
          @click="onSplit"
        />
        <ViProgressLinear v-if="jobBusy()" indeterminate color="primary" height="8" rounded class="mt-3" />
        <p v-if="jobBusy()" class="text-caption text-medium-emphasis mt-1 mb-0">Đang xử lý…</p>
        <ViAlert
          v-if="jobError"
          type="error"
          variant="tonal"
          density="compact"
          class="mt-3"
          :text="jobError"
        />
      </ViCard>

      <!-- Bước 3: kết quả -->
      <ViCard v-if="jobStatus === 'done'" rounded="lg" class="section">
        <h2 class="text-subtitle-1 font-weight-bold mb-3">Kết quả ({{ segments.length }} đoạn)</h2>
        <ul class="segments">
          <li v-for="seg in segments" :key="seg.fileId">{{ seg.name }}</li>
        </ul>
        <ViButton color="primary" variant="flat" label="Tải zip" class="mt-3" @click="onDownload" />
      </ViCard>
    </main>
  </div>
</template>

<style scoped>
.content {
  max-width: 640px;
  margin: 0 auto;
  padding: var(--space-8pt-4) var(--space-8pt-3);
}
.section {
  padding: var(--space-8pt-3);
}
.segments {
  margin: 0;
  padding-left: var(--space-8pt-3);
  color: var(--bsg-nl-700, #37474f);
}
.segments li {
  margin-bottom: var(--space-8pt-1);
}
</style>
