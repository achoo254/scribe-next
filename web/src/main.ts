import { createApp } from "vue";
import App from "./App.vue";
import { router } from "./router";

// Vuetify 3 + iNET ViUI design system (Vi* là wrapper Vuetify → cần createVuetify).
import "vuetify/styles";
import { createVuetify } from "vuetify";
import * as components from "vuetify/components";
import * as directives from "vuetify/directives";
import { tabler } from "@hobui/viui/iconsets/tabler";
import "@hobui/viui/style.css"; // tokens --bsg-* + style Vi*

import "./style.css";

// Theme màu iNET — giá trị token chính thức lấy từ inet-viui MCP (get_brand_rules), map vào Vuetify.
const vuetify = createVuetify({
  components,
  directives,
  icons: { defaultSet: "tabler", sets: { tabler } },
  theme: {
    defaultTheme: "light",
    themes: {
      light: {
        colors: {
          primary: "#024799", // --bsg-primary (8.88:1 on white)
          secondary: "#4A6278", // --bsg-nl-600
          accent: "#cc0e0e", // --bsg-accent (logo/destructive)
          error: "#B91C1C",
          success: "#0E9F6E",
          warning: "#D47B0A",
          info: "#024799",
        },
      },
    },
  },
});

createApp(App).use(vuetify).use(router).mount("#app");
