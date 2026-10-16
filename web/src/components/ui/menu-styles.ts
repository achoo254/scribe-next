/* Copied from localman-next apps/web/src/components/ui/menu-styles.ts (same design
   system). Keep in sync with the source; change it here only when Scribe
   needs to, and say why. */
import { cn } from "../../lib/cn";

/** Panel of a dropdown or context menu. */
export const MENU_CONTENT_CLASS = cn(
	"z-80 min-w-50 rounded-md border border-border bg-surface p-1 text-fg shadow-lg",
	"data-[state=closed]:animate-fade-out data-[state=open]:animate-pop-in",
);

/** One row of a dropdown or context menu. */
export const MENU_ITEM_CLASS = cn(
	"flex min-h-10 w-full cursor-pointer select-none items-center gap-2 rounded-sm px-3 font-mono text-[0.8125rem] outline-none",
	"data-disabled:cursor-not-allowed data-highlighted:bg-fg/6 data-disabled:opacity-45",
);

export const MENU_SEPARATOR_CLASS = "my-1 h-px bg-border-subtle";
