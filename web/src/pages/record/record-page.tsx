import { IconMicrophone, IconPlayerStopFilled } from "@tabler/icons-react";
import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useState } from "react";
import { Alert } from "../../components/ui/alert";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { ProgressBar } from "../../components/ui/progress-bar";
import { refreshStorage } from "../../features/files/file-queries";
import { supportsScreenCapture } from "../../features/recorder/media-capture";
import type { Recorder } from "../../features/recorder/recorder";
import type { RecordSources } from "../../features/recorder/recorder-messages";
import { useRecorder } from "../../features/recorder/use-recorder";
import { isJobBusy } from "../../features/split/split-job";
import { useSplitJob } from "../../features/split/use-split-job";
import { useReportSessionActivity } from "../../layout/session-activity";
import { isNativeShell } from "../../lib/api";
import { formatClock, parsePositiveNumber } from "../../lib/format";
import { LiveMonitor } from "./live-monitor";
import { NativeRecordCard } from "./native-record-card";
import { RecordSourcesField } from "./record-sources";
import { RecordingHistory } from "./recording-history";
import { SplitAfterRecording } from "./split-after-recording";

const canScreen = supportsScreenCapture();

/**
 * "Ghi âm" tab (Mode A): sources → record/stop → history and playback.
 * In the iOS app the Swift recorder replaces the browser one (mic only:
 * iOS gives no system sound or screen to a regular app).
 */
export function RecordPage() {
	if (isNativeShell()) {
		return (
			<div data-id="record" className="grid gap-4">
				<NativeRecordCard />
				<RecordingHistory />
			</div>
		);
	}
	return <BrowserRecordPage />;
}

/** Resolves once the recorder is no longer saving a recording. */
function saved(recorder: Recorder): Promise<void> {
	return new Promise((resolve) => {
		const unsubscribe = recorder.subscribe(() => {
			if (recorder.getState().finalizing) return;
			unsubscribe();
			resolve();
		});
		if (!recorder.getState().finalizing) {
			unsubscribe();
			resolve();
		}
	});
}

function BrowserRecordPage() {
	const queryClient = useQueryClient();
	const rec = useRecorder();
	const split = useSplitJob();
	const { recorder } = rec;
	// Sign-out saves the recording first (stop uploads the last chunks and
	// finalizes), so nothing is sent after the session is gone.
	const stopRecording = useCallback(async () => {
		await recorder.stop();
		await saved(recorder);
	}, [recorder]);
	useReportSessionActivity("browser-recorder", {
		recording: rec.recording || rec.starting || rec.finalizing,
		segmentsWaiting: 0,
		fileUploading: false,
		stopRecording,
	});
	const [sources, setSources] = useState<RecordSources>({
		mic: true,
		systemAudio: false,
		screen: false,
	});
	const [splitEnabled, setSplitEnabled] = useState(false);
	const [splitMinutes, setSplitMinutes] = useState("10");

	const locked = rec.recording || rec.finalizing;
	const noSource = !sources.mic && !sources.systemAudio && !sources.screen;
	const needsScreen = sources.systemAudio || sources.screen;
	const minutes = parsePositiveNumber(splitMinutes);
	const startBlocked =
		noSource ||
		(needsScreen && !canScreen) ||
		(splitEnabled && minutes === null);
	const splitBusy = isJobBusy(split.status);

	// A finished cut (done: segments replace the recording; failed: the
	// original is still there) changes the history and the quota.
	useEffect(() => {
		if (split.status === "done" || split.status === "failed") {
			void refreshStorage(queryClient, ["recordings"]);
		}
	}, [split.status, queryClient]);

	async function toggle(): Promise<void> {
		if (!rec.recording) {
			split.reset();
			await rec.recorder.start(sources);
			return;
		}
		const result = await rec.recorder.stop();
		if (!result) return;
		if (splitEnabled && minutes !== null) {
			// The history refreshes when the job ends, so the original does
			// not flash in before being replaced by its segments.
			void refreshStorage(queryClient);
			await split.start({
				inputFileId: result.fileId,
				criterion: "minutes",
				value: minutes,
				recordingSplit: true,
			});
		} else {
			void refreshStorage(queryClient, ["recordings"]);
		}
	}

	return (
		<div data-id="record" className="grid gap-4">
			<Card className="grid gap-4 p-4 sm:p-6">
				<h2 className="m-0 font-bold font-mono text-lg">Ghi âm</h2>
				<RecordSourcesField
					value={sources}
					onChange={setSources}
					disabled={locked}
				/>
				{noSource ? (
					<Alert tone="warning">Hãy bật ít nhất một nguồn ghi.</Alert>
				) : null}
				<Alert tone="info" data-id="record-sources-help">
					Tiếng hệ thống và ghi màn hình cần chia sẻ màn hình — tốt nhất trên{" "}
					<strong>Chrome/Edge</strong>: chọn “Toàn màn hình” và bật “Chia sẻ âm
					thanh” để có tiếng hệ thống. Firefox/Safari có thể không hỗ trợ tiếng
					hệ thống. Chọn nhiều nguồn → trộn chung vào 1 bản ghi.
					{canScreen ? null : (
						<>
							<br />
							Trình duyệt này không hỗ trợ chia sẻ màn hình (chỉ ghi được mic).
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

				{rec.recording ? (
					<>
						<p
							data-id="record-timer"
							aria-live="polite"
							className="m-0 flex items-center gap-2 font-mono font-semibold text-error"
						>
							<span
								aria-hidden="true"
								className="size-2.5 rounded-full bg-error motion-safe:animate-pulse"
							/>
							Đang ghi — {formatClock(rec.elapsedS)}
						</p>
						<LiveMonitor
							recorder={rec.recorder}
							stream={rec.previewStream}
							showPreview={sources.screen}
						/>
					</>
				) : null}

				<Button
					variant={rec.recording ? "danger" : "primary"}
					size="lg"
					data-id="record-toggle"
					className="justify-self-start"
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
					{rec.recording ? "Dừng ghi" : "Bắt đầu ghi"}
				</Button>

				{rec.finalizing ? (
					<p className="m-0 text-muted text-sm">Đang lưu bản ghi…</p>
				) : null}
				{rec.error ? (
					<Alert tone="danger" data-id="record-error">
						{rec.error}
					</Alert>
				) : null}

				{splitBusy ? (
					<div className="grid gap-2" aria-busy="true">
						<ProgressBar className="relative rounded-full" />
						<p className="m-0 text-muted text-sm">
							Đang cắt bản ghi thành các đoạn…
						</p>
					</div>
				) : null}
				{split.error ? <Alert tone="danger">{split.error}</Alert> : null}
				{split.status === "done" ? (
					<Alert tone="success" data-id="record-split-done">
						Đã cắt thành {split.segments.length} đoạn — xem trong{" "}
						<strong>Lịch sử ghi</strong> bên dưới.
					</Alert>
				) : null}
			</Card>

			<RecordingHistory />
		</div>
	);
}
