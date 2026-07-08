// Catalog model ASR — nguồn DUY NHẤT cho type + danh sách picker + hint dung lượng.
// Dùng chung RecordView (Mode A auto-transcript) + TranscriptView (Mode C batch) + 2 composable.
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

// ct2 (faster-whisper CPU int8): small/medium/large PhoWhisper.
// mlx (mlx-whisper Metal GPU): large-v3-mlx — chỉ macOS Apple Silicon.
export type ModelSize = "small" | "medium" | "large" | "large-v3-mlx";

export type SizeOption = { title: string; value: ModelSize };

const BASE_OPTIONS: SizeOption[] = [
  { title: "Small — nhanh nhất, chất lượng khá", value: "small" },
  { title: "Medium — cân bằng (mặc định)", value: "medium" },
  { title: "Large — chậm nhất, chất lượng cao nhất", value: "large" },
];

// Chỉ hiện trên macOS Apple Silicon (mlx_supported). Dùng Metal GPU → nhanh hơn ct2 CPU nhiều lần.
const MLX_OPTION: SizeOption = {
  title: "Whisper Large-v3 (MLX) — Apple Silicon, nhanh + chất lượng cao",
  value: "large-v3-mlx",
};

export const SIZE_HINT: Record<ModelSize, string> = {
  small: "~0.5GB",
  medium: "~1.5GB",
  large: "~3GB",
  "large-v3-mlx": "~3GB",
};

// sizeOptions là ref: khởi tạo với 3 model ct2, append MLX sau async check mlx_supported
// (khỏi chặn render). Trên Windows/Intel — hoặc khi invoke lỗi (dev browser) — MLX không xuất hiện.
export function useAsrModels() {
  const sizeOptions = ref<SizeOption[]>([...BASE_OPTIONS]);
  invoke<boolean>("mlx_supported")
    .then((ok) => {
      if (ok) sizeOptions.value = [...BASE_OPTIONS, MLX_OPTION];
    })
    .catch(() => {});
  return { sizeOptions, sizeHint: SIZE_HINT };
}
