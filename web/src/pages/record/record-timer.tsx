import { useT } from "../../i18n/i18n-context";
import { formatClock } from "../../lib/format";

export interface RecordTimerProps {
	elapsedS: number;
	/** The clock stands still: "Đã tạm dừng — 2:05", without the pulsing dot. */
	paused?: boolean;
	"data-id"?: string;
}

/** "● Đang ghi — 2:05" beside the stop button, browser and iOS alike. */
export function RecordTimer({
	elapsedS,
	paused = false,
	"data-id": dataId,
}: RecordTimerProps) {
	const t = useT();
	const clock = formatClock(elapsedS);
	return (
		<p
			data-id={dataId}
			aria-live="polite"
			className={`m-0 flex items-center gap-2 font-mono font-semibold ${paused ? "text-muted" : "text-error"}`}
		>
			<span
				aria-hidden="true"
				className={`size-2.5 rounded-full ${paused ? "bg-muted" : "bg-error motion-safe:animate-pulse"}`}
			/>
			{paused
				? t("record.timerPaused", { clock })
				: t("record.timer", { clock })}
		</p>
	);
}
