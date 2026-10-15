import {
	IconPlayerPauseFilled,
	IconPlayerPlayFilled,
	IconVolume,
	IconVolumeOff,
} from "@tabler/icons-react";
import { useRef, useState } from "react";
import { mediaUrl } from "../lib/download";
import { formatClock } from "../lib/format";
import { IconButton } from "./ui/icon-button";

export interface MediaPlayerProps {
	fileId: string;
	/** Screen recordings play in a native <video>. */
	video?: boolean;
	/**
	 * Duration measured by the backend. MediaRecorder webm often has no
	 * duration in its container (`audio.duration` is Infinity), so this is
	 * the total until the element reports a real one.
	 */
	durationS?: number;
	"data-id"?: string;
}

/**
 * Plays a stored file from `/api/files/:id` (HTTP Range, so seeking
 * works). Audio gets a compact bar whose volume slider pops up above the
 * speaker button instead of covering the seek bar.
 */
export function MediaPlayer({
	fileId,
	video = false,
	durationS,
	"data-id": dataId,
}: MediaPlayerProps) {
	if (video) {
		return (
			// biome-ignore lint/a11y/useMediaCaption: user recordings have no captions
			<video
				data-id={dataId}
				src={mediaUrl(fileId)}
				controls
				preload="metadata"
				className="block max-h-90 w-full rounded-md bg-surface-subtle"
			/>
		);
	}
	return <AudioPlayer fileId={fileId} durationS={durationS} data-id={dataId} />;
}

function AudioPlayer({
	fileId,
	durationS,
	"data-id": dataId,
}: Omit<MediaPlayerProps, "video">) {
	const audio = useRef<HTMLAudioElement>(null);
	const [playing, setPlaying] = useState(false);
	const [currentTime, setCurrentTime] = useState(0);
	const [duration, setDuration] = useState(
		durationS && durationS > 0 ? durationS : 0,
	);
	const [volume, setVolume] = useState(1);
	const [muted, setMuted] = useState(false);
	const hasDuration = Number.isFinite(duration) && duration > 0;

	function readDuration(): void {
		const element = audio.current;
		if (element && Number.isFinite(element.duration) && element.duration > 0) {
			setDuration(element.duration);
		}
	}

	function togglePlay(): void {
		const element = audio.current;
		if (!element) return;
		if (element.paused) void element.play();
		else element.pause();
	}

	return (
		<div
			data-id={dataId}
			className="flex w-full items-center gap-2 rounded-full border border-border-subtle bg-surface-subtle py-1 pr-2 pl-1"
		>
			{/* biome-ignore lint/a11y/useMediaCaption: user recordings have no captions */}
			<audio
				ref={audio}
				src={mediaUrl(fileId)}
				preload="metadata"
				onLoadedMetadata={readDuration}
				onDurationChange={readDuration}
				onTimeUpdate={(event) =>
					setCurrentTime(event.currentTarget.currentTime)
				}
				onPlay={() => setPlaying(true)}
				onPause={() => setPlaying(false)}
				onEnded={() => setPlaying(false)}
				onVolumeChange={(event) => {
					setVolume(event.currentTarget.volume);
					setMuted(event.currentTarget.muted);
				}}
			/>
			<IconButton
				size="sm"
				label={playing ? "Tạm dừng" : "Phát"}
				data-id={dataId ? `${dataId}-play` : undefined}
				icon={
					playing ? (
						<IconPlayerPauseFilled size={18} aria-hidden="true" />
					) : (
						<IconPlayerPlayFilled size={18} aria-hidden="true" />
					)
				}
				onClick={togglePlay}
			/>
			<span className="flex-none font-mono text-muted text-xs tabular-nums">
				{formatClock(currentTime)} / {hasDuration ? formatClock(duration) : "—"}
			</span>
			<input
				type="range"
				aria-label="Tua"
				data-id={dataId ? `${dataId}-seek` : undefined}
				min={0}
				max={hasDuration ? duration : 0}
				step={0.01}
				value={currentTime}
				disabled={!hasDuration}
				onChange={(event) => {
					if (audio.current)
						audio.current.currentTime = Number(event.target.value);
				}}
				className="min-w-0 flex-1 cursor-pointer accent-primary disabled:cursor-default disabled:opacity-45 pointer-coarse:h-11"
			/>
			<div className="group relative flex flex-none items-center">
				<IconButton
					size="sm"
					label={muted ? "Bật tiếng" : "Tắt tiếng"}
					data-id={dataId ? `${dataId}-mute` : undefined}
					icon={
						muted || volume === 0 ? (
							<IconVolumeOff size={18} aria-hidden="true" />
						) : (
							<IconVolume size={18} aria-hidden="true" />
						)
					}
					onClick={() => {
						if (audio.current) audio.current.muted = !audio.current.muted;
					}}
				/>
				<div className="invisible absolute bottom-full left-1/2 z-10 mb-1.5 -translate-x-1/2 rounded-sm border border-border bg-surface px-1.5 py-2.5 opacity-0 shadow-md transition-opacity group-focus-within:visible group-focus-within:opacity-100 group-hover:visible group-hover:opacity-100">
					<input
						type="range"
						aria-label="Âm lượng"
						data-id={dataId ? `${dataId}-volume` : undefined}
						min={0}
						max={1}
						step={0.05}
						value={muted ? 0 : volume}
						onChange={(event) => {
							const element = audio.current;
							if (!element) return;
							const next = Number(event.target.value);
							element.volume = next;
							element.muted = next === 0;
						}}
						className="h-22 w-1.5 cursor-pointer accent-primary [direction:rtl] [writing-mode:vertical-lr]"
					/>
				</div>
			</div>
		</div>
	);
}
