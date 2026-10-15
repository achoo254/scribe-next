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
  const retentionDays = ref(0); // BE trả về; 0 = chưa biết → FE ẩn thông báo tự-xoá

  async function refresh(): Promise<void> {
    loading.value = true;
    error.value = "";
    try {
      const res = await fetch("/api/files", { credentials: "include" });
      if (!res.ok) throw new Error("Không tải được lịch sử ghi");
      const body = (await res.json()) as { items: RecordingItem[]; retentionDays?: number };
      items.value = body.items ?? [];
      if (typeof body.retentionDays === "number") retentionDays.value = body.retentionDays;
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

  /** Tải 1 bản ghi (Content-Disposition attachment → giữ tên + đuôi đúng). */
  function download(id: string): void {
    window.location.href = `/api/files/${id}?download=1`;
  }
  /** Tải TẤT CẢ bản ghi dưới dạng 1 zip. */
  function downloadAll(): void {
    window.location.href = "/api/files/archive";
  }
  /** Tải các bản ghi trong khoảng [from, to) ms (dùng cho "tải theo ngày" — mốc tính theo giờ cục bộ). */
  function downloadRange(from: number, to: number): void {
    window.location.href = `/api/files/archive?from=${from}&to=${to}`;
  }

  return { items, loading, error, retentionDays, refresh, remove, download, downloadAll, downloadRange };
}
