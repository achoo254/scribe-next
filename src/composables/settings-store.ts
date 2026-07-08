// Cầu nối AppSettings (get_settings/set_settings) với ngữ nghĩa MERGE.
// set_settings ghi TOÀN BỘ AppSettings; nếu 1 tab dựng object thiếu field, serde default sẽ nhồi
// lại giá trị mặc định → xóa mất field tab khác vừa đổi (vd glossary). patchSettings đọc bản hiện
// tại rồi overlay patch → chỉ field trong patch thay đổi, phần còn lại giữ nguyên.
import { invoke } from "@tauri-apps/api/core";

export type Settings = Record<string, unknown>;

export async function loadSettings(): Promise<Settings> {
  return invoke<Settings>("get_settings").catch(() => ({}));
}

export async function patchSettings(patch: Settings): Promise<void> {
  const cur = await loadSettings();
  await invoke("set_settings", { settings: { ...cur, ...patch } }).catch(() => {});
}
