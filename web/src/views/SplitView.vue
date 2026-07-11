<script setup lang="ts">
import { onUnmounted, ref } from "vue";
import { useRouter } from "vue-router";
import { useAuth } from "../composables/use-auth";
import { useUpload } from "../composables/use-upload";
import { useSplitter, type JobCriterion } from "../composables/use-splitter";

const { user, logout } = useAuth();
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
  progress: jobProgress,
  segments,
  error: jobError,
  creating,
  createJob,
  download,
  stopPolling,
} = useSplitter();

const fileName = ref("");
const criterion = ref<JobCriterion>("minutes");
const value = ref(10);

const criterionOptions: { value: JobCriterion; label: string }[] = [
  { value: "minutes", label: "Phút / đoạn" },
  { value: "megabytes", label: "MB / đoạn" },
  { value: "parts", label: "Số phần" },
];

const jobBusy = (): boolean => jobStatus.value === "queued" || jobStatus.value === "running";

async function onLogout(): Promise<void> {
  await logout();
  await router.push({ name: "login" });
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
  <div class="app">
    <header class="topbar">
      <strong>OneScribe</strong>
      <div class="right">
        <span v-if="user" class="email">{{ user.email }}</span>
        <button @click="onLogout">Đăng xuất</button>
      </div>
    </header>
    <main class="content">
      <router-link class="back" to="/">&larr; Về trang chủ</router-link>
      <h1>Cắt file</h1>

      <section class="card">
        <label class="file-picker">
          <span>Chọn file audio/video</span>
          <input type="file" accept="audio/*,video/*" :disabled="uploading" @change="onFileChange" />
        </label>

        <p v-if="fileName" class="file-name">Tệp: {{ fileName }}</p>

        <div v-if="uploading" class="progress">
          <div class="bar"><div class="fill" :style="{ width: uploadProgress + '%' }" /></div>
          <span>Đang tải lên… {{ uploadProgress }}%</span>
        </div>

        <p v-if="uploadError" class="err">{{ uploadError }}</p>

        <p v-if="uploadResult" class="ok">
          Đã tải lên xong — thời lượng {{ formatDuration(uploadResult.durationS) }}
        </p>
      </section>

      <section v-if="uploadResult" class="card">
        <h2>Tiêu chí cắt</h2>
        <div class="criteria">
          <label v-for="opt in criterionOptions" :key="opt.value" class="radio">
            <input v-model="criterion" type="radio" name="criterion" :value="opt.value" />
            {{ opt.label }}
          </label>
        </div>
        <label class="value-input">
          Giá trị
          <input v-model.number="value" type="number" min="1" />
        </label>
        <button class="split-btn" :disabled="uploading || creating || jobBusy()" @click="onSplit">
          {{ creating || jobBusy() ? "Đang cắt…" : "Cắt" }}
        </button>

        <div v-if="jobBusy()" class="progress">
          <div class="bar"><div class="fill" :style="{ width: jobProgress + '%' }" /></div>
          <span>Đang xử lý… {{ jobProgress }}%</span>
        </div>

        <p v-if="jobError" class="err">{{ jobError }}</p>
      </section>

      <section v-if="jobStatus === 'done'" class="card">
        <h2>Kết quả</h2>
        <ul class="segments">
          <li v-for="seg in segments" :key="seg.fileId">{{ seg.name }}</li>
        </ul>
        <button @click="onDownload">Tải zip</button>
      </section>
    </main>
  </div>
</template>

<style scoped>
.topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.75rem 1.25rem;
  border-bottom: 1px solid #e5e7eb;
}
.right {
  display: flex;
  align-items: center;
  gap: 1rem;
}
.email {
  color: #6b7280;
}
.content {
  padding: 2rem 1.25rem;
  max-width: 640px;
  margin: 0 auto;
}
.back {
  display: inline-block;
  margin-bottom: 1rem;
  color: #6b7280;
  text-decoration: none;
}
.card {
  border: 1px solid #e5e7eb;
  border-radius: 12px;
  padding: 1.25rem;
  margin-bottom: 1.25rem;
}
.file-picker {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}
.file-name {
  margin-top: 0.75rem;
  color: #374151;
}
.progress {
  margin-top: 0.75rem;
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}
.bar {
  height: 8px;
  border-radius: 4px;
  background: #e5e7eb;
  overflow: hidden;
}
.fill {
  height: 100%;
  background: #2563eb;
  transition: width 0.2s ease;
}
.criteria {
  display: flex;
  gap: 1rem;
  margin: 0.75rem 0;
  flex-wrap: wrap;
}
.radio {
  display: flex;
  align-items: center;
  gap: 0.35rem;
}
.value-input {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-bottom: 1rem;
}
.value-input input {
  width: 100px;
  padding: 0.4rem;
}
.split-btn {
  padding: 0.6rem 1.25rem;
  border: 1px solid #d1d5db;
  border-radius: 8px;
  background: #2563eb;
  color: #fff;
  cursor: pointer;
}
.split-btn:disabled {
  opacity: 0.6;
  cursor: default;
}
.segments {
  margin: 0 0 1rem;
  padding-left: 1.25rem;
}
.err {
  color: #dc2626;
  margin-top: 0.75rem;
}
.ok {
  color: #16a34a;
  margin-top: 0.75rem;
}
</style>
