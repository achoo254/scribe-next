// Tabler iconset cho Vuetify — registry import TƯỜNG MINH (tree-shake, không kéo cả 5000 icon).
// Honor mandate "icon = Tabler": cả glyph control Vuetify lẫn ViIcon đều dùng Tabler.
// Thêm icon mới: import + thêm vào `registry`. ViIcon dùng tên (vd <ViIcon icon="IconMicrophone" />).
import { h, type Component } from "vue";
import type { IconSet, IconAliases, IconProps } from "vuetify";
import {
  IconChevronUp, IconChevronDown, IconChevronLeft, IconChevronRight,
  IconChevronsLeft, IconChevronsRight, IconCheck, IconX, IconCircle,
  IconCircleCheck, IconCircleX, IconCircleDot, IconInfoCircle,
  IconAlertTriangle, IconAlertCircle, IconSquare, IconSquareCheck,
  IconSquareMinus, IconArrowUp, IconArrowDown, IconSelector, IconMenu2,
  IconPencil, IconStar, IconStarFilled, IconStarHalf, IconLoader2,
  IconPlus, IconMinus, IconPaperclip, IconCalendar,
  // app icons
  IconMicrophone, IconMicrophone2, IconScissors, IconFileText,
  IconFolder, IconPlayerPause, IconPlayerPlay, IconPlayerStop,
  IconCopy, IconDownload, IconRefresh, IconDeviceFloppy, IconTrash,
} from "@tabler/icons-vue";

const registry: Record<string, Component> = {
  IconChevronUp, IconChevronDown, IconChevronLeft, IconChevronRight,
  IconChevronsLeft, IconChevronsRight, IconCheck, IconX, IconCircle,
  IconCircleCheck, IconCircleX, IconCircleDot, IconInfoCircle,
  IconAlertTriangle, IconAlertCircle, IconSquare, IconSquareCheck,
  IconSquareMinus, IconArrowUp, IconArrowDown, IconSelector, IconMenu2,
  IconPencil, IconStar, IconStarFilled, IconStarHalf, IconLoader2,
  IconPlus, IconMinus, IconPaperclip, IconCalendar,
  IconMicrophone, IconMicrophone2, IconScissors, IconFileText,
  IconFolder, IconPlayerPause, IconPlayerPlay, IconPlayerStop,
  IconCopy, IconDownload, IconRefresh, IconDeviceFloppy, IconTrash,
};

// Vuetify alias -> tên trong registry.
export const aliases: IconAliases = {
  collapse: "IconChevronUp", complete: "IconCheck", cancel: "IconX",
  close: "IconX", delete: "IconCircleX", clear: "IconX",
  success: "IconCircleCheck", info: "IconInfoCircle",
  warning: "IconAlertTriangle", error: "IconAlertCircle",
  prev: "IconChevronLeft", next: "IconChevronRight",
  checkboxOn: "IconSquareCheck", checkboxOff: "IconSquare",
  checkboxIndeterminate: "IconSquareMinus", delimiter: "IconCircle",
  sortAsc: "IconArrowUp", sortDesc: "IconArrowDown",
  expand: "IconChevronDown", menu: "IconMenu2", subgroup: "IconChevronDown",
  dropdown: "IconChevronDown", radioOn: "IconCircleDot", radioOff: "IconCircle",
  edit: "IconPencil", ratingEmpty: "IconStar", ratingFull: "IconStarFilled",
  ratingHalf: "IconStarHalf", loading: "IconLoader2", first: "IconChevronsLeft",
  last: "IconChevronsRight", unfold: "IconSelector", file: "IconPaperclip",
  plus: "IconPlus", minus: "IconMinus", calendar: "IconCalendar",
} as unknown as IconAliases;

export const tabler: IconSet = {
  component: (props: IconProps) => {
    const name = typeof props.icon === "string" ? props.icon : "";
    const comp = registry[name];
    return comp ? h(comp, { size: "1.25em", stroke: 1.75 }) : h("span");
  },
};
