import { createApp, h } from "vue";
import App from "./App.vue";
import { router } from "./router";

// Vuetify 3 + iNET ViUI design system (Vi* là wrapper Vuetify → cần createVuetify).
import "vuetify/styles";
import { createVuetify, type IconSet, type IconProps } from "vuetify";
import * as components from "vuetify/components";
import * as directives from "vuetify/directives";
import {
  IconChevronDown, IconChevronUp, IconChevronLeft, IconChevronRight,
  IconChevronsLeft, IconChevronsRight, IconCircleCheck, IconCircleX, IconX,
  IconInfoCircle, IconAlertTriangle, IconAlertCircle, IconSquare, IconSquareCheck,
  IconSquareMinus, IconCircle, IconCircleFilled, IconArrowUp, IconArrowDown,
  IconMenu2, IconSelector, IconPencil, IconLoader2, IconPaperclip, IconPlus,
  IconMinus, IconCalendar, IconCheck, IconCircleDot,
  // app icons (khớp desktop src/plugins/tabler-iconset.ts)
  IconMicrophone, IconScissors, IconFileText, IconFolder, IconDownload,
  IconLogout, IconBrandGoogle, IconPlayerStop,
  // media player (audio tùy biến — Lịch sử ghi)
  IconPlayerPlayFilled, IconPlayerPauseFilled, IconVolume, IconVolumeOff,
} from "@tabler/icons-vue";
import "@hobui/viui/style.css"; // tokens --bsg-* + style Vi*

import "./style.css";

// Iconset tự chủ backed @tabler/icons-vue — set tabler của @hobui dùng key opaque nên "?" ở
// chevron/alert. Chỉ import icon cần dùng (bundle nhỏ); alias Vuetify trỏ tên kebab dưới đây.
const TABLER: Record<string, unknown> = {
  "chevron-down": IconChevronDown, "chevron-up": IconChevronUp,
  "chevron-left": IconChevronLeft, "chevron-right": IconChevronRight,
  "chevrons-left": IconChevronsLeft, "chevrons-right": IconChevronsRight,
  "circle-check": IconCircleCheck, "circle-x": IconCircleX, x: IconX,
  "info-circle": IconInfoCircle, "alert-triangle": IconAlertTriangle,
  "alert-circle": IconAlertCircle, square: IconSquare, "square-check": IconSquareCheck,
  "square-minus": IconSquareMinus, circle: IconCircle, "circle-filled": IconCircleFilled,
  "circle-dot": IconCircleDot,
  "arrow-up": IconArrowUp, "arrow-down": IconArrowDown, "menu-2": IconMenu2,
  selector: IconSelector, pencil: IconPencil, "loader-2": IconLoader2,
  paperclip: IconPaperclip, plus: IconPlus, minus: IconMinus,
  calendar: IconCalendar, check: IconCheck,
  // app icons (tabs / logout / google / split)
  microphone: IconMicrophone, scissors: IconScissors, "file-text": IconFileText,
  folder: IconFolder, download: IconDownload, logout: IconLogout,
  "brand-google": IconBrandGoogle, "player-stop": IconPlayerStop,
  // media player audio tùy biến
  "player-play": IconPlayerPlayFilled, "player-pause": IconPlayerPauseFilled,
  volume: IconVolume, "volume-off": IconVolumeOff,
};
const tablerSet: IconSet = {
  component: (props: IconProps) =>
    h((TABLER[String(props.icon)] ?? IconCircle) as never, { size: 20, stroke: 2 }),
};

// Theme màu iNET — giá trị token chính thức lấy từ inet-viui MCP (get_brand_rules), map vào Vuetify.
const vuetify = createVuetify({
  components,
  directives,
  // Map alias icon Vuetify (mặc định trỏ tên mdi) → tên tabler kebab để tabler set resolve được
  // (nếu không, ViSelect chevron/ViAlert success hiện "?").
  icons: {
    defaultSet: "tabler",
    sets: { tabler: tablerSet },
    aliases: {
      complete: "check",
      cancel: "circle-x",
      close: "x",
      delete: "circle-x",
      clear: "circle-x",
      success: "circle-check",
      info: "info-circle",
      warning: "alert-triangle",
      error: "alert-circle",
      prev: "chevron-left",
      next: "chevron-right",
      checkboxOn: "square-check",
      checkboxOff: "square",
      checkboxIndeterminate: "square-minus",
      delimiter: "circle-filled",
      sortAsc: "arrow-up",
      sortDesc: "arrow-down",
      expand: "chevron-down",
      menu: "menu-2",
      subgroup: "chevron-down",
      dropdown: "chevron-down",
      radioOn: "circle-dot",
      radioOff: "circle",
      edit: "pencil",
      loading: "loader-2",
      first: "chevrons-left",
      last: "chevrons-right",
      unfold: "selector",
      file: "paperclip",
      plus: "plus",
      minus: "minus",
      calendar: "calendar",
      collapse: "chevron-up",
    },
  },
  // Theme One Series dark (khớp desktop src/plugins/vuetify.ts + src/tokens/one-series.ts).
  // Giá trị = token One Series (vendored ViUI MCP): primary xanh lá, accent cam, nền tối.
  theme: {
    defaultTheme: "inetOneDark",
    themes: {
      inetOneDark: {
        dark: true,
        colors: {
          background: "#0d0d0d", // products-one-bg
          surface: "#1a1a1a", // products-one-bg-card
          "surface-bright": "#2e2e2e", // products-one-bg-border
          "surface-variant": "#2e2e2e",
          primary: "#49c98d", // products-one-green
          secondary: "#f7ad30", // products-one-orange
          accent: "#f7ad30", // products-one-orange
          error: "#f87171",
          info: "#619ee9",
          success: "#34d399",
          warning: "#fbbf24",
          "on-background": "#e0e0e0",
          "on-surface": "#e0e0e0",
          "on-primary": "#0d0d0d",
          "on-secondary": "#0d0d0d",
        },
      },
    },
  },
  // Font khớp desktop (Be Vietnam Pro — load qua Google Fonts ở index.html).
  defaults: {
    global: { style: "font-family: 'Be Vietnam Pro', sans-serif" },
  },
});

createApp(App).use(vuetify).use(router).mount("#app");
