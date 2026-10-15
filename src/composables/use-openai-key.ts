// OpenAI API key cho nguồn chép cloud (Mode C). Key nằm trong OS keychain phía Rust (openai_key.rs);
// frontend chỉ biết có/không — KHÔNG bao giờ đọc lại giá trị key.
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

// gpt-4o-mini-transcribe — giá ước tính theo phút OpenAI công bố (bill thật tính theo token).
export const OPENAI_USD_PER_MIN = 0.003;

export function useOpenAiKey() {
  const hasKey = ref(false);
  const input = ref(""); // ô nhập — xoá ngay sau khi lưu
  const error = ref("");

  async function refresh() {
    hasKey.value = await invoke<boolean>("openai_key_status").catch(() => false);
  }

  async function save() {
    error.value = "";
    try {
      await invoke("set_openai_key", { key: input.value });
      input.value = "";
      await refresh();
    } catch (e) {
      error.value = String(e);
    }
  }

  async function remove() {
    error.value = "";
    try {
      await invoke("delete_openai_key");
      await refresh();
    } catch (e) {
      error.value = String(e);
    }
  }

  return { hasKey, input, error, refresh, save, remove };
}
