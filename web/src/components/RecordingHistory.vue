<script setup lang="ts">
// Lịch sử ghi — list từ /api/files + phát lại inline (MediaPlayer) + tải (từng bản / theo ngày / tất cả).
// Expose refresh() cho RecordView.
import { onMounted, ref, computed } from "vue";
import { ViCard, ViButton, ViIcon, ViAlert } from "@hobui/viui";
import MediaPlayer from "./MediaPlayer.vue";
import { useRecordings, type RecordingItem } from "../composables/use-recordings";
import { useUsage } from "../composables/use-usage";

const { items, loading, error, refresh, remove, download, downloadAll, downloadRange } = useRecordings();
const usage = useUsage();
const playingId = ref<string | null>(null);
const confirmId = ref<string | null>(null); // bản ghi đang chờ xác nhận xoá
const busyId = ref<string | null>(null);

async function onDelete(id: string): Promise<void> {
  busyId.value = id;
  const ok = await remove(id);
  busyId.value = null;
  confirmId.value = null;
  if (ok) {
    if (playingId.value === id) playingId.value = null;
    void usage.refresh(); // dung lượng giảm sau khi xoá
  }
}

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

interface DateGroup {
  key: string;
  label: string;
  from: number; // ms mốc 00:00 ngày đó (giờ cục bộ)
  to: number; // ms 00:00 ngày kế → khoảng [from, to)
  items: RecordingItem[];
}

// Nhóm bản ghi theo NGÀY (giờ cục bộ). items đã sort mới→cũ từ BE nên Map giữ đúng thứ tự.
const groups = computed<DateGroup[]>(() => {
  const map = new Map<string, DateGroup>();
  for (const it of items.value) {
    const parsed = it.createdAt ? new Date(it.createdAt) : null;
    const d = parsed && !Number.isNaN(parsed.getTime()) ? parsed : new Date(0);
    const y = d.getFullYear();
    const m = d.getMonth();
    const day = d.getDate();
    const key = `${y}-${m}-${day}`;
    let g = map.get(key);
    if (!g) {
      const from = new Date(y, m, day, 0, 0, 0, 0).getTime();
      g = {
        key,
        label: d.toLocaleDateString("vi-VN", { weekday: "long", day: "2-digit", month: "2-digit", year: "numeric" }),
        from,
        to: from + 86_400_000,
        items: [],
      };
      map.set(key, g);
    }
    g.items.push(it);
  }
  return [...map.values()];
});

onMounted(refresh);
defineExpose({ refresh });
</script>

<template>
  <ViCard title="Lịch sử ghi" class="os-hist">
    <ViAlert v-if="error" type="error" variant="tonal" density="compact" :text="error" />
    <p v-else-if="loading" class="os-muted">Đang tải…</p>
    <p v-else-if="!items.length" class="os-muted">Chưa có bản ghi nào.</p>
    <template v-else>
      <!-- Tải tất cả bản ghi (1 zip). -->
      <div class="os-toolbar">
        <ViButton variant="tonal" color="primary" size="small" @click="downloadAll">
          <ViIcon icon="download" start />Tải tất cả
        </ViButton>
      </div>

      <div v-for="g in groups" :key="g.key" class="os-group">
        <div class="os-group-head">
          <span class="os-group-date">{{ g.label }}</span>
          <ViButton variant="text" color="primary" size="small" @click="downloadRange(g.from, g.to)">
            <ViIcon icon="download" start />Tải ngày ({{ g.items.length }})
          </ViButton>
        </div>

        <ul class="os-list">
          <li v-for="it in g.items" :key="it.id" class="os-item">
            <div class="os-item-row">
              <span class="os-tag">{{ it.hasVideo ? "Màn hình" : "Mic" }}</span>
              <div class="os-item-info">
                <div class="os-item-name">{{ it.name }}</div>
                <div class="os-muted">
                  {{ fmtDur(it.durationS) }} · {{ fmtSize(it.size) }} · {{ fmtDate(it.createdAt) }}
                </div>
              </div>
              <div class="os-item-actions">
                <ViButton
                  variant="text"
                  color="primary"
                  size="small"
                  :label="playingId === it.id ? 'Ẩn' : 'Nghe lại'"
                  @click="playingId = playingId === it.id ? null : it.id"
                />
                <ViButton variant="text" color="primary" size="small" label="Tải" @click="download(it.id)" />
                <template v-if="confirmId === it.id">
                  <span class="os-confirm-q">Xoá?</span>
                  <ViButton
                    variant="text"
                    color="error"
                    size="small"
                    :loading="busyId === it.id"
                    label="Xoá"
                    @click="onDelete(it.id)"
                  />
                  <ViButton variant="text" size="small" label="Huỷ" @click="confirmId = null" />
                </template>
                <ViButton
                  v-else
                  variant="text"
                  color="error"
                  size="small"
                  label="Xoá"
                  @click="confirmId = it.id"
                />
              </div>
            </div>
            <MediaPlayer
              v-if="playingId === it.id"
              :file-id="it.id"
              :video="it.hasVideo"
              :duration-s="it.durationS"
              class="os-player"
            />
          </li>
        </ul>
      </div>
    </template>
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
.os-toolbar {
  display: flex;
  justify-content: flex-end;
  margin-bottom: 0.75rem;
}
.os-group + .os-group {
  margin-top: 1.25rem;
}
.os-group-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  padding-bottom: 0.5rem;
  margin-bottom: 0.5rem;
  border-bottom: 1px solid rgb(var(--v-theme-surface-bright));
}
.os-group-date {
  font-size: 0.8rem;
  font-weight: 600;
  text-transform: capitalize;
  color: rgb(var(--v-theme-on-surface));
  opacity: 0.85;
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
.os-item-actions {
  display: flex;
  align-items: center;
  gap: 0.25rem;
  flex: none;
}
.os-confirm-q {
  font-size: 0.8rem;
  color: rgb(var(--v-theme-error));
}
.os-player {
  margin-top: 0.5rem;
}
</style>
