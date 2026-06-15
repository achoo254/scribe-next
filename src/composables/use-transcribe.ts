// Composable Mode C: chọn file + tải model + transcribe batch + lắng nghe sidecar (transcribe_msg).
import { ref, computed, watch, onUnmounted } from "vue";
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

export type ModelSize = "small" | "medium" | "large";

export function useTranscribe() {
  const files = ref<string[]>([]);
  const selectedSize = ref<ModelSize>("medium"); // mặc định medium (cân bằng tốc độ/chất lượng)
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
  const expected = ref(0); // tổng số file của batch
  const settled = ref(0);  // số file đã xong (done + error)
  // File mà CHÍNH composable này gửi đi — lọc transcribe_msg để không "nhận nhầm" event của
  // tab khác (Mode A tự-động-transcript dùng chung sidecar + event). Cần khi App bật keep-alive.
  let owned = new Set<string>();

  function checkBatchEnd() {
    if (settled.value >= expected.value) transcribing.value = false;
  }

  // Tiến độ TỔNG cả batch: mỗi file chiếm 1/expected; file đang chạy góp progressPct phần trăm của nó.
  const overallPct = computed(() =>
    expected.value
      ? Math.min(100, Math.round((settled.value * 100 + progressPct.value) / expected.value))
      : 0,
  );

  // Kiểm tra model của size đang chọn đã tải chưa + lấy đường dẫn.
  async function refreshModel() {
    modelReady.value = await invoke<boolean>("model_status", { size: selectedSize.value }).catch(() => false);
    modelPath.value = await invoke<string>("model_path", { size: selectedSize.value }).catch(() => "");
  }

  // Đổi size → reset trạng thái NGAY (tránh lóe "sẵn sàng" của size cũ) rồi kiểm tra lại model size mới.
  watch(selectedSize, () => {
    modelReady.value = false;
    downloading.value = false;
    downloadPct.value = 0;
    refreshModel();
  });

  async function init() {
    await refreshModel();
    unlisteners.push(
      await listen<Msg>("transcribe_msg", (e) => {
        const m = e.payload;
        switch (m.type) {
          case "download":
            if (!downloading.value) break; // download do tab khác khởi tạo → bỏ qua
            // pct của hub có thể nhảy (nhiều file) → giữ đơn điệu tăng.
            downloadPct.value = Math.max(downloadPct.value, m.pct ?? 0);
            break;
          case "model_ready":
            if (!downloading.value) break;
            // event không kèm size → xác minh trên đĩa theo size đang chọn (model_status per-size);
            // chỉ tắt downloading khi ĐÚNG model của tab này đã có (tránh tin nhầm download tab khác).
            refreshModel().then(() => { if (modelReady.value) downloading.value = false; });
            break;
          case "info":
            if (!transcribing.value) break; // info lúc nạp model của tab khác → bỏ qua
            deviceInfo.value = m.message ?? "";
            break;
          case "progress":
            if (!m.file || !owned.has(m.file)) break; // event của file tab khác / thiếu file → bỏ qua
            currentFile.value = m.file;
            progressPct.value = m.pct ?? 0;
            break;
          case "done":
            if (!m.file || !owned.has(m.file)) break;
            results.value.push({
              file: m.file,
              text: m.text ?? "",
              txtPath: m.txt_path ?? null,
              srtPath: m.srt_path ?? null,
            });
            panelText.value = m.text ?? "";
            currentFile.value = m.file;
            progressPct.value = 0; // file xong → reset phần file đang chạy (đã cộng vào settled)
            settled.value += 1;
            checkBatchEnd();
            break;
          case "error":
            if (m.file) {
              if (!owned.has(m.file)) break; // lỗi của file tab khác → bỏ qua
              // Lỗi 1 file trong batch → ghi nhận + settle, KHÔNG dừng cả batch.
              fileErrors.value.push({ file: m.file, message: m.message ?? "Lỗi" });
              progressPct.value = 0;
              settled.value += 1;
              checkBatchEnd();
            } else if (transcribing.value || downloading.value) {
              // Lỗi chung (model/spawn) — chỉ nhận khi tab này đang chạy (tránh nhận lỗi tab khác).
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
      await invoke("ensure_model", { size: selectedSize.value });
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
    settled.value = 0;
    expected.value = files.value.length;
    owned = new Set(files.value); // chỉ nhận event của đúng các file batch này
    transcribing.value = true;
    progressPct.value = 0;
    try {
      await invoke("transcribe_files", { paths: files.value, outputs: ["txt", "srt"], size: selectedSize.value });
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
    files, selectedSize, modelReady, modelPath, downloading, downloadPct, transcribing, progressPct,
    overallPct, doneCount: settled, total: expected,
    currentFile, deviceInfo, results, fileErrors, panelText, error,
    init, refreshModel, pickFiles, ensureModel, transcribe, cancel,
  };
}
