import type { ComponentProps } from "react";
import { cn } from "../../lib/cn";

export type WordmarkProps = Omit<ComponentProps<"span">, "children">;

/** "Scribe" in bold JetBrains Mono, the "S" in violet step 11. */
export function Wordmark({ className, ...rest }: WordmarkProps) {
	return (
		<span
			className={cn(
				"font-bold font-mono text-fg text-xl tracking-[-0.02em]",
				className,
			)}
			{...rest}
		>
			<span className="text-primary">S</span>cribe
		</span>
	);
}

export interface BrandMarkProps {
	/** Square size in px. */
	size?: number;
	className?: string;
}

/**
 * App mark: sound wave (primary) turning into transcript lines (accent),
 * the same drawing as the app icon, coloured by tokens.
 */
export function BrandMark({ size = 28, className }: BrandMarkProps) {
	return (
		<svg
			width={size}
			height={size}
			viewBox="0 0 32 32"
			aria-hidden="true"
			className={cn("block flex-none", className)}
		>
			<g className="fill-primary">
				<rect x="3.5" y="10" width="3.5" height="12" rx="1.75" />
				<rect x="9" y="6" width="3.5" height="20" rx="1.75" />
				<rect x="14.5" y="9" width="3.5" height="14" rx="1.75" />
			</g>
			<g className="fill-accent">
				<rect x="21" y="9" width="8" height="3.5" rx="1.75" />
				<rect x="21" y="14.25" width="6.5" height="3.5" rx="1.75" />
				<rect x="21" y="19.5" width="5" height="3.5" rx="1.75" />
			</g>
		</svg>
	);
}
