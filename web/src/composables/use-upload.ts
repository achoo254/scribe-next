// Upload 1 file lên BE theo chunk 5MB, append thẳng disk phía BE (không buffer RAM).
//
// Flow: init (BE sinh sessionId) → status (lấy offset đã ghi, hỗ trợ resume) → loop POST chunk
// theo offset → finalize (BE trả fileId + thời lượng). Session được giữ lại trong composable để
// nếu upload() được gọi lại cho CÙNG 1 file (sau lỗi mạng) thì resume từ offset đã ghi, không
// init lại từ đầu.
import { ref } from "vue";

const CHUNK_SIZE = 5 * 1024 * 1024; // 5MB

export interface UploadResult {
  fileId: string;
  name: string;
  durationS: number;
}

interface ApiErrorBody {
  error?: string;
}

async function readErrorMessage(res: Response, fallback: string): Promise<string> {
  const body = (await res.json().catch(() => null)) as ApiErrorBody | null;
  return body?.error ?? fallback;
}

export function useUpload() {
  const uploading = ref(false);
  const progress = ref(0); // 0-100
  const error = ref("");
  const result = ref<UploadResult | null>(null);

  // Session hiện tại (module-scope theo instance composable) — dùng để resume khi retry cùng file.
  let sessionId: string | null = null;
  let sessionFile: File | null = null;

  async function upload(file: File): Promise<UploadResult | null> {
    uploading.value = true;
    error.value = "";
    result.value = null;

    try {
      if (sessionId === null || sessionFile !== file) {
        const initRes = await fetch("/api/upload/init", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          credentials: "include",
          body: JSON.stringify({ name: file.name, size: file.size }),
        });
        if (!initRes.ok) {
          throw new Error(
            await readErrorMessage(initRes, "Không thể khởi tạo upload (file quá lớn hoặc vượt hạn mức)"),
          );
        }
        const initBody = (await initRes.json()) as { sessionId: string };
        sessionId = initBody.sessionId;
        sessionFile = file;
      }

      const statusRes = await fetch(`/api/upload/status?session=${sessionId}`, {
        credentials: "include",
      });
      if (!statusRes.ok) {
        throw new Error(await readErrorMessage(statusRes, "Không thể lấy trạng thái upload"));
      }
      const statusBody = (await statusRes.json()) as { written: number };

      let offset = statusBody.written;
      progress.value = file.size > 0 ? Math.floor((offset / file.size) * 100) : 100;

      while (offset < file.size) {
        const chunk = file.slice(offset, Math.min(offset + CHUNK_SIZE, file.size));
        const chunkRes = await fetch(`/api/upload/chunk?session=${sessionId}&offset=${offset}`, {
          method: "POST",
          credentials: "include",
          headers: { "Content-Type": "application/octet-stream" },
          body: chunk,
        });
        if (!chunkRes.ok) {
          throw new Error(await readErrorMessage(chunkRes, "Tải lên thất bại, có thể thử lại"));
        }
        const chunkBody = (await chunkRes.json()) as { written: number };
        offset = chunkBody.written;
        progress.value = file.size > 0 ? Math.floor((offset / file.size) * 100) : 100;
      }

      const finalizeRes = await fetch("/api/upload/finalize", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "include",
        body: JSON.stringify({ session: sessionId }),
      });
      if (!finalizeRes.ok) {
        throw new Error(await readErrorMessage(finalizeRes, "Không thể hoàn tất upload"));
      }
      const finalized = (await finalizeRes.json()) as UploadResult;

      progress.value = 100;
      result.value = finalized;
      // Thành công → xoá session, lần upload tiếp theo (file khác) sẽ init mới.
      sessionId = null;
      sessionFile = null;
      return finalized;
    } catch (e) {
      error.value = e instanceof Error ? e.message : "Tải lên thất bại";
      return null;
    } finally {
      uploading.value = false;
    }
  }

  function reset(): void {
    uploading.value = false;
    progress.value = 0;
    error.value = "";
    result.value = null;
    sessionId = null;
    sessionFile = null;
  }

  return { uploading, progress, error, result, upload, reset };
}
