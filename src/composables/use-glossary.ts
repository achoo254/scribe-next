// Glossary (initial_prompt ASR) — bias model đúng tên riêng/thuật ngữ. Dùng chung Mode A + C.
// Lưu trong AppSettings.glossary; đọc/ghi MERGE qua settings-store (không đụng field khác).
import { ref } from "vue";
import { loadSettings, patchSettings } from "./settings-store";

export function useGlossary() {
  const glossary = ref("");

  async function load() {
    const s = await loadSettings();
    glossary.value = typeof s.glossary === "string" ? s.glossary : "";
  }

  async function save() {
    await patchSettings({ glossary: glossary.value.trim() || null });
  }

  return { glossary, load, save };
}
