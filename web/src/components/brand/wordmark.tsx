import type { ComponentProps } from "react";
import scribeMark from "../../../brand/scribe-mark-128.webp";
import { APP_NAME } from "../../lib/brand";
import { cn } from "../../lib/cn";

export type WordmarkProps = Omit<ComponentProps<"span">, "children">;

/**
 * The app name in bold Inter and the primary colour, as the account app
 * (auth.hoandat.com) sets the brand name in its top bar.
 */
export function Wordmark({ className, ...rest }: WordmarkProps) {
	return (
		<span
			className={cn("font-bold font-sans text-base text-primary", className)}
			{...rest}
		>
			{APP_NAME}
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
