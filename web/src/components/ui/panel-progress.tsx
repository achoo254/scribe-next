import type { ComponentProps } from "react";
import { cn } from "../../lib/cn";
import { useDelayedFlag } from "../../lib/use-delayed-flag";
import { ProgressBar } from "./progress-bar";

export interface PanelProgressProps extends ComponentProps<"div"> {
	/**
	 * Data on screen is being refreshed, e.g. `isFetching && !isPending`.
	 * Passed through `useDelayedFlag`, so quick refetches show nothing.
	 */
	active: boolean;
	/**
	 * The content no longer matches what was asked (a filter changed):
	 * dims it slightly while the bar shows.
	 */
	dim?: boolean;
}

/**
 * Panel that keeps its data visible while it refreshes, with a thin bar
 * along its top edge. The bar is decorative; set `aria-busy` on this
 * element (or the surrounding panel) for assistive technology. Rounded
 * panels pass `overflow-hidden` so the bar follows the corners.
 *
 * ```tsx
 * <PanelProgress active={query.isFetching && !query.isPending} aria-busy={query.isFetching}>
 *   <List items={query.data} />
 * </PanelProgress>
 * ```
 */
export function PanelProgress({
	active,
	dim = false,
	className,
	children,
	...rest
}: PanelProgressProps) {
	const shown = useDelayedFlag(active);
	return (
		<div {...rest} className={cn("relative", className)}>
			{shown ? (
				<ProgressBar
					data-id="panel-progress"
					className="absolute inset-x-0 top-0 z-10"
				/>
			) : null}
			<div
				data-dimmed={dim && shown ? "" : undefined}
				className={cn("transition-opacity", dim && shown && "opacity-60")}
			>
				{children}
			</div>
		</div>
	);
}
