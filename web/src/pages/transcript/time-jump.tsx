import { useT } from "../../i18n/i18n-context";
import { formatClock } from "../../lib/format";

export interface TimeJumpProps {
	/** Seconds from the start of the session. */
	at: number;
	onJump: (at: number) => void;
	"data-id"?: string;
}

/** "12:34" of a minutes item or a bookmark: a tap plays the audio from there. */
export function TimeJump({ at, onJump, "data-id": dataId }: TimeJumpProps) {
	const t = useT();
	const time = formatClock(at);
	return (
		<button
			type="button"
			data-id={dataId}
			aria-label={t("transcript.minutes.jump", { time })}
			onClick={() => onJump(at)}
			className="min-h-8 flex-none cursor-pointer rounded-sm border-0 bg-transparent px-1 font-mono text-primary text-sm tabular-nums hover:bg-surface-subtle focus-visible:shadow-focus-primary focus-visible:outline-none pointer-coarse:min-h-11"
		>
			{time}
		</button>
	);
}
