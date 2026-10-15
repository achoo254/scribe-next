// Dung lượng đã dùng / quota (shared singleton) — GET /api/me/usage. RecordView/SplitView gọi
// refresh() sau khi ghi/cắt để QuotaBar cập nhật ngay.
import { ref } from "vue";

const usageBytes = ref(0);
const quotaBytes = ref(5 * 1024 * 1024 * 1024);
const loaded = ref(false);

async function refresh(): Promise<void> {
  try {
    const res = await fetch("/api/me/usage", { credentials: "include" });
    if (!res.ok) return;
    const b = (await res.json()) as { usageBytes: number; quotaBytes: number };
    usageBytes.value = b.usageBytes;
    quotaBytes.value = b.quotaBytes;
    loaded.value = true;
  } catch {
    /* im lặng — QuotaBar chỉ là thông tin phụ */
  }
}

export function useUsage() {
  return { usageBytes, quotaBytes, loaded, refresh };
}
