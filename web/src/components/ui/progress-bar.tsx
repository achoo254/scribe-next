import { cn } from "../../lib/cn";

/**
 * Thin indeterminate bar shared by `PanelProgress` and `RouteProgress`:
 * one `--space-1` high, a third-wide primary segment sweeping across. With
 * reduced motion the segment fills the track and stands still. Decorative
 * (`aria-hidden`); the owner exposes the state with `aria-busy`.
 */
export function ProgressBar({
	className,
	"data-id": dataId,
}: {
	/** Positioning of the track, e.g. `absolute inset-x-0 top-0`. */
	className: string;
	"data-id"?: string;
}) {
	return (
		<span
			aria-hidden="true"
			data-id={dataId}
			className={cn(
				"pointer-events-none block h-1 overflow-hidden bg-primary/15",
				className,
			)}
		>
			<span className="block h-full w-full bg-primary motion-safe:w-1/3 motion-safe:animate-progress-sweep" />
		</span>
	);
}
