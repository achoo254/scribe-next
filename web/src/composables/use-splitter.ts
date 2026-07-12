// Tạo job cắt file (Mode B) + poll tiến độ tới khi done/failed + tải zip kết quả.
import { ref } from "vue";

export type JobCriterion = "minutes" | "megabytes" | "parts";
export type JobStatus = "queued" | "running" | "done" | "failed";

export interface JobSegment {
  name: string;
  fileId: string;
}

interface ApiErrorBody {
  error?: string;
}

interface JobStatusBody {
  status: JobStatus;
  progress: number;
  result?: { segments: JobSegment[] };
  error?: string;
}

const POLL_INTERVAL_MS = 1500;
const MAX_POLL_FAILS = 5; // lỗi mạng/5xx thoáng qua → retry, không bỏ cuộc ngay

async function readErrorMessage(res: Response, fallback: string): Promise<string> {
  const body = (await res.json().catch(() => null)) as ApiErrorBody | null;
  return body?.error ?? fallback;
}

export function useSplitter() {
  const jobId = ref<string | null>(null);
  const status = ref<JobStatus | null>(null);
  const progress = ref(0);
  const segments = ref<JobSegment[]>([]);
  const error = ref("");
  const creating = ref(false);

  let pollTimer: number | null = null;

  function stopPolling(): void {
    if (pollTimer !== null) {
      clearTimeout(pollTimer);
      pollTimer = null;
    }
  }

  async function pollOnce(id: string, fails = 0): Promise<void> {
    try {
      const res = await fetch(`/api/jobs/${id}`, { credentials: "include" });
      if (!res.ok) {
        throw new Error(await readErrorMessage(res, "Không thể lấy tiến độ công việc"));
      }
      const body = (await res.json()) as JobStatusBody;
      status.value = body.status;
      progress.value = body.progress;

      if (body.status === "done") {
        segments.value = body.result?.segments ?? [];
        return;
      }
      if (body.status === "failed") {
        error.value = body.error ?? "Cắt file thất bại";
        return;
      }
      pollTimer = window.setTimeout(() => void pollOnce(id, 0), POLL_INTERVAL_MS); // reset fails khi OK
    } catch (e) {
      // Lỗi thoáng qua (mạng/5xx) → retry với backoff; chỉ bỏ cuộc sau MAX_POLL_FAILS lần liên tiếp.
      const next = fails + 1;
      if (next >= MAX_POLL_FAILS) {
        error.value = e instanceof Error ? e.message : "Lỗi khi theo dõi tiến độ";
        return;
      }
      pollTimer = window.setTimeout(() => void pollOnce(id, next), POLL_INTERVAL_MS * 2);
    }
  }

  async function createJob(inputFileId: string, criterion: JobCriterion, value: number): Promise<void> {
    stopPolling();
    creating.value = true;
    error.value = "";
    segments.value = [];
    status.value = null;
    progress.value = 0;

    try {
      const res = await fetch("/api/jobs", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "include",
        body: JSON.stringify({ inputFileId, criterion, value }),
      });
      if (!res.ok) {
        throw new Error(await readErrorMessage(res, "Không thể tạo công việc cắt file"));
      }
      const body = (await res.json()) as { jobId: string };
      jobId.value = body.jobId;
      status.value = "queued";
      void pollOnce(body.jobId);
    } catch (e) {
      error.value = e instanceof Error ? e.message : "Không thể tạo công việc cắt file";
    } finally {
      creating.value = false;
    }
  }

  function download(id: string): void {
    window.location.href = `/api/jobs/${id}/download`;
  }

  // Dừng poll + xoá sạch state job — gọi khi user chọn file mới để không giữ kết quả/link tải của file cũ.
  function reset(): void {
    stopPolling();
    jobId.value = null;
    status.value = null;
    progress.value = 0;
    segments.value = [];
    error.value = "";
    creating.value = false;
  }

  return { jobId, status, progress, segments, error, creating, createJob, download, stopPolling, reset };
}
