// Composable Mode C: chọn file + tải model + transcribe batch + lắng nghe sidecar (transcribe_msg).
// Kết quả giữ per-file (segments cho player click-to-seek + text sửa được + lưu lại). Export nhiều
// định dạng (txt/srt/vtt/json). Retry file lỗi. Progress tổng cả batch.
import { ref, computed, watch, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type { ModelSize } from "./asr-models";

export interface Segment {
  start: number;
  end: number;
  text: string;
}
export interface TransResult {
  file: string;
  text: string; // sửa được trong UI
  segments: Segment[];
  txtPath: string | null;
  srtPath: string | null;
  vttPath: string | null;
  jsonPath: string | null;
  dirty: boolean; // đã sửa nhưng chưa lưu
  saved: boolean; // vừa lưu xong (hiện dấu "đã lưu")
}
interface Msg {
  type: string;
  pct?: number;
  file?: string;
  text?: string;
  segments?: Segment[];
  txt_path?: string | null;
  srt_path?: string | null;
  vtt_path?: string | null;
  json_path?: string | null;
  message?: string;
}

const AUDIO_EXT = ["mp3", "m4a", "wav", "aac", "flac", "ogg", "opus"];
// Video: faster-whisper (PyAV) tự rút track âm thanh để transcribe — không cần tách audio trước.
const VIDEO_EXT = ["mp4", "mkv", "mov", "avi", "webm", "m4v", "ts", "flv", "wmv", "mpg", "mpeg"];

const ext = (p: string): string => (p.split(".").pop() || "").toLowerCase();
export const isVideoPath = (p: string): boolean => VIDEO_EXT.includes(ext(p));
// Đường .txt cạnh file gốc (khi user không chọn xuất txt vẫn lưu được bản sửa).
const deriveTxtPath = (p: string): string => p.replace(/\.[^.]+$/, "") + ".txt";

export function useTranscribe() {
  const files = ref<string[]>([]);
  const selectedSize = ref<ModelSize>("medium"); // mặc định medium (cân bằng tốc độ/chất lượng)
  const exportFormats = ref<string[]>(["txt", "srt"]); // định dạng xuất (Phase 02)
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
  const error = ref("");
  const unlisteners: UnlistenFn[] = [];
  const expected = ref(0); // tổng số file của batch
  const settled = ref(0); // số file đã xong (done + error)
  // File mà CHÍNH composable này gửi đi — lọc transcribe_msg để không "nhận nhầm" event của
  // tab khác (Mode A tự-động-transcript dùng chung sidecar + event). Cần khi App bật keep-alive.
  let owned = new Set<string>();

  // outputs gửi sidecar: luôn đảm bảo có ít nhất "txt" (để có bản lưu/sửa mặc định).
  const outputs = (): string[] => (exportFormats.value.length ? exportFormats.value : ["txt"]);

  function checkBatchEnd() {
    if (settled.value >= expected.value) transcribing.value = false;
  }

  // Tiến độ TỔNG cả batch: mỗi file chiếm 1/expected; file đang chạy góp progressPct phần trăm của nó.
  const overallPct = computed(() =>
    expected.value
      ? Math.min(100, Math.round((settled.value * 100 + progressPct.value) / expected.value))
      : 0,
  );

  // MLX transcribe là 1 lời gọi blocking, KHÔNG phát progress từng đoạn → overallPct đứng 0 tới khi
  // done (trông như treo). Hiện thanh indeterminate = "đang chạy". Cũng phủ lúc ct2 mới khởi động.
  const indeterminate = computed(() => transcribing.value && overallPct.value === 0);

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
            refreshModel().then(() => {
              if (modelReady.value) downloading.value = false;
            });
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
          case "done": {
            if (!m.file || !owned.has(m.file)) break;
            const done: TransResult = {
              file: m.file,
              text: m.text ?? "",
              segments: m.segments ?? [],
              txtPath: m.txt_path ?? null,
              srtPath: m.srt_path ?? null,
              vttPath: m.vtt_path ?? null,
              jsonPath: m.json_path ?? null,
              dirty: false,
              saved: false,
            };
            // Upsert theo file → done trùng / retry lại KHÔNG tạo card nhân đôi.
            const idx = results.value.findIndex((r) => r.file === done.file);
            if (idx >= 0) results.value[idx] = done;
            else results.value.push(done);
            fileErrors.value = fileErrors.value.filter((e) => e.file !== done.file); // retry OK → xoá lỗi cũ
            currentFile.value = m.file;
            progressPct.value = 0; // file xong → reset phần file đang chạy (đã cộng vào settled)
            settled.value += 1;
            checkBatchEnd();
            break;
          }
          case "error":
            if (m.file) {
              if (!owned.has(m.file)) break; // lỗi của file tab khác → bỏ qua
              // Lỗi 1 file trong batch → ghi nhận + settle, KHÔNG dừng cả batch. Dedup theo file
              // (retry lỗi lại → thay entry cũ, không chồng).
              fileErrors.value = fileErrors.value.filter((e) => e.file !== m.file);
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
    const res = await open({
      multiple: true,
      filters: [
        { name: "Âm thanh & Video", extensions: [...AUDIO_EXT, ...VIDEO_EXT] },
        { name: "Âm thanh", extensions: AUDIO_EXT },
        { name: "Video", extensions: VIDEO_EXT },
      ],
    });
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

  // Bắt đầu 1 batch (danh sách paths) — dùng chung cho transcribe() và retry().
  async function runBatch(paths: string[]) {
    error.value = "";
    settled.value = 0;
    expected.value = paths.length;
    owned = new Set(paths); // chỉ nhận event của đúng các file batch này
    transcribing.value = true;
    progressPct.value = 0;
    try {
      await invoke("transcribe_files", { paths, outputs: outputs(), size: selectedSize.value });
    } catch (e) {
      error.value = String(e);
      transcribing.value = false;
    }
  }

  async function transcribe() {
    if (!files.value.length) {
      error.value = "Hãy chọn ít nhất 1 file.";
      return;
    }
    results.value = [];
    fileErrors.value = [];
    await runBatch([...files.value]);
  }

  // Thử lại 1 file lỗi (Phase 04) — chạy như batch 1 file. GIỮ entry lỗi cũ tới khi có kết quả:
  // done → xoá; lỗi-file → thay; lỗi CHUNG (fileless: model bị xoá/spawn fail) → entry cũ còn đó
  // (file không biến mất khỏi UI, vẫn còn nút thử lại).
  async function retry(file: string) {
    await runBatch([file]);
  }

  // Lưu bản transcript đã sửa → ghi đè .txt (tạo cạnh file gốc nếu chưa xuất txt).
  async function save(r: TransResult) {
    const path = r.txtPath ?? deriveTxtPath(r.file);
    try {
      await invoke("save_transcript", { path, content: r.text });
      r.txtPath = path;
      r.dirty = false;
      r.saved = true;
    } catch (e) {
      error.value = String(e);
    }
  }

  // Đánh dấu đã sửa (view gọi khi textarea đổi).
  function edit(r: TransResult, value: string) {
    r.text = value;
    r.dirty = true;
    r.saved = false;
  }

  const cancel = () => {
    invoke("cancel_transcribe").catch(() => {});
    transcribing.value = false;
    downloading.value = false;
  };

  onUnmounted(() => unlisteners.forEach((u) => u()));

  return {
    files, selectedSize, exportFormats, modelReady, modelPath, downloading, downloadPct,
    transcribing, progressPct, overallPct, indeterminate, doneCount: settled, total: expected,
    currentFile, deviceInfo, results, fileErrors, error,
    init, refreshModel, pickFiles, ensureModel, transcribe, retry, save, edit, cancel,
  };
}
