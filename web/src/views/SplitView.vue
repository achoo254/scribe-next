<script setup lang="ts">
// Tab "Cắt file" (web) — gộp 1 ViCard giống desktop SplitView: chọn+upload → tiêu chí → cắt → kết quả.
// Web upload async lên server (khác desktop chọn file local), nhưng trình bày trong cùng 1 card.
import { onUnmounted, onActivated, ref, computed, watch } from "vue";
import { ViCard, ViButton, ViIcon, ViRadioGroup, ViInput, ViProgressLinear, ViAlert } from "@hobui/viui";
import { useUpload } from "../composables/use-upload";
import { useSplitter, type JobCriterion } from "../composables/use-splitter";
import { useUsage } from "../composables/use-usage";
import { useManagedFiles } from "../composables/use-managed-files";

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
  reset: resetJob,
} = useSplitter();

const usage = useUsage();
const mf = useManagedFiles();

const fileInput = ref<HTMLInputElement | null>(null);
const fileName = ref("");
const criterion = ref<JobCriterion>("minutes");
const value = ref(10);

// Upload + đoạn cắt đều tính vào quota → cập nhật QuotaBar + danh sách tệp khi xong.
watch(uploadResult, (r) => { if (r) { void usage.refresh(); void mf.refresh(); } });
watch(jobStatus, (s) => { if (s === "done") { void usage.refresh(); void mf.refresh(); } });

// Nhóm tệp: upload (file nguồn) và output gộp theo lần cắt (jobId).
interface OutputGroup { jobId: string; count: number; bytes: number; createdAt?: string; ids: string[] }
const uploads = computed(() => mf.items.value.filter((f) => f.kind === "upload"));
const outputGroups = computed<OutputGroup[]>(() => {
  const map = new Map<string, OutputGroup>();
  for (const f of mf.items.value) {
    if (f.kind !== "output") continue;
    const key = f.jobId ?? "—";
    let g = map.get(key);
    if (!g) {
      g = { jobId: key, count: 0, bytes: 0, createdAt: f.createdAt, ids: [] };
      map.set(key, g);
    }
    g.count += 1;
    g.bytes += f.size;
    g.ids.push(f.id);
  }
  return [...map.values()];
});

