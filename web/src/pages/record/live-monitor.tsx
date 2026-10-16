import { IconMicrophone } from "@tabler/icons-react";
import { useEffect, useRef } from "react";
import { MeterBar, type MeterTone } from "../../components/ui/meter-bar";
import {
	type LevelSource,
	useRecorderLevel,
} from "../../features/recorder/use-recorder";
import { activeTranslator } from "../../i18n/active-translator";
import { useT } from "../../i18n/i18n-context";
import type { MessageKey } from "../../i18n/message-types";

const SURFACES = {
	monitor: "record.live.monitor",
	window: "record.live.window",
	browser: "record.live.browser",
} as const satisfies Readonly<Record<string, MessageKey>>;

/** "Toàn màn hình · 1920×1080 · 30fps" from the screen track. */
export function describeScreen(stream: MediaStream | null): string {
	const settings = stream?.getVideoTracks()[0]?.getSettings() as
		| (MediaTrackSettings & { displaySurface?: string })
		| undefined;
	if (!settings) return "";
	const surface = settings.displaySurface ?? "";
	const parts = [
		Object.hasOwn(SURFACES, surface)
			? activeTranslator().t(SURFACES[surface as keyof typeof SURFACES])
			: "",
	];
	if (settings.width && settings.height)
		parts.push(`${settings.width}×${settings.height}`);
	if (settings.frameRate) parts.push(`${Math.round(settings.frameRate)}fps`);
	return parts.filter(Boolean).join(" · ");
}

/** RMS is usually 0.05–0.3: ×250 makes the bar readable, capped at 100. */
export function levelPercent(level: number): number {
	return Math.min(100, Math.round(level * 250));
}

export function levelTone(percent: number): MeterTone {
	if (percent >= 90) return "error";
	if (percent >= 70) return "warning";
	return "primary";
}

export function LevelMeter({ recorder }: { recorder: LevelSource }) {
	const t = useT();
	const percent = levelPercent(useRecorderLevel(recorder));
	return (
		<div className="flex items-center gap-2 text-icon-muted">
			<IconMicrophone size={18} aria-hidden="true" />
			<MeterBar
				role="meter"
				label={t("record.live.level")}
				value={percent}
				tone={levelTone(percent)}
				animate={false}
				data-id="record-level"
			/>
		</div>
	);
}

export interface LiveMonitorProps {
	recorder: LevelSource;
	stream: MediaStream | null;
	showPreview: boolean;
}

/** While recording: screen preview, live audio level, screen details. */
export function LiveMonitor({
	recorder,
	stream,
	showPreview,
}: LiveMonitorProps) {
	const video = useRef<HTMLVideoElement>(null);
	useEffect(() => {
		if (video.current) video.current.srcObject = stream;
	}, [stream]);
	const info = describeScreen(stream);

	return (
		<div
			data-id="record-live"
			className="grid gap-3 rounded-md border border-border-subtle bg-surface-subtle p-3"
		>
			{showPreview ? (
				<video
					ref={video}
					muted
					autoPlay
					playsInline
					data-id="record-preview"
					className="max-h-70 w-full rounded-sm bg-bg object-contain"
				/>
			) : null}
			<LevelMeter recorder={recorder} />
			{info ? <p className="m-0 text-muted text-sm">{info}</p> : null}
		</div>
	);
}
