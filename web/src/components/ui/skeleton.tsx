import { type ComponentProps, useEffect, useState } from "react";
import { useT } from "../../i18n/i18n-context";
import { cn } from "../../lib/cn";

export type SkeletonVariant = "block" | "text" | "circle";

export interface SkeletonProps {
	/** `block` (default) and `circle` take their size from `className`. */
	variant?: SkeletonVariant;
	/** Number of lines of the `text` variant; the last one is shorter. */
	lines?: number;
	className?: string;
}

/** Muted block; the shimmer only moves when the user allows motion. */
const SURFACE = "skeleton motion-safe:skeleton-shimmer";

/**
 * Placeholder in the shape of content that is still loading. Hidden from
 * assistive technology: wrap a screen's skeletons in `SkeletonRegion`,
 * which announces the load once.
 *
 * ```tsx
 * <Skeleton className="h-6 w-40" />
 * <Skeleton variant="circle" className="size-11" />
 * <Skeleton variant="text" lines={3} className="w-full max-w-md" />
 * ```
 */
export function Skeleton({
	variant = "block",
	lines = 3,
	className,
}: SkeletonProps) {
	if (variant === "text") {
		const count = Math.max(1, Math.floor(lines));
		return (
			<span
				aria-hidden="true"
				data-skeleton="text"
				className={cn("grid gap-2", className)}
			>
				{Array.from({ length: count }, (_, index) => (
					<span
						// Lines are identical and never reorder.
						// biome-ignore lint/suspicious/noArrayIndexKey: static placeholder list
						key={index}
						className={cn(
							SURFACE,
							"block h-3 rounded-xs",
							index === count - 1 && count > 1 ? "w-3/5" : "w-full",
						)}
					/>
				))}
			</span>
		);
	}
	return (
		<span
			aria-hidden="true"
			data-skeleton={variant}
			className={cn(
				SURFACE,
				"block",
				variant === "circle" ? "rounded-full" : "rounded-md",
				className,
			)}
		/>
	);
}

export interface SkeletonRegionProps extends ComponentProps<"div"> {
	/** What screen readers hear once; defaults to "Đang tải…". */
	label?: string;
}

/**
 * Wraps the skeletons of one loading area: the area is `aria-busy`, and a
 * visually hidden polite status beside it says "Đang tải…" once. The status
 * sits outside the busy element because assistive technology may ignore
 * changes inside an `aria-busy` subtree, and its text is filled after mount
 * so the live region exists before it changes.
 */
export function SkeletonRegion({
	label,
	className,
	children,
	...rest
}: SkeletonRegionProps) {
	const t = useT();
	const [mounted, setMounted] = useState(false);
	useEffect(() => setMounted(true), []);
	return (
		<>
			<output className="sr-only" data-skeleton-status="">
				{mounted ? (label ?? t("ui.loading")) : null}
			</output>
			<div aria-busy="true" className={className} {...rest}>
				{children}
			</div>
		</>
	);
}
