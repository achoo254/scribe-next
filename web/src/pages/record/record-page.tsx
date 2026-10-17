import { IconMicrophone, IconPlayerStopFilled } from "@tabler/icons-react";
import { Alert } from "../../components/ui/alert";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { ProgressBar } from "../../components/ui/progress-bar";
import { supportsScreenCapture } from "../../features/recorder/media-capture";
import { isJobBusy } from "../../features/split/split-job";
import { useT } from "../../i18n/i18n-context";
import { renderRich } from "../../i18n/rich-text";
import { PageHeader } from "../../layout/page-header";
import { isNativeShell } from "../../lib/api";
import { parsePositiveNumber } from "../../lib/format";
import { BookmarkControl } from "./bookmark-control";
import { LiveMonitor } from "./live-monitor";
import { NativeRecordCard } from "./native-record-card";
import { RecordSourcesField } from "./record-sources";
import { useBrowserRecordState } from "./record-state";
import { RecordTimer } from "./record-timer";
import { RecordUpload } from "./record-upload";
import { RecordingHistory } from "./recording-history";
import { SplitAfterRecording } from "./split-after-recording";

const canScreen = supportsScreenCapture();

/**
 * "Ghi âm" page (Mode A): sources → record/stop → history and playback.
 * In the iOS app the Swift recorder replaces the browser one (mic only:
 * iOS gives no system sound or screen to a regular app). The recording
 * itself lives in `RecordStateProvider`, so it outlives this page.
 */
export function RecordPage() {
	const t = useT();
	const native = isNativeShell();
	return (
		<section data-id="record" className="grid gap-5">
			<PageHeader
				title={t("record.title")}
				description={
					native ? t("record.nativeDescription") : t("record.description")
				}
			/>
			{native ? <NativeRecordCard /> : <BrowserRecordCard />}
			<RecordingHistory />
		</section>
	);
}

function BrowserRecordCard() {
	const t = useT();
	const {
		rec,
		split,
		sources,
		setSources,
		splitEnabled,
		setSplitEnabled,
		splitMinutes,
		setSplitMinutes,
		toggle,
		bookmarks,
		mark,
	} = useBrowserRecordState();

	const locked = rec.recording || rec.finalizing;
	const noSource = !sources.mic && !sources.systemAudio && !sources.screen;
	const needsScreen = sources.systemAudio || sources.screen;
	const minutes = parsePositiveNumber(splitMinutes);
	const startBlocked =
		noSource ||
		(needsScreen && !canScreen) ||
		(splitEnabled && minutes === null);
	const splitBusy = isJobBusy(split.status);

	return (
		<Card data-id="record-card" className="grid gap-5 p-4 sm:p-6">
			<div className="grid gap-4">
				<RecordSourcesField
					value={sources}
					onChange={setSources}
					disabled={locked}
				/>
				<RecordUpload />
				{noSource ? <Alert tone="warning">{t("record.noSource")}</Alert> : null}
				<Alert tone="info" data-id="record-sources-help">
					{renderRich(t("record.sourcesHelp"))}
					{canScreen ? null : (
						<>
							<br />
							{t("record.noScreenCapture")}
						</>
					)}
				</Alert>
				<SplitAfterRecording
					enabled={splitEnabled}
					onEnabledChange={setSplitEnabled}
					minutes={splitMinutes}
					onMinutesChange={setSplitMinutes}
					disabled={locked}
				/>
			</div>

			<div className="grid gap-4 border-border-subtle border-t pt-5">
				<div className="flex flex-wrap items-center gap-x-5 gap-y-3">
					<Button
						variant={rec.recording ? "danger" : "primary"}
						size="lg"
						data-id="record-toggle"
						loading={rec.starting || rec.finalizing}
						disabled={!rec.recording && startBlocked}
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
						<RecordTimer data-id="record-timer" elapsedS={rec.elapsedS} />
					) : null}
					{rec.recording ? (
						<BookmarkControl
							idPrefix="record"
							onMark={mark}
							bookmarks={bookmarks}
						/>
					) : null}
					{rec.finalizing ? (
						<p className="m-0 text-muted text-sm">{t("record.saving")}</p>
					) : null}
				</div>

				{rec.recording ? (
					<LiveMonitor
						recorder={rec.recorder}
						stream={rec.previewStream}
						showPreview={sources.screen}
					/>
				) : null}
				{rec.error ? (
					<Alert tone="danger" data-id="record-error">
						{rec.error}
					</Alert>
				) : null}

				{splitBusy ? (
					<div className="grid gap-2" aria-busy="true">
						<ProgressBar className="relative rounded-full" />
						<p className="m-0 text-muted text-sm">{t("record.splitting")}</p>
					</div>
				) : null}
				{split.error ? <Alert tone="danger">{split.error}</Alert> : null}
				{split.status === "done" ? (
					<Alert tone="success" data-id="record-split-done">
						{renderRich(
							t("record.splitDone", { count: split.segments.length }),
						)}
					</Alert>
				) : null}
			</div>
		</Card>
	);
}
