import { cn } from "../../lib/cn";

export type MeterTone = "primary" | "warning" | "error";

const FILL: Record<MeterTone, string> = {
	primary: "bg-primary",
	warning: "bg-warning",
	error: "bg-error",
};

export interface MeterBarProps {
	/** 0–100; values outside are clamped. */
	value: number;
	/** Accessible name, e.g. "Dung lượng đã dùng". */
	label: string;
	tone?: MeterTone;
	/**
	 * `progressbar` for work that completes (upload), `meter` for a level
	 * within a range (quota, audio level).
	 */
	role?: "progressbar" | "meter";
	/** Track height: `sm` 6px (quota), `md` 8px (progress, audio level). */
	size?: "sm" | "md";
	/** Smooth width changes; off for the audio level (updates every frame). */
	animate?: boolean;
	className?: string;
	"data-id"?: string;
}

/**
 * Determinate bar on the `--c-bar-track` token. The indeterminate sweep
 * (`ProgressBar`) is for work of unknown length.
 */
export function MeterBar({
	value,
	label,
	tone = "primary",
	role = "progressbar",
	size = "md",
	animate = true,
	className,
	"data-id": dataId,
}: MeterBarProps) {
	const clamped = Math.min(
		100,
		Math.max(0, Number.isFinite(value) ? value : 0),
	);
	const rounded = Math.round(clamped);
	return (
		<div
			role={role}
			aria-label={label}
			aria-valuemin={0}
			aria-valuemax={100}
			aria-valuenow={rounded}
			data-id={dataId}
			data-tone={tone}
			className={cn(
				"w-full overflow-hidden rounded-full bg-bar-track",
				size === "sm" ? "h-1.5" : "h-2",
				className,
			)}
		>
			<div
				className={cn(
					"h-full rounded-full",
					FILL[tone],
					animate && "transition-[width] duration-(--dur-base)",
				)}
				style={{ width: `${clamped}%` }}
			/>
		</div>
	);
}
