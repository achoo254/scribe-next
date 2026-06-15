import { createApp } from "vue";
import "./assets/styles/viui-tokens.css";
import "./styles.css";
import vuetify from "./plugins/vuetify";
import App from "./App.vue";
import RegionSelectorOverlay from "./views/RegionSelectorOverlay.vue";

// 2 cửa sổ dùng chung index.html này → chọn root mount theo label cửa sổ hiện tại.
// "region-selector" = overlay chọn vùng (trong suốt); còn lại = app chính.
(async () => {
  let label = "main";
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    label = getCurrentWindow().label;
  } catch {
    /* chạy frontend thuần (npm run dev, không có Tauri) → mặc định app chính */
  }

  if (label === "region-selector") {
    // Overlay cần nền trong suốt để thấy màn hình phía sau (không qua Vuetify theme).
    console.log("[onescribe] mounting RegionSelectorOverlay (label=region-selector)");
    document.documentElement.style.background = "transparent";
    document.body.style.background = "transparent";
    createApp(RegionSelectorOverlay).use(vuetify).mount("#app");
  } else {
    createApp(App).use(vuetify).mount("#app");
  }
})();
