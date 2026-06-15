import { createApp } from "vue";
import "./assets/styles/viui-tokens.css";
import "./styles.css";
import vuetify from "./plugins/vuetify";
import App from "./App.vue";
import RegionSelectorOverlay from "./views/RegionSelectorOverlay.vue";
import RecordBorderOverlay from "./views/RecordBorderOverlay.vue";
import RecordBorderCloseButton from "./views/RecordBorderCloseButton.vue";

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

  // Cửa sổ overlay (trong suốt, không chrome): nền trong suốt + bỏ margin/scroll mặc định của body
  // (nếu không, cửa sổ nhỏ như nút ✕ sẽ hiện scrollbar do body margin/overflow rò vào).
  const overlayReset = () => {
    for (const el of [document.documentElement, document.body]) {
      el.style.background = "transparent";
      el.style.margin = "0";
      el.style.padding = "0";
      el.style.height = "100%";
      el.style.overflow = "hidden";
    }
  };

  if (label === "region-selector") {
    overlayReset();
    createApp(RegionSelectorOverlay).use(vuetify).mount("#app");
  } else if (label === "record-border") {
    overlayReset();
    createApp(RecordBorderOverlay).mount("#app");
  } else if (label === "record-border-close") {
    overlayReset();
    createApp(RecordBorderCloseButton).mount("#app");
  } else {
    createApp(App).use(vuetify).mount("#app");
  }
})();
