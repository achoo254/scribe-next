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

export interface MonitorInfo {
  index: number;
  name: string;
  width: number;
  height: number;
}
export interface Region {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface StartOpts {
  source: string; // "mic" | "system"
  device: string | null; // tên thiết bị theo nguồn
  outDir: string;
  format: string;
  splitValue: number;
  splitUnit: string;
  // Ghi màn hình (tuỳ chọn).
  recordScreen?: boolean;
  screenMonitor?: number;
  screenFps?: number;
  region?: Region | null;
}

export function useRecorder() {
  const devices = ref<DeviceInfo[]>([]); // mic (input)
  const outputDevices = ref<DeviceInfo[]>([]); // output (render) — ghi tiếng hệ thống
  const monitors = ref<MonitorInfo[]>([]); // màn hình để ghi (ghi màn hình)
  const region = ref<Region | null>(null); // vùng đã chọn từ overlay
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
    // Output device (loopback) — rỗng trên non-Windows.
    outputDevices.value = await invoke<DeviceInfo[]>("list_output_devices").catch(() => []);
    monitors.value = await invoke<MonitorInfo[]>("list_monitors").catch(() => []);
    unlisteners.push(
      await listen<Level>("level", (e) => (level.value = e.payload)),
      await listen<SegmentFile>("segment_created", (e) => {
        if (!segments.value.some((s) => s.name === e.payload.name)) segments.value.push(e.payload);
      }),
      await listen<SegmentFile[]>("segments_final", (e) => (segments.value = e.payload)),
      await listen<string>("rec_state", (e) => (state.value = e.payload as RecState)),
      await listen<string>("rec_error", (e) => (error.value = e.payload)),
      await listen<Region>("region_selected", (e) => (region.value = e.payload)),
      // Kéo thanh công cụ di chuyển khung → Rust emit vùng mới khi thả → cập nhật + lưu.
      await listen<Region>("region_moved", (e) => (region.value = e.payload)),
      await listen("region_cancelled", () => {}),
      // Nút ✕ trên khung → Rust emit "region_cleared" → xoá vùng (watch sẽ ẩn khung + nút).
      await listen("region_cleared", () => (region.value = null)),
    );
  }

  // Mở overlay chọn vùng trên `monitor` (kết quả về qua event region_selected → region.value).
  // Lỗi command (vd window build fail / Rust chưa rebuild) phải hiện lên UI, không nuốt im lặng.
  async function openRegionSelector(monitor: number) {
    try {
      await invoke("open_region_selector", { monitor });
    } catch (e) {
      error.value = `Không mở được cửa sổ chọn vùng: ${String(e)}`;
    }
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
      source: opts.source,
      device: opts.device,
      outDir: opts.outDir,
      format: opts.format,
      splitValue: opts.splitValue,
      splitUnit: opts.splitUnit,
      recordScreen: opts.recordScreen ?? false,
      screenMonitor: opts.screenMonitor ?? 0,
      screenFps: opts.screenFps ?? 30,
      region: opts.region ?? null,
    });
  }

  const pause = () => invoke("pause_recording");
  const resume = () => invoke("resume_recording");
  const stop = () => invoke("stop_recording");

  onUnmounted(() => unlisteners.forEach((u) => u()));

  return {
    devices, outputDevices, monitors, region, state, level, segments, error,
    init, pickFolder, openRegionSelector, start, pause, resume, stop,
  };
}
