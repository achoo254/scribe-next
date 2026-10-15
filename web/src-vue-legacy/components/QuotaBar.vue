<script setup lang="ts">
// Thanh dung lượng: đã dùng / quota (5GB). Cảnh báo màu khi gần đầy. Refresh qua shared store.
import { computed, onMounted } from "vue";
import { ViProgressLinear } from "@hobui/viui";
import { useUsage } from "../composables/use-usage";

const { usageBytes, quotaBytes, loaded, refresh } = useUsage();

const pct = computed(() =>
  quotaBytes.value > 0 ? Math.min(100, (usageBytes.value / quotaBytes.value) * 100) : 0,
);
const color = computed(() => (pct.value >= 90 ? "error" : pct.value >= 75 ? "warning" : "primary"));

function gb(bytes: number): string {
  return (bytes / 1024 / 1024 / 1024).toFixed(2);
}

onMounted(refresh);
</script>

<template>
  <div v-if="loaded" class="os-quota">
    <div class="os-quota-head">
      <span class="os-quota-label">Dung lượng</span>
      <span class="os-quota-val">{{ gb(usageBytes) }} / {{ gb(quotaBytes) }} GB</span>
    </div>
    <ViProgressLinear :model-value="pct" :color="color" height="6" rounded />
  </div>
</template>

<style scoped>
.os-quota {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}
.os-quota-head {
  display: flex;
  justify-content: space-between;
  font-size: 0.75rem;
  color: rgb(var(--v-theme-on-surface));
  opacity: 0.75;
}
.os-quota-val {
  font-variant-numeric: tabular-nums;
}
</style>