function fmtSize(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${Math.max(1, Math.round(bytes / 1024))} KB`;
}
function fmtDate(iso?: string): string {
  if (!iso) return "";
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? "" : d.toLocaleString("vi-VN");
}

async function onDeleteFile(id: string): Promise<void> {
  if (await mf.remove(id)) void usage.refresh();
}
async function onDeleteGroup(ids: string[]): Promise<void> {
  if ((await mf.removeMany(ids)) > 0) void usage.refresh();
}

// keep-alive: refresh mỗi lần mở tab (bắt cả lần mount đầu) để phản ánh đúng dung lượng đang chiếm.
onActivated(() => void mf.refresh());

const criterionOptions: { title: string; value: JobCriterion }[] = [
  { title: "Phút", value: "minutes" },
  { title: "MB", value: "megabytes" },
  { title: "Số phần", value: "parts" },
];

const valueLabel = computed(() =>
  criterion.value === "minutes"
    ? "Số phút mỗi đoạn"
    : criterion.value === "megabytes"
      ? "MB mỗi đoạn"
      : "Chia thành (số đoạn)",
);

const jobBusy = (): boolean => jobStatus.value === "queued" || jobStatus.value === "running";

function pickFile(): void {
  fileInput.value?.click();
}

async function onFileChange(e: Event): Promise<void> {
  const input = e.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  fileName.value = file.name;
  resetJob(); // xoá kết quả/link tải của file trước, tránh tải nhầm zip khi cắt nhiều file cùng phiên
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
  <ViCard title="Cắt file" class="os-split-card">
    <div class="os-form">
      <!-- Bước 1: chọn + upload file -->
      <div class="os-row">
        <input
          ref="fileInput"
          type="file"
          accept="audio/*,video/*"
          class="d-none"
          @change="onFileChange"
        />
        <ViButton variant="outlined" :disabled="uploading" @click="pickFile">
          <ViIcon icon="folder" start />Chọn file audio/video
        </ViButton>
        <span v-if="fileName" class="os-muted">{{ fileName }}</span>
      </div>
      <ViProgressLinear
        v-if="uploading"
        :model-value="uploadProgress"
        color="primary"
        height="8"
        rounded
      />
      <p v-if="uploading" class="os-muted">Đang tải lên… {{ uploadProgress }}%</p>
      <ViAlert
        v-if="uploadError"
        type="error"
        variant="tonal"
        density="compact"
        :text="uploadError"
      />
      <ViAlert
        v-if="uploadResult"
        type="success"
        variant="tonal"
        density="compact"
        :text="`Đã tải lên — thời lượng ${formatDuration(uploadResult.durationS)}`"
      />

      <!-- Bước 2: tiêu chí + cắt -->
      <template v-if="uploadResult">
        <ViRadioGroup
          v-model="criterion"
          inline
          label="Tiêu chí"
          :items="criterionOptions"
          :disabled="jobBusy()"
        />
        <ViInput
          v-model.number="value"
          type="number"
          min="1"
          :label="valueLabel"
          density="comfortable"
          class="os-num"
        />
        <ViButton
          color="primary"
          size="large"
          :loading="creating || jobBusy()"
          :disabled="uploading"
          @click="onSplit"
        >
          <ViIcon icon="scissors" start />Cắt
        </ViButton>
        <ViProgressLinear v-if="jobBusy()" indeterminate color="primary" height="8" rounded />
        <p v-if="jobBusy()" class="os-muted">Đang xử lý…</p>
        <ViAlert
          v-if="jobError"
          type="error"
          variant="tonal"
          density="compact"
          :text="jobError"
        />
      </template>

      <!-- Bước 3: kết quả -->
      <template v-if="jobStatus === 'done'">
        <h3 class="os-result-title">Kết quả ({{ segments.length }} đoạn)</h3>
        <ul class="os-segments">
          <li v-for="seg in segments" :key="seg.fileId">{{ seg.name }}</li>
        </ul>
        <ViButton color="primary" variant="flat" @click="onDownload">
          <ViIcon icon="download" start />Tải zip
        </ViButton>
      </template>
    </div>
  </ViCard>

  <!-- Quản lý tệp Cắt file (upload + đoạn cắt) — chiếm quota nhưng không hiện ở Lịch sử ghi. -->
  <ViCard v-if="mf.items.value.length" title="Tệp trên hệ thống" class="os-manage-card">
    <p class="os-muted os-manage-hint">
      Tệp bạn tải lên để cắt và các đoạn đã cắt vẫn tính vào dung lượng. Xoá bớt để giải phóng
      (tự động xoá sau 14 ngày nếu không xoá tay).
    </p>

    <template v-if="uploads.length">
      <h3 class="os-manage-sub">Tệp đã tải lên</h3>
      <ul class="os-manage-list">
        <li v-for="f in uploads" :key="f.id" class="os-manage-item">
          <div class="os-manage-info">
            <div class="os-manage-name">{{ f.name }}</div>
            <div class="os-muted">{{ fmtSize(f.size) }} · {{ fmtDate(f.createdAt) }}</div>
          </div>
          <div class="os-manage-actions">
            <ViButton variant="text" color="primary" size="small" label="Tải" @click="mf.download(f.id)" />
            <ViButton variant="text" color="error" size="small" label="Xoá" @click="onDeleteFile(f.id)" />
          </div>
        </li>
      </ul>
    </template>

    <template v-if="outputGroups.length">
      <h3 class="os-manage-sub">Đoạn đã cắt</h3>
      <ul class="os-manage-list">
        <li v-for="g in outputGroups" :key="g.jobId" class="os-manage-item">
          <div class="os-manage-info">
            <div class="os-manage-name">{{ g.count }} đoạn</div>
            <div class="os-muted">{{ fmtSize(g.bytes) }} · {{ fmtDate(g.createdAt) }}</div>
          </div>
          <div class="os-manage-actions">
            <ViButton variant="text" color="error" size="small" label="Xoá tất cả" @click="onDeleteGroup(g.ids)" />
          </div>
        </li>
      </ul>
    </template>
  </ViCard>
</template>

<style scoped>
.os-split-card {
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
  gap: 0.5rem;
  flex-wrap: wrap;
}
.os-muted {
  font-size: 0.875rem;
  color: rgb(var(--v-theme-on-surface));
  opacity: 0.75;
  margin: 0;
}
.os-num {
  max-width: 240px;
}
.os-result-title {
  font-size: 1rem;
  font-weight: 600;
  margin: 0;
  color: rgb(var(--v-theme-on-surface));
}
.os-segments {
  margin: 0;
  padding-left: 1.25rem;
  color: rgb(var(--v-theme-on-surface));
  opacity: 0.85;
  max-height: 220px;
  overflow-y: auto;
}
.os-segments li {
  margin-bottom: 0.25rem;
}
.os-manage-card {
  padding: 1rem;
  margin-top: 1rem;
}
.os-manage-hint {
  margin-bottom: 1rem;
}
.os-manage-sub {
  font-size: 0.9rem;
  font-weight: 600;
  margin: 0.75rem 0 0.5rem;
  color: rgb(var(--v-theme-on-surface));
}
.os-manage-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}
.os-manage-item {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.5rem 0;
  border-top: 1px solid rgb(var(--v-theme-surface-bright));
}
.os-manage-item:first-child {
  border-top: none;
}
.os-manage-info {
  flex: 1;
  min-width: 0;
}
.os-manage-name {
  font-weight: 600;
  color: rgb(var(--v-theme-on-surface));
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.os-manage-actions {
  display: flex;
  align-items: center;
  gap: 0.25rem;
  flex: none;
}
</style>
