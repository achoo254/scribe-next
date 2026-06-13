// Composable Mode C: chọn file + tải model + transcribe batch + lắng nghe sidecar (transcribe_msg).
import { ref, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

export interface TransResult {
  file: string;
  text: string;
  txtPath: string | null;
  srtPath: string | null;
}
interface Msg {
  type: string;
  pct?: number;
  file?: string;
  text?: string;
  txt_path?: string | null;
  srt_path?: string | null;
  message?: string;
}

const AUDIO_EXT = ["mp3", "m4a", "wav", "aac", "flac", "ogg", "opus"];

export function useTranscribe() {
  const files = ref<string[]>([]);
  const modelReady = ref(false);
  const modelPath = ref("");
  const downloading = ref(false);
  const downloadPct = ref(0);
  const transcribing = ref(false);
  const progressPct = ref(0);
  const currentFile = ref("");
  const deviceInfo = ref("");
  const results = ref<TransResult[]>([]);
  const fileErrors = ref<{ file: string; message: string }[]>([]);
  const panelText = ref("");
  const error = ref("");
  const unlisteners: UnlistenFn[] = [];
  let expected = 0;
  let settled = 0; // done + error mỗi file

  function checkBatchEnd() {
    if (settled >= expected) {
      transcribing.value = false;
      progressPct.value = 100;
    }
  }

  async function init() {
    modelReady.value = await invoke<boolean>("model_status").catch(() => false);
    modelPath.value = await invoke<string>("model_path").catch(() => "");
    unlisteners.push(
      await listen<Msg>("transcribe_msg", (e) => {
        const m = e.payload;
        switch (m.type) {
          case "download":
            downloading.value = true;
            // pct của hub có thể nhảy (nhiều file) → giữ đơn điệu tăng.
            downloadPct.value = Math.max(downloadPct.value, m.pct ?? 0);
            break;
          case "model_ready":
            downloading.value = false;
            modelReady.value = true;
            break;
          case "info":
            deviceInfo.value = m.message ?? "";
            break;
          case "progress":
            currentFile.value = m.file ?? "";
            progressPct.value = m.pct ?? 0;
            break;
          case "done":
            results.value.push({
              file: m.file ?? "",
              text: m.text ?? "",
              txtPath: m.txt_path ?? null,
              srtPath: m.srt_path ?? null,
            });
            panelText.value = m.text ?? "";
            settled += 1;
            checkBatchEnd();
            break;
          case "error":
            if (m.file) {
              // Lỗi 1 file trong batch → ghi nhận + settle, KHÔNG dừng cả batch.
              fileErrors.value.push({ file: m.file, message: m.message ?? "Lỗi" });
              settled += 1;
              checkBatchEnd();
            } else {
              // Lỗi chung (model/spawn) → dừng.
              error.value = m.message ?? "Lỗi không xác định";
              transcribing.value = false;
              downloading.value = false;
            }
            break;
        }
      }),
    );
  }

  async function pickFiles() {
    const res = await open({ multiple: true, filters: [{ name: "Audio", extensions: AUDIO_EXT }] });
    if (Array.isArray(res)) files.value = res as string[];
    else if (typeof res === "string") files.value = [res];
  }

  async function ensureModel() {
    error.value = "";
    downloading.value = true;
    downloadPct.value = 0;
    try {
      await invoke("ensure_model");
    } catch (e) {
      error.value = String(e);
      downloading.value = false;
    }
  }

  async function transcribe() {
    if (!files.value.length) {
      error.value = "Hãy chọn ít nhất 1 file.";
      return;
    }
    error.value = "";
    results.value = [];
    fileErrors.value = [];
    panelText.value = "";
    settled = 0;
    expected = files.value.length;
    transcribing.value = true;
    progressPct.value = 0;
    try {
      await invoke("transcribe_files", { paths: files.value, outputs: ["txt", "srt"] });
    } catch (e) {
      error.value = String(e);
      transcribing.value = false;
    }
  }

  const cancel = () => {
    invoke("cancel_transcribe").catch(() => {});
    transcribing.value = false;
    downloading.value = false;
  };

  onUnmounted(() => unlisteners.forEach((u) => u()));

  return {
    files, modelReady, modelPath, downloading, downloadPct, transcribing, progressPct,
    currentFile, deviceInfo, results, fileErrors, panelText, error, init, pickFiles, ensureModel, transcribe, cancel,
  };
}
