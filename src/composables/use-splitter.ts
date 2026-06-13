// Composable Mode B: chọn file/thư mục + probe + cắt batch + lắng nghe tiến độ.
import { ref, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

export interface OutFile {
  name: string;
  path: string;
}
export interface SplitProgress {
  file_index: number;
  total: number;
  file_name: string;
  file_pct: number;
  overall_pct: number;
}
export interface FileResult {
  file: string;
  segments: OutFile[];
}

const AUDIO_EXT = ["mp3", "m4a", "wav", "aac", "flac", "ogg", "opus", "wma"];

export function useSplitter() {
  const files = ref<string[]>([]);
  const durations = ref<Record<string, number>>({}); // path → giây (probe)
  const outDir = ref("");
  const running = ref(false);
  const progress = ref<SplitProgress | null>(null);
  const results = ref<FileResult[]>([]);
  const errors = ref<{ file: string; error: string }[]>([]);
  const unlisteners: UnlistenFn[] = [];

  async function init() {
    unlisteners.push(
      await listen<SplitProgress>("split_progress", (e) => (progress.value = e.payload)),
      await listen<[string, OutFile[]]>("split_file_done", (e) =>
        results.value.push({ file: e.payload[0], segments: e.payload[1] }),
      ),
      await listen<[string, string]>("split_error", (e) =>
        errors.value.push({ file: e.payload[0], error: e.payload[1] }),
      ),
      await listen("split_all_done", () => {
        running.value = false;
        progress.value = null;
      }),
    );
  }

  async function pickFiles() {
    const res = await open({
      multiple: true,
      filters: [{ name: "Audio", extensions: AUDIO_EXT }],
    });
    if (Array.isArray(res)) files.value = res as string[];
    else if (typeof res === "string") files.value = [res];
    else return;
    // Probe để hiển thị thời lượng trước khi cắt.
    durations.value = {};
    for (const p of files.value) {
      try {
        const info = await invoke<{ duration_s: number }>("probe_file", { path: p });
        durations.value[p] = info.duration_s;
      } catch {
        /* bỏ qua file probe lỗi */
      }
    }
  }

  async function pickOutDir() {
    const d = await open({ directory: true });
    if (typeof d === "string") outDir.value = d;
  }

  async function split(criterion: string, value: number) {
    if (!files.value.length) {
      errors.value = [{ file: "", error: "Hãy chọn ít nhất 1 file." }];
      return;
    }
    if (!outDir.value) {
      errors.value = [{ file: "", error: "Hãy chọn thư mục lưu." }];
      return;
    }
    running.value = true;
    results.value = [];
    errors.value = [];
    progress.value = null;
    try {
      await invoke("split_files", {
        paths: files.value,
        criterion,
        value,
        outDir: outDir.value,
      });
    } catch (e) {
      errors.value.push({ file: "", error: String(e) });
      running.value = false;
    }
  }

  onUnmounted(() => unlisteners.forEach((u) => u()));

  return { files, durations, outDir, running, progress, results, errors, init, pickFiles, pickOutDir, split };
}
