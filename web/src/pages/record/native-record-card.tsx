import { IconMicrophone, IconPlayerStopFilled } from "@tabler/icons-react";
import { useState } from "react";
import { useAuth } from "../../auth/auth-context";
import { Alert } from "../../components/ui/alert";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { SegmentedControl } from "../../components/ui/segmented-control";
import { userKeyOf } from "../../features/recorder/native-recorder";
import { useNativeRecorder } from "../../features/recorder/use-native-recorder";
import { useReportSessionActivity } from "../../layout/session-activity";
import { formatClock } from "../../lib/format";
import { LevelMeter } from "./live-monitor";

type SegmentMinutes = "1" | "5" | "10";

const SEGMENT_OPTIONS = [
	{ value: "1", label: "1 phút", "data-id": "native-segment-1" },
	{ value: "5", label: "5 phút", "data-id": "native-segment-5" },
	{ value: "10", label: "10 phút", "data-id": "native-segment-10" },
] as const;

/**
 * "Ghi âm" in the iOS app: the Swift engine records the microphone in
 * segments that keep going on the lock screen and during calls, and
 * uploads each finished segment to "Lịch sử ghi".
 */
export function NativeRecordCard() {
	const rec = useNativeRecorder();
	const { user } = useAuth();
	const [minutes, setMinutes] = useState<SegmentMinutes>("1");
	const busy = rec.recording || rec.starting || rec.finalizing;
	// Signing out stops the Swift recorder itself (RecorderPlugin.signOut).
	useReportSessionActivity("native-recorder", {
		recording: busy,
		segmentsWaiting: rec.uploadsWaiting,
		fileUploading: false,
	});

	async function toggle(): Promise<void> {
		if (rec.recording) {
			await rec.recorder.stop();
			return;
		}
		if (!user) return;
		const key = rec.userKey ?? (await userKeyOf(user.email));
		await rec.recorder.start(key, Number(minutes));
	}

	return (
		<Card data-id="native-record" className="grid gap-4 p-4 sm:p-6">
			<h2 className="m-0 font-bold font-mono text-lg">Ghi âm</h2>
			<p className="m-0 text-muted text-sm">
				Ghi bằng micro của iPhone, vẫn ghi khi khoá màn hình hoặc có cuộc gọi.
				Mỗi đoạn ghi xong được tải lên Lịch sử ghi ngay trong lúc ghi tiếp.
			</p>
			<div className="grid gap-1.5">
				<span
					aria-hidden="true"
					className="font-mono font-semibold text-[0.8125rem] text-fg"
				>
					Độ dài mỗi đoạn
				</span>
				<SegmentedControl
					legend="Độ dài mỗi đoạn"
					data-id="native-segment"
					options={SEGMENT_OPTIONS}
					value={minutes}
					onChange={(next) => {
						if (!busy) setMinutes(next);
					}}
				/>
			</div>

			{rec.recording ? (
				<>
					<p
						data-id="native-record-timer"
						aria-live="polite"
						className="m-0 flex items-center gap-2 font-mono font-semibold text-error"
					>
						<span
							aria-hidden="true"
							className="size-2.5 rounded-full bg-error motion-safe:animate-pulse"
						/>
						Đang ghi — {formatClock(rec.elapsedS)}
					</p>
					<LevelMeter recorder={rec.recorder} />
				</>
			) : null}

			<Button
				variant={rec.recording ? "danger" : "primary"}
				size="lg"
				className="justify-self-start"
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
				{rec.recording ? "Dừng ghi" : "Bắt đầu ghi"}
			</Button>

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
					Đang chờ tải lên: {rec.uploadsWaiting} đoạn (giữ trên máy, tự tải khi
					có mạng).
				</p>
			) : null}
			{rec.uploadsFailed > 0 ? (
				<Alert tone="warning">
					{rec.uploadsFailed} đoạn bị máy chủ từ chối — vẫn giữ trên máy.
				</Alert>
			) : null}
		</Card>
	);
}
