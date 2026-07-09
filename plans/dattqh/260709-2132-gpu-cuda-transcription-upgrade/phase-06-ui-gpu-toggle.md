# Phase 06 — UI: Toggle GPU + tải CUDA runtime

## Context Links
- Overview: [plan.md](plan.md)
- Depends on: [phase-03](phase-03-model-whisper-large-v3-turbo.md) (turbo tự hiện qua `sizeOptions`, không cần code riêng), [phase-04](phase-04-rust-cuda-plumbing.md) (cần 3 command + field settings `use_gpu`)

## Overview
- Priority: P2
- Status: pending
- Thêm khối UI trong `TranscriptView.vue` (theo đúng vị trí/pattern glossary hiện có — 1 chỗ sửa, áp dụng chung Mode A + C qua settings): toggle "Dùng GPU (CUDA)" — chỉ hiện khi `cuda_supported()`=true; kèm trạng thái CUDA runtime (đã tải / cần tải + nút tải + progress bar), pattern giống hệt khối "tải model" đã có (dòng 104-125 hiện tại).

## Key Insights
- Turbo model KHÔNG cần code UI riêng — đã tự xuất hiện trong `sizeOptions` (từ `useAsrModels()`, Phase 03 sửa `BASE_OPTIONS`). Phase này CHỈ thêm phần GPU.
- Gating hiển thị toggle GPU dùng đúng pattern `mlx_supported` trong `useAsrModels()` (asr-models.ts dòng 33-41): invoke async, check `.catch(() => {})` để không crash khi invoke lỗi (dev trong browser không có Tauri context).
- GPU là setting TOÀN CỤC (áp Mode A + C, giống glossary — comment `use-glossary.ts` dòng 1 "Dùng chung Mode A + C") — chỉ cần sửa UI ở TranscriptView.vue (Mode C), Mode A tự động dùng theo (Rust đọc `settings.use_gpu` trong `Transcriber::transcribe()`, không phân biệt tab gọi).
- CUDA runtime download dùng KÊNH SỰ KIỆN riêng biệt (`cuda_download`/`cuda_ready`, KHÔNG trùng `download`/`model_ready` của model — xem Phase 04) — composable mới `use-cuda.ts` lắng nghe cùng event `transcribe_msg` (kênh duy nhất Rust emit, xem `transcribe.rs` reader thread) nhưng filter theo `type` khác, độc lập hoàn toàn với `use-transcribe.ts`/`use-record-transcribe.ts` (không sửa 2 file đó).
- Toggle bật GPU nhưng CUDA runtime CHƯA tải → sidecar tự fallback CPU (Phase 02 đã xử lý), UI KHÔNG cần block hành động transcribe — chỉ hiển thị rõ trạng thái (giống pattern "model chưa tải vẫn cho thấy nút Transcribe bị disable qua `!tr.modelReady.value`" — nhưng GPU KHÁC: không disable gì, vì fallback CPU vẫn chạy được, chỉ là chậm hơn dự kiến). Hiển thị cảnh báo nhẹ, không chặn.

## Requirements
### Functional
- Composable mới `src/composables/use-gpu.ts`: gộp 2 trách nhiệm (đặt cùng file vì cùng 1 khái niệm "GPU" trên UI, tránh tạo quá nhiều file nhỏ — DRY/KISS):
  - `useGpuSetting()`: đọc/ghi `AppSettings.use_gpu` qua `settings-store` (mirror `use-glossary.ts`).
  - `useCudaRuntime()`: `cudaSupported` (ref, invoke `cuda_supported` 1 lần lúc mounted), `ready` (invoke `cuda_runtime_status`), `downloading`/`downloadPct` (nghe `transcribe_msg` type `cuda_download`/`cuda_ready`), `ensure()` (invoke `ensure_cuda_runtime`).
- `TranscriptView.vue`: thêm khối UI sau phần chọn model (trước hoặc sau glossary — đặt ngay dưới model picker, cùng nhóm "cấu hình nhận dạng"): `ViSwitch` bound `gpu.useGpu`, chỉ render khối này nếu `cuda.cudaSupported`; trong khối, nếu `!cuda.ready` → `ViAlert warning` + `ViButton "Tải CUDA runtime (~1.3GB)"` + `ViProgressLinear` (pattern y hệt khối model dòng 107-125); nếu `cuda.ready` → `ViAlert success` ngắn "CUDA runtime sẵn sàng".

