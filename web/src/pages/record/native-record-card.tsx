import { IconMicrophone, IconPlayerStopFilled } from "@tabler/icons-react";
import { Alert } from "../../components/ui/alert";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { SegmentedControl } from "../../components/ui/segmented-control";
import { useT } from "../../i18n/i18n-context";
import { BookmarkControl } from "./bookmark-control";
import { LevelMeter } from "./live-monitor";
import { useNativeRecordState } from "./record-state";
import { RecordTimer } from "./record-timer";

const SEGMENT_MINUTES = ["1", "5", "10"] as const;

/**
 * "Ghi âm" in the iOS app: the Swift engine records the microphone in
 * segments that keep going on the lock screen and during calls, and
 * uploads each finished segment to "Lịch sử ghi". State and the segment
 * length live in `RecordStateProvider`, so they survive leaving the page.
 */
export function NativeRecordCard() {
	const t = useT();
	const { rec, minutes, setMinutes, toggle, bookmarks, mark } =
		useNativeRecordState();
	const busy = rec.recording || rec.starting || rec.finalizing;
	const options = SEGMENT_MINUTES.map((value) => ({
		value,
		label: t("record.native.segmentOption", { count: Number(value) }),
		"data-id": `native-segment-${value}`,
	}));

	return (
		<Card data-id="native-record" className="grid gap-5 p-4 sm:p-6">
			<div className="grid gap-1.5">
				<span
					aria-hidden="true"
					className="font-mono font-semibold text-[0.8125rem] text-fg"
				>
					{t("record.native.segmentLength")}
				</span>
				<SegmentedControl
					legend={t("record.native.segmentLength")}
					data-id="native-segment"
					options={options}
					value={minutes}
					onChange={(next) => {
						if (!busy) setMinutes(next);
					}}
				/>
			</div>

			<div className="grid gap-4 border-border-subtle border-t pt-5">
				<div className="flex flex-wrap items-center gap-x-5 gap-y-3">
					<Button
						variant={rec.recording ? "danger" : "primary"}
						size="lg"
						data-id="native-record-toggle"
						loading={rec.starting || rec.finalizing}
						leadingIcon={
							rec.recording ? (
								<IconPlayerStopFilled size={20} aria-hidden="true" />
							) : (
								<IconMicrophone size={20} aria-hidden="true" />
							)
						}
						onClick={() => void toggle()}
					>
						{rec.recording ? t("record.stop") : t("record.start")}
					</Button>
					{rec.recording ? (
						<RecordTimer
							data-id="native-record-timer"
							elapsedS={rec.elapsedS}
						/>
					) : null}
					{rec.recording ? (
						<BookmarkControl
							idPrefix="native-record"
							onMark={mark}
							bookmarks={bookmarks}
						/>
					) : null}
				</div>
				{rec.recording ? <LevelMeter recorder={rec.recorder} /> : null}

				{rec.notice ? <Alert tone="info">{rec.notice}</Alert> : null}
				{rec.error ? (
					<Alert tone="danger" data-id="native-record-error">
						{rec.error}
					</Alert>
				) : null}
				{rec.uploadsWaiting > 0 ? (
					<p
						data-id="native-uploads"
						className="m-0 text-muted text-sm"
						aria-live="polite"
					>
						{t("record.native.uploadsWaiting", { count: rec.uploadsWaiting })}
					</p>
				) : null}
				{rec.uploadsFailed > 0 ? (
					<Alert tone="warning">
						{t("record.native.uploadsFailed", { count: rec.uploadsFailed })}
					</Alert>
				) : null}
			</div>
		</Card>
	);
}
