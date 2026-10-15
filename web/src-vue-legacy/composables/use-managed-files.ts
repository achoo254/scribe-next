// Quản lý file KHÔNG phải bản ghi (upload nguồn Cắt file + đoạn output) — GET /api/files/manage.
// Những file này tính vào quota nhưng không hiện ở "Lịch sử ghi"; cho phép xem & xoá để giải phóng.
import { ref } from "vue";

export interface ManagedFile {
  id: string;
  name: string;
  kind: "upload" | "output";
  size: number;
  jobId?: string;
  createdAt?: string;
}

export function useManagedFiles() {
  const items = ref<ManagedFile[]>([]);
  const loading = ref(false);
  const error = ref("");

  async function refresh(): Promise<void> {
    loading.value = true;
    error.value = "";
    try {
      const res = await fetch("/api/files/manage", { credentials: "include" });
      if (!res.ok) throw new Error("Không tải được danh sách tệp");
      const body = (await res.json()) as { items: ManagedFile[] };
      items.value = body.items ?? [];
    } catch (e) {
      error.value = e instanceof Error ? e.message : "Lỗi tải danh sách tệp";
    } finally {
      loading.value = false;
    }
  }

  /** Xoá 1 tệp (DELETE /api/files/:id — dùng chung route với bản ghi). */
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

  /** Xoá nhiều tệp (vd tất cả đoạn cắt của 1 lần cắt). Trả số xoá thành công. */
  async function removeMany(ids: string[]): Promise<number> {
    let ok = 0;
    for (const id of ids) {
      if (await remove(id)) ok++;
    }
    return ok;
  }

  function download(id: string): void {
    window.location.href = `/api/files/${id}?download=1`;
  }

  return { items, loading, error, refresh, remove, removeMany, download };
}