### Non-functional
- KHÔNG sửa `use-transcribe.ts`/`use-record-transcribe.ts`/`AsrModelControls.vue`/`RecordView.vue` — GPU setting đọc thẳng từ Rust ở tầng `Transcriber::transcribe()`, frontend không cần forward gì thêm cho Mode A.
- Theo đúng token/wrapper `Vi*` (README: "UI bắt buộc theo iNET ViUI... không hardcode màu/spacing").

## Architecture
```
use-gpu.ts
  useGpuSetting() ─── load/save AppSettings.use_gpu (settings-store, mirror use-glossary.ts)
  useCudaRuntime()
     ├─ onMounted: invoke("cuda_supported") → cudaSupported
     ├─ invoke("cuda_runtime_status") → ready
     ├─ listen("transcribe_msg"):
     │     type="cuda_download" → downloading=true, downloadPct=max(cur,pct)
     │     type="cuda_ready"    → ready=true, downloading=false
     └─ ensure(): downloading=true; invoke("ensure_cuda_runtime")

TranscriptView.vue
  onMounted: gpu.load(); cudaRt.init()
  <ViSwitch v-if="cudaRt.cudaSupported" v-model="gpu.useGpu" label="Dùng GPU (CUDA)" @update:model-value="gpu.save()" />
  <template v-if="cudaRt.cudaSupported && gpu.useGpu">
     ready ? ViAlert success : (ViAlert warning + nút tải + progress)
  </template>
```

## Related Code Files
- **Create:** `src/composables/use-gpu.ts`
- **Modify:** `src/views/TranscriptView.vue` (script setup: import + init; template: thêm khối sau model picker dòng ~70)

## Implementation Steps
1. Tạo `use-gpu.ts`:
   ```ts
   // Setting GPU (CUDA) toàn cục — dùng chung Mode A + C, mirror use-glossary.ts.
   import { ref, onUnmounted } from "vue";
   import { invoke } from "@tauri-apps/api/core";
   import { listen, type UnlistenFn } from "@tauri-apps/api/event";
   import { loadSettings, patchSettings } from "./settings-store";

   export function useGpuSetting() {
     const useGpu = ref(false);
     async function load() {
       const s = await loadSettings();
       useGpu.value = typeof s.use_gpu === "boolean" ? s.use_gpu : false;
     }
     async function save() {
       await patchSettings({ use_gpu: useGpu.value });
     }
     return { useGpu, load, save };
   }

   export function useCudaRuntime() {
     const cudaSupported = ref(false);
     const ready = ref(false);
     const downloading = ref(false);
     const downloadPct = ref(0);
     const error = ref("");
     const unlisteners: UnlistenFn[] = [];

     async function init() {
       cudaSupported.value = await invoke<boolean>("cuda_supported").catch(() => false);
       if (!cudaSupported.value) return;
       ready.value = await invoke<boolean>("cuda_runtime_status").catch(() => false);
       unlisteners.push(
         await listen<{ type: string; pct?: number }>("transcribe_msg", (e) => {
           const m = e.payload;
           if (m.type === "cuda_download") {
             downloading.value = true;
             downloadPct.value = Math.max(downloadPct.value, m.pct ?? 0);
           } else if (m.type === "cuda_ready") {
             ready.value = true;
             downloading.value = false;
           }
         }),
       );
     }

     async function ensure() {
       error.value = "";
       downloading.value = true;
       downloadPct.value = 0;
       try {
         await invoke("ensure_cuda_runtime");
       } catch (e) {
         error.value = String(e);
         downloading.value = false;
       }
     }

     onUnmounted(() => unlisteners.forEach((u) => u()));
     return { cudaSupported, ready, downloading, downloadPct, error, init, ensure };
   }
   ```
