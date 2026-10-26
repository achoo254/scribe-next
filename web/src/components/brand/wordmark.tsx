import type { ComponentProps } from "react";
import scribeMark from "../../../brand/scribe-mark-128.webp";
import { APP_NAME } from "../../lib/brand";
import { cn } from "../../lib/cn";

export type WordmarkProps = Omit<ComponentProps<"span">, "children">;

/** The app name in bold JetBrains Mono, its first letter in violet step 11. */
export function Wordmark({ className, ...rest }: WordmarkProps) {
	return (
		<span
			className={cn(
				"font-bold font-mono text-fg text-xl tracking-[-0.02em]",
				className,
			)}
			{...rest}
		>
			<span className="text-primary">{APP_NAME.slice(0, 1)}</span>
			{APP_NAME.slice(1)}
		</span>
	);
}

export interface BrandMarkProps {
	/** Square size in px. */
	size?: number;
	className?: string;
}

/**
 * App mark: the AI-drawn Scribe icon (rounded tile, transparent corners),
 * the same art as the app icon. A file, never inlined (vite.config.ts): the
 * CSP allows images from this origin only.
 */
export function BrandMark({ size = 28, className }: BrandMarkProps) {
	return (
		<img
			src={scribeMark}
			width={size}
			height={size}
			alt=""
			className={cn("block flex-none", className)}
		/>
	);
}
