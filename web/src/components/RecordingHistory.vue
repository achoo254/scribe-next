<script setup lang="ts">
// Lịch sử ghi — list từ /api/files + phát lại inline (MediaPlayer). Expose refresh() cho RecordView.
import { onMounted, ref } from "vue";
import { ViCard, ViButton, ViAlert } from "@hobui/viui";
import MediaPlayer from "./MediaPlayer.vue";
import { useRecordings } from "../composables/use-recordings";

const { items, loading, error, refresh } = useRecordings();
const playingId = ref<string | null>(null);

function fmtDur(s?: number): string {
  if (!s || s <= 0) return "—";
  const t = Math.round(s);
  const h = Math.floor(t / 3600);
  const m = Math.floor((t % 3600) / 60);
  const ss = String(t % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}
function fmtSize(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${Math.max(1, Math.round(bytes / 1024))} KB`;
}
function fmtDate(iso?: string): string {
  if (!iso) return "";
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? "" : d.toLocaleString("vi-VN");
}

onMounted(refresh);
defineExpose({ refresh });
</script>

<template>
  <ViCard title="Lịch sử ghi" class="os-hist">
    <ViAlert v-if="error" type="error" variant="tonal" density="compact" :text="error" />
    <p v-else-if="loading" class="os-muted">Đang tải…</p>
    <p v-else-if="!items.length" class="os-muted">Chưa có bản ghi nào.</p>
    <ul v-else class="os-list">
      <li v-for="it in items" :key="it.id" class="os-item">
        <div class="os-item-row">
          <span class="os-tag">{{ it.hasVideo ? "Màn hình" : "Mic" }}</span>
          <div class="os-item-info">
            <div class="os-item-name">{{ it.name }}</div>
            <div class="os-muted">
              {{ fmtDur(it.durationS) }} · {{ fmtSize(it.size) }} · {{ fmtDate(it.createdAt) }}
            </div>
          </div>
          <ViButton
            variant="text"
            color="primary"
            size="small"
            :label="playingId === it.id ? 'Ẩn' : 'Nghe lại'"
            @click="playingId = playingId === it.id ? null : it.id"
          />
        </div>
        <MediaPlayer
          v-if="playingId === it.id"
          :file-id="it.id"
          :video="it.hasVideo"
          class="os-player"
        />
      </li>
    </ul>
  </ViCard>
</template>

<style scoped>
.os-hist {
  padding: 1rem;
}
.os-muted {
  font-size: 0.875rem;
  color: rgb(var(--v-theme-on-surface));
  opacity: 0.7;
  margin: 0;
}
.os-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}
.os-item {
  border-top: 1px solid rgb(var(--v-theme-surface-bright));
  padding-top: 0.75rem;
}
.os-item:first-child {
  border-top: none;
  padding-top: 0;
}
.os-item-row {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}
.os-tag {
  flex: none;
  font-size: 0.625rem;
  font-weight: 600;
  text-transform: uppercase;
  padding: 0.15rem 0.4rem;
  border-radius: 999px;
  color: rgb(var(--v-theme-secondary));
  border: 1px solid rgb(var(--v-theme-secondary));
}
.os-item-info {
  flex: 1;
  min-width: 0;
}
.os-item-name {
  font-weight: 600;
  color: rgb(var(--v-theme-on-surface));
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.os-player {
  margin-top: 0.5rem;
}
</style>