2. `TranscriptView.vue` script: thêm import `useGpuSetting, useCudaRuntime` từ `../composables/use-gpu`; khai báo `const gpu = useGpuSetting(); const cudaRt = useCudaRuntime();`; trong `onMounted` (dòng 19-23) thêm `gpu.load(); cudaRt.init();`.
3. `TranscriptView.vue` template: chèn ngay sau khối `ViSelect` model picker (sau dòng 70, trước khối glossary dòng 72):
   ```html
   <template v-if="cudaRt.cudaSupported.value">
     <ViSwitch
       v-model="gpu.useGpu.value"
       label="Dùng GPU (CUDA) — nhanh hơn nhiều, cần tải runtime lần đầu"
       hide-details
       @update:model-value="gpu.save()"
     />
     <template v-if="gpu.useGpu.value">
       <ViAlert v-if="cudaRt.ready.value" type="success" variant="tonal" density="compact">
         CUDA runtime sẵn sàng.
       </ViAlert>
       <template v-else>
         <ViAlert type="warning" variant="tonal" density="compact">
           Chưa tải CUDA runtime (~1.3GB) — cần tải 1 lần, sau đó dùng offline. Chưa tải thì transcribe vẫn chạy (CPU, chậm hơn).
         </ViAlert>
         <div class="os-row">
           <ViButton color="secondary" :disabled="cudaRt.downloading.value" @click="cudaRt.ensure()">
             <ViIcon icon="IconDownload" start />Tải CUDA runtime
           </ViButton>
           <span v-if="cudaRt.downloading.value" class="os-muted">{{ cudaRt.downloadPct.value }}%</span>
         </div>
         <ViProgressLinear
           v-if="cudaRt.downloading.value"
           :model-value="cudaRt.downloadPct.value"
           :indeterminate="cudaRt.downloadPct.value === 0"
           color="secondary"
           height="6"
           rounded
         />
       </template>
       <ViAlert v-if="cudaRt.error.value" type="error" density="compact" closable>{{ cudaRt.error.value }}</ViAlert>
     </template>
   </template>
   ```
4. Import `ViSwitch` vào danh sách import `vi-ui` đầu file (dòng 7-10) nếu chưa có.
5. `npm run build` (vue-tsc) → xanh.

## Todo List
- [ ] Tạo `use-gpu.ts` (2 composable)
- [ ] `TranscriptView.vue`: wiring script + template
- [ ] `npm run build` pass
- [ ] Test UI trên máy có GPU: toggle hiện, tải CUDA runtime, progress chạy, xong hiện "sẵn sàng"
- [ ] Test UI trên máy giả lập không GPU (hoặc build check code path `cudaSupported=false`): khối GPU không hiện gì cả — không đổi UI so với trước phase này

## Success Criteria
- Máy không NVIDIA: UI KHÔNG hiện gì thêm — giao diện y hệt trước phase này (đúng yêu cầu "máy không NVIDIA không thấy gì thay đổi").
- Máy có GPU: toggle hiện, bật lên + tải xong → transcribe dùng GPU (kiểm chéo qua `deviceInfo` hiển thị sẵn có trong `TranscriptView.vue` dòng 146, nay sẽ hiện "Đang dùng CUDA (float16)" từ info message Phase 02).
- Tắt toggle → quay lại CPU int8 ngay lần transcribe tiếp theo (không cần restart app).

## Risk Assessment
| Risk | Likelihood × Impact | Mitigation |
|---|---|---|
| `deviceInfo` (use-transcribe.ts dòng 121-124) chỉ nhận info message khi `transcribing.value=true` — info "Đang dùng CUDA" gửi lúc get_model có thể tới TRƯỚC khi transcribing set true (race) | TB × Thấp | không sửa (ngoài scope UI phase này) — nếu miss, deviceInfo chỉ không hiện tức thời, không ảnh hưởng kết quả transcribe; ghi nhận unresolved question |
| User bật toggle rồi thoát app giữa lúc đang tải CUDA runtime | TB × Thấp | tải lại từ đầu lần sau (Phase 01 không resume) — chấp nhận, giống hành vi tải model hiện tại cũng không resume |

## Security Considerations
- Không áp dụng — thuần UI, không xử lý input nhạy cảm.

## Next Steps
- Phase 07 verify toàn luồng UI→Rust→sidecar trên máy RTX 5070 Ti thật.

## Unresolved Questions
1. Race điều kiện `deviceInfo` (xem Risk) — chấp nhận được, không chặn ship; có thể vá riêng nếu user thấy khó chịu khi dùng thật.
