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
  IconMinus, IconCalendar, IconCheck,
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
  "arrow-up": IconArrowUp, "arrow-down": IconArrowDown, "menu-2": IconMenu2,
  selector: IconSelector, pencil: IconPencil, "loader-2": IconLoader2,
  paperclip: IconPaperclip, plus: IconPlus, minus: IconMinus,
  calendar: IconCalendar, check: IconCheck,
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
      radioOn: "circle-check",
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
