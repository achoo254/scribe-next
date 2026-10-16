import { useT } from "../../i18n/i18n-context";
import { formatClock } from "../../lib/format";

export interface RecordTimerProps {
	elapsedS: number;
	"data-id"?: string;
}

/** "● Đang ghi — 2:05" beside the stop button, browser and iOS alike. */
export function RecordTimer({ elapsedS, "data-id": dataId }: RecordTimerProps) {
	const t = useT();
	return (
		<p
			data-id={dataId}
			aria-live="polite"
			className="m-0 flex items-center gap-2 font-mono font-semibold text-error"
		>
			<span
				aria-hidden="true"
				className="size-2.5 rounded-full bg-error motion-safe:animate-pulse"
			/>
			{t("record.timer", { clock: formatClock(elapsedS) })}
		</p>
	);
}
