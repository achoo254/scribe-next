import type { ComponentProps } from "react";
import { cn } from "../../lib/cn";

export type BadgeTone =
	| "neutral"
	| "primary"
	| "accent"
	| "info"
	| "success"
	| "warning"
	| "danger";

/**
 * `--tone` tints background/border (tint-soft utility); `--tone-fg` is the
 * label colour. Semantic tones use the *-text tokens, which keep ≥4.5:1 on
 * the tint in both themes.
 */
const TONES: Record<BadgeTone, string> = {
	neutral: "[--tone:var(--c-text-muted)] [--tone-fg:var(--c-text-muted)]",
	primary: "[--tone:var(--c-primary)] [--tone-fg:var(--c-primary)]",
	accent: "[--tone:var(--c-accent)] [--tone-fg:var(--c-accent)]",
	info: "[--tone:var(--c-info)] [--tone-fg:var(--c-info-text)]",
	success: "[--tone:var(--c-success)] [--tone-fg:var(--c-success-text)]",
	warning: "[--tone:var(--c-warning)] [--tone-fg:var(--c-warning-text)]",
	danger: "[--tone:var(--c-error)] [--tone-fg:var(--c-error-text)]",
};

export interface BadgeProps extends ComponentProps<"span"> {
	tone?: BadgeTone;
}

export function Badge({ tone = "neutral", className, ...rest }: BadgeProps) {
	return (
		<span
			data-tone={tone}
			className={cn(
				"tint-soft inline-flex items-center gap-1 whitespace-nowrap rounded-sm border px-2 py-1.25",
				"font-mono font-semibold text-(--tone-fg) text-[0.6875rem] uppercase leading-none tracking-[0.08em]",
				TONES[tone],
				className,
			)}
			{...rest}
		/>
	);
}
