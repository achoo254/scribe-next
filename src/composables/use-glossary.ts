// Glossary (initial_prompt ASR) — bias model đúng tên riêng/thuật ngữ. Dùng chung Mode A + C.
// Lưu trong AppSettings.glossary; đọc/ghi MERGE qua settings-store (không đụng field khác).
import { computed, ref } from "vue";
import { loadSettings, patchSettings } from "./settings-store";

// Ngưỡng mềm ~100 token (đo trên tokenizer Whisper: ~2,7 ký tự/token với text Việt-Anh lẫn). Whisper
// chỉ giữ 223 token CUỐI của prompt và prompt ăn chung ngân sách 448 token với phần chép mỗi đoạn 30s →
// glossary dài làm mất chữ (đo thực tế: ~600 token → khớp 49% so với 67% khi không prompt).
export const GLOSSARY_SOFT_LIMIT = 280;

export function useGlossary() {
  const glossary = ref("");
  const tooLong = computed(() => glossary.value.trim().length > GLOSSARY_SOFT_LIMIT);

  async function load() {
    const s = await loadSettings();
    glossary.value = typeof s.glossary === "string" ? s.glossary : "";
  }

  async function save() {
    await patchSettings({ glossary: glossary.value.trim() || null });
  }

  return { glossary, tooLong, load, save };
}
