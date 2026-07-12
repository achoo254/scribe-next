// Lịch sử ghi (Mode A) — GET /api/files. Refreshable để RecordView gọi lại sau khi ghi xong.
import { ref } from "vue";

export interface RecordingItem {
  id: string;
  name: string;
  durationS?: number;
  hasVideo: boolean;
  size: number;
  createdAt?: string;
}

export function useRecordings() {
  const items = ref<RecordingItem[]>([]);
  const loading = ref(false);
  const error = ref("");

  async function refresh(): Promise<void> {
    loading.value = true;
    error.value = "";
    try {
      const res = await fetch("/api/files", { credentials: "include" });
      if (!res.ok) throw new Error("Không tải được lịch sử ghi");
      const body = (await res.json()) as { items: RecordingItem[] };
      items.value = body.items ?? [];
    } catch (e) {
      error.value = e instanceof Error ? e.message : "Lỗi tải lịch sử";
    } finally {
      loading.value = false;
    }
  }

  /** Xoá 1 bản ghi (DELETE /api/files/:id) → bỏ khỏi list nếu thành công. */
  async function remove(id: string): Promise<boolean> {
    try {
      const res = await fetch(`/api/files/${id}`, { method: "DELETE", credentials: "include" });
      if (!res.ok) return false;
      items.value = items.value.filter((x) => x.id !== id);
      return true;
    } catch {
      return false;
    }
  }

  return { items, loading, error, refresh, remove };
}
