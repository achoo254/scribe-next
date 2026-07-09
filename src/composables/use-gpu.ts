// Setting GPU (CUDA) + vòng đời tải CUDA runtime — dùng chung Mode A + C (mirror use-glossary.ts).
// 2 composable cùng file vì cùng khái niệm "GPU" trên UI (KISS/DRY, tránh vụn file nhỏ).
import { ref, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { loadSettings, patchSettings } from "./settings-store";

// Đọc/ghi AppSettings.use_gpu (Rust đọc field này ở Transcriber::transcribe cho cả 2 mode).
export function useGpuSetting() {
  const useGpu = ref(false);
  async function load() {
    const s = await loadSettings();
    useGpu.value = typeof s.use_gpu === "boolean" ? s.use_gpu : false;
  }
  async function save() {
    await patchSettings({ use_gpu: useGpu.value });
  }
  return { useGpu, load, save };
}

// Trạng thái CUDA runtime: có GPU không (cudaSupported), đã tải chưa (ready), tiến độ tải.
// Nghe cùng event transcribe_msg (kênh Rust emit duy nhất) nhưng filter type cuda_download/cuda_ready
// → độc lập hoàn toàn với download/model_ready của model (use-transcribe.ts không đổi).
export function useCudaRuntime() {
  const cudaSupported = ref(false);
  const ready = ref(false);
  const downloading = ref(false);
  const downloadPct = ref(0);
  const error = ref("");
  const unlisteners: UnlistenFn[] = [];

  async function init() {
    cudaSupported.value = await invoke<boolean>("cuda_supported").catch(() => false);
    if (!cudaSupported.value) return; // máy không NVIDIA → không nghe gì, UI không hiện khối GPU
    ready.value = await invoke<boolean>("cuda_runtime_status").catch(() => false);
    unlisteners.push(
      await listen<{ type: string; pct?: number; message?: string }>("transcribe_msg", (e) => {
        const m = e.payload;
        if (m.type === "cuda_download") {
          downloading.value = true;
          downloadPct.value = Math.max(downloadPct.value, m.pct ?? 0); // giữ đơn điệu tăng
        } else if (m.type === "cuda_ready") {
          ready.value = true;
          downloading.value = false;
        } else if (m.type === "error" && downloading.value) {
          // Lỗi lúc đang tải CUDA runtime (mạng/sha256/đĩa) — không có field file. Guard downloading
          // để KHÔNG nuốt (kẹt nút tải) + không nhầm với lỗi transcribe/model của use-transcribe.ts.
          error.value = m.message ?? "Lỗi tải CUDA runtime";
          downloading.value = false;
        }
      }),
    );
  }

  async function ensure() {
    error.value = "";
    downloading.value = true;
    downloadPct.value = 0;
    try {
      await invoke("ensure_cuda_runtime");
    } catch (e) {
      error.value = String(e);
      downloading.value = false;
    }
  }

  onUnmounted(() => unlisteners.forEach((u) => u()));
  return { cudaSupported, ready, downloading, downloadPct, error, init, ensure };
}
