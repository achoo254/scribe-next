// Composable: tự động transcribe các đoạn ghi (Mode A) — tái dùng sidecar Mode C (model giữ ấm).
// Khác use-transcribe: không gom batch + panelText, mà map trạng thái theo TỪNG đoạn (key = path).
import { ref, watch, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ModelSize } from "./asr-models";

export type SegStatus = "pending" | "transcribing" | "done" | "error";

export interface SegTranscript {
  status: SegStatus;
  pct: number;
  text: string;
  txtPath: string | null;
  srtPath: string | null;
  error: string;
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

function blank(): SegTranscript {
  return { status: "pending", pct: 0, text: "", txtPath: null, srtPath: null, error: "" };
}

export function useRecordTranscribe() {
  const enabled = ref(false);
  const selectedSize = ref<ModelSize>("medium"); // cân bằng tốc độ/chất lượng
  const modelReady = ref(false);
  const modelPath = ref("");
  const downloading = ref(false);
  const downloadPct = ref(0);
  const deviceInfo = ref("");
  const error = ref("");
  // Trạng thái transcript theo đường dẫn đoạn ghi (giữ thứ tự enqueue).
  const transcripts = ref<Record<string, SegTranscript>>({});
  const unlisteners: UnlistenFn[] = [];

  // Tạo entry nếu chưa có — CHỈ dùng ở enqueue (nơi khởi tạo đoạn của phiên hiện tại).
  function entry(file?: string): SegTranscript | null {
    if (!file) return null;
    if (!transcripts.value[file]) transcripts.value[file] = blank();
    return transcripts.value[file];
  }
  // Tra entry CÓ SẴN (không tạo mới) — handler event dùng cái này để bỏ qua message của
  // đoạn phiên cũ (sidecar có thể còn xả hàng đợi sau khi reset), tránh rò entry chéo phiên.
  function find(file?: string): SegTranscript | null {
    return file ? transcripts.value[file] ?? null : null;
  }

  // Kiểm tra model size đang chọn đã tải chưa + đường dẫn lưu.
  async function refreshModel() {
    modelReady.value = await invoke<boolean>("model_status", { size: selectedSize.value }).catch(() => false);
    modelPath.value = await invoke<string>("model_path", { size: selectedSize.value }).catch(() => "");
  }

  // Đổi size → reset trạng thái model ngay (tránh lóe "sẵn sàng" của size cũ).
  watch(selectedSize, () => {
    modelReady.value = false;
    downloading.value = false;
    downloadPct.value = 0;
    error.value = "";
    refreshModel();
  });

  async function init() {
    await refreshModel();
    unlisteners.push(
      await listen<Msg>("transcribe_msg", (e) => {
        const m = e.payload;
        switch (m.type) {
          case "download":
            if (!downloading.value) break; // download do tab khác (Mode C) khởi tạo → bỏ qua
            downloadPct.value = Math.max(downloadPct.value, m.pct ?? 0);
            break;
          case "model_ready":
            if (!downloading.value) break;
            // event không kèm size → xác minh trên đĩa theo size đang chọn (model_status per-size);
            // chỉ tắt downloading khi ĐÚNG model của tab này đã có (tránh tin nhầm download tab khác).
            refreshModel().then(() => { if (modelReady.value) downloading.value = false; });
            break;
          case "info":
            deviceInfo.value = m.message ?? "";
            break;
          case "progress": {
            const t = find(m.file);
            if (t) {
              t.status = "transcribing";
              t.pct = m.pct ?? 0;
            }
            break;
          }
          case "done": {
            const t = find(m.file);
            if (t) {
              t.status = "done";
              t.pct = 100;
              t.text = m.text ?? "";
              t.txtPath = m.txt_path ?? null;
              t.srtPath = m.srt_path ?? null;
            }
            break;
          }
          case "error":
            if (m.file) {
              const t = find(m.file);
              if (t) {
                t.status = "error";
                t.error = m.message ?? "Lỗi";
              }
            } else {
              // Lỗi chung (model/spawn) → báo + dừng download.
              error.value = m.message ?? "Lỗi không xác định";
              downloading.value = false;
            }
            break;
        }
      }),
    );
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

  // Đẩy 1 đoạn (đã finalize) vào sidecar — xử lý tuần tự, model giữ ấm.
  // Dedup theo path: đang xử lý/đã xong → bỏ; lỗi trước đó → CHO thử lại (lỗi transient).
  // Bất biến quan trọng: tạo/đặt entry "pending" ĐỒNG BỘ trước await → nếu flushFinalized
  // bắn nhiều lần trong cùng tick (4 watcher), lần sau thấy "pending" sẽ bỏ qua, không gửi trùng.
  async function enqueue(path: string) {
    if (!enabled.value || !modelReady.value) return;
    const existing = transcripts.value[path];
    if (existing && existing.status !== "error") return;
    const t = entry(path)!;
    t.status = "pending";
    t.pct = 0;
    t.error = "";
    try {
      await invoke("transcribe_files", { paths: [path], outputs: ["txt", "srt"], size: selectedSize.value });
    } catch (e) {
      t.status = "error";
      t.error = String(e);
    }
  }

  function reset() {
    transcripts.value = {};
    error.value = "";
  }

  onUnmounted(() => unlisteners.forEach((u) => u()));

  return {
    enabled, selectedSize, modelReady, modelPath, downloading, downloadPct, deviceInfo, error,
    transcripts, init, refreshModel, ensureModel, enqueue, reset,
  };
}
