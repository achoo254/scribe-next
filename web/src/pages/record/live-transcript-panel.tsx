import { useEffect, useRef } from "react";
import { Alert } from "../../components/ui/alert";
import type { LiveTranscriptState } from "../../features/recorder/live-transcript";
import { useT } from "../../i18n/i18n-context";
import { formatClock } from "../../lib/format";

export interface LiveTranscriptPanelProps {
	live: LiveTranscriptState;
	/** The recording is still going (title "Đang chép", connection line). */
	recording: boolean;
}

/**
 * "Đang chép": sentences of the live transcript with their session-clock
 * position, newest at the bottom. A preview only; a lost connection or a
 * refused token is said plainly and never stops the recording.
 */
export function LiveTranscriptPanel({
	live,
	recording,
}: LiveTranscriptPanelProps) {
	const t = useT();
	const listRef = useRef<HTMLOListElement>(null);
	const count = live.sentences.length;

	// Follow the newest sentence, like a caption.
	useEffect(() => {
		const list = listRef.current;
		if (list && count > 0) list.scrollTop = list.scrollHeight;
	}, [count]);

	const connecting =
		recording &&
		!live.connected &&
		!live.stopped &&
		live.disconnectedAtS === null &&
		!live.error;

	return (
		<section
			data-id="live-transcript"
			aria-label={t("record.native.liveTitle")}
			className="grid gap-2 border-border-subtle border-t pt-4"
		>
			<h3 className="m-0 font-semibold text-fg text-sm">
				{recording
					? t("record.native.liveTitle")
					: t("record.native.liveTitleDone")}
			</h3>
			{live.error ? (
				<Alert tone="warning" data-id="live-transcript-error">
					{live.error}
				</Alert>
			) : null}
			{recording && live.disconnectedAtS !== null && !live.stopped ? (
				<p
					data-id="live-transcript-disconnected"
					className="m-0 text-sm text-warning-text"
					aria-live="polite"
				>
					{t("record.native.liveDisconnected", {
						clock: formatClock(live.disconnectedAtS),
					})}
				</p>
			) : null}
			{connecting ? (
				<p className="m-0 text-muted text-sm">
					{t("record.native.liveConnecting")}
				</p>
			) : null}
			{count === 0 && recording && !live.error ? (
				<p className="m-0 text-muted text-sm">{t("record.native.liveEmpty")}</p>
			) : null}
			{count > 0 ? (
				<ol
					ref={listRef}
					data-id="live-transcript-list"
					aria-live="polite"
					className="m-0 grid max-h-72 list-none gap-1.5 overflow-y-auto p-0 text-sm"
				>
					{live.sentences.map((s) => (
						<li
							key={s.id}
							data-id={`live-transcript-item-${s.id}`}
							className="flex gap-2"
						>
							<span className="shrink-0 font-mono text-muted">
								{formatClock(s.atS)}
							</span>
							{s.gapUntilS === undefined ? (
								<span className={s.partial ? "text-muted italic" : "text-fg"}>
									{s.text}
								</span>
							) : (
								<span className="text-muted italic">
									{t("record.native.liveGap", {
										from: formatClock(s.atS),
										to: formatClock(s.gapUntilS),
									})}
								</span>
							)}
						</li>
					))}
				</ol>
			) : null}
			<p className="m-0 text-muted text-xs">{t("record.native.liveNote")}</p>
		</section>
	);
}
