import type { ComponentProps } from "react";
import { cn } from "../../lib/cn";

export interface SwitchProps
	extends Omit<
		ComponentProps<"button">,
		"type" | "role" | "onChange" | "onClick"
	> {
	checked: boolean;
	/** Called with the requested state; the owner decides when it applies. */
	onCheckedChange: (checked: boolean) => void;
}

/**
 * On/off switch (`role="switch"`, `aria-checked`). Needs an accessible
 * name: `aria-label` or `aria-labelledby`. The track stays 36×20; on
 * touch screens a transparent pseudo-element grows the hit area to 44px.
 */
export function Switch({
	checked,
	onCheckedChange,
	className,
	...rest
}: SwitchProps) {
	return (
		<button
			type="button"
			role="switch"
			aria-checked={checked}
			className={cn(
				"relative inline-flex h-5 w-9 flex-none cursor-pointer items-center rounded-full border border-transparent p-0.5 transition-colors",
				"after:absolute after:content-[''] pointer-coarse:after:-inset-3",
				"disabled:cursor-not-allowed disabled:opacity-45",
				checked ? "bg-primary-solid" : "bg-border-strong",
				className,
			)}
			onClick={() => onCheckedChange(!checked)}
			{...rest}
		>
			<span
				aria-hidden="true"
				className={cn(
					"size-3.5 rounded-full bg-surface shadow-sm transition-transform motion-reduce:transition-none",
					checked && "translate-x-4",
				)}
			/>
		</button>
	);
}
