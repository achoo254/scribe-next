import type { ComponentProps } from "react";
import { cn } from "../../lib/cn";

/**
 * Surface used by list rows, meters, plan cards and dashboard tiles:
 * 24px radius, token border and background, small shadow. Use the class on
 * semantic elements (`li`, `article`, `section`) and `Card` for a plain div.
 */
export const cardClassName =
	"rounded-xl border border-border bg-surface shadow-sm";

export function Card({ className, ...rest }: ComponentProps<"div">) {
	return <div className={cn(cardClassName, className)} {...rest} />;
}
