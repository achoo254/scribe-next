// Composable Mode A: bọc invoke + lắng nghe event (level/segment/rec_state/rec_error) + chọn thư mục.
import { ref, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

export interface DeviceInfo {
  id: string;
  name: string;
}
export interface SegmentFile {
  name: string;
  path: string;
}
export interface Level {
  rms: number;
  peak: number;
}
export type RecState = "idle" | "recording" | "paused";

export interface StartOpts {
  mic: string | null;
  outDir: string;
  format: string;
  splitValue: number;
  splitUnit: string;
}

export function useRecorder() {
  const devices = ref<DeviceInfo[]>([]);
  const state = ref<RecState>("idle");
  const level = ref<Level>({ rms: 0, peak: 0 });
  const segments = ref<SegmentFile[]>([]);
  const error = ref("");
  const unlisteners: UnlistenFn[] = [];

  async function init() {
    try {
      devices.value = await invoke<DeviceInfo[]>("list_devices");
    } catch (e) {
      error.value = String(e);
    }
    unlisteners.push(
      await listen<Level>("level", (e) => (level.value = e.payload)),
      await listen<SegmentFile>("segment_created", (e) => {
        if (!segments.value.some((s) => s.name === e.payload.name)) segments.value.push(e.payload);
      }),
      await listen<SegmentFile[]>("segments_final", (e) => (segments.value = e.payload)),
      await listen<string>("rec_state", (e) => (state.value = e.payload as RecState)),
      await listen<string>("rec_error", (e) => (error.value = e.payload)),
    );
  }

  async function pickFolder(): Promise<string | null> {
    const res = await open({ directory: true, multiple: false });
    return typeof res === "string" ? res : null;
  }

  async function start(opts: StartOpts) {
    error.value = "";
    segments.value = [];
    level.value = { rms: 0, peak: 0 };
    await invoke("start_recording", {
      mic: opts.mic,
      outDir: opts.outDir,
      format: opts.format,
      splitValue: opts.splitValue,
      splitUnit: opts.splitUnit,
    });
  }

  const pause = () => invoke("pause_recording");
  const resume = () => invoke("resume_recording");
  const stop = () => invoke("stop_recording");

  onUnmounted(() => unlisteners.forEach((u) => u()));

  return { devices, state, level, segments, error, init, pickFolder, start, pause, resume, stop };
}
