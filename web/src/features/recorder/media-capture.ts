/**
 * Gets the media of the chosen sources and turns them into one stream for
 * MediaRecorder. Three independent sources: microphone (getUserMedia),
 * system sound and screen video (both getDisplayMedia). Several audio
 * tracks are mixed through Web Audio into one; a screen track makes the
 * recording a video.
 *
 * Web limit: system sound only comes with a screen share (the browser's
 * picker always offers video); without "screen" the video track is
 * dropped. System sound works best in Chrome/Edge.
 */
import type { RecordSources } from "./recorder-messages";

export interface CaptureHandlers {
	/** The user pressed the browser's "Stop sharing". */
	onShareEnded: () => void;
	/** The mic track ended on its own (iOS lock screen, device unplugged). */
	onMicEnded: () => void;
}

export interface Capture {
	/** What MediaRecorder records: screen video plus one (mixed) audio track. */
	stream: MediaStream;
	/** Every track the browser granted; all are stopped when recording ends. */
	sourceTracks: MediaStreamTrack[];
	/** Granted audio inputs before mixing (the mixed track never "ends"). */
	sourceAudio: MediaStreamTrack[];
	/** Shared by the level meter and the mixer; null without audio. */
	audioContext: AudioContext | null;
	/** Reads the level of the original tracks (not connected to speakers). */
	analyser: AnalyserNode | null;
}

/** Screen capture exists (Chrome/Edge/Firefox desktop; not iOS). */
export function supportsScreenCapture(): boolean {
	return (
		typeof navigator !== "undefined" &&
		typeof navigator.mediaDevices?.getDisplayMedia === "function"
	);
}

/** Best webm type the browser can record. */
export function pickMimeType(hasVideo: boolean): string {
	const candidates = hasVideo
		? ["video/webm;codecs=vp9,opus", "video/webm;codecs=vp8,opus", "video/webm"]
		: ["audio/webm;codecs=opus", "audio/webm"];
	return (
		candidates.find((type) => MediaRecorder.isTypeSupported(type)) ??
		(hasVideo ? "video/webm" : "audio/webm")
	);
}

/**
 * Asks for the sources. getDisplayMedia goes first (it is the most
 * sensitive to user activation). A denied mic is ignored when another
 * source was granted, and rethrown when it was the only one.
 */
export async function captureSources(
	sources: RecordSources,
	handlers: CaptureHandlers,
): Promise<Capture> {
	const audioTracks: MediaStreamTrack[] = [];
	let videoTrack: MediaStreamTrack | null = null;

	if (sources.systemAudio || sources.screen) {
		const display = await navigator.mediaDevices.getDisplayMedia({
			video: { frameRate: { ideal: 30, max: 30 } },
			// Ask for audio only when system sound is wanted.
			audio: sources.systemAudio,
		});
		const displayVideo = display.getVideoTracks()[0] ?? null;
		if (sources.screen && displayVideo) {
			videoTrack = displayVideo;
			displayVideo.addEventListener("ended", handlers.onShareEnded);
		} else {
			displayVideo?.stop();
		}
		if (sources.systemAudio) {
			const systemTracks = display.getAudioTracks();
			audioTracks.push(...systemTracks);
			// No video kept: the audio track tells when sharing stops.
			if (!sources.screen) {
				systemTracks[0]?.addEventListener("ended", handlers.onShareEnded);
			}
		} else {
			for (const track of display.getAudioTracks()) track.stop();
		}
	}

	if (sources.mic) {
		try {
			const mic = await navigator.mediaDevices.getUserMedia({ audio: true });
			for (const track of mic.getAudioTracks()) {
				track.addEventListener("ended", handlers.onMicEnded);
				audioTracks.push(track);
			}
		} catch (error) {
			if (audioTracks.length === 0 && !videoTrack) throw error;
		}
	}

	const video = videoTrack ? [videoTrack] : [];
	const granted = {
		sourceTracks: [...video, ...audioTracks],
		sourceAudio: audioTracks,
	};
	if (audioTracks.length === 0) {
		return {
			...granted,
			stream: new MediaStream(video),
			audioContext: null,
			analyser: null,
		};
	}

	const audioContext = new AudioContext();
	const analyser = audioContext.createAnalyser();
	analyser.fftSize = 512;
	for (const track of audioTracks) {
		audioContext
			.createMediaStreamSource(new MediaStream([track]))
			.connect(analyser);
	}
	if (audioTracks.length === 1) {
		return {
			...granted,
			stream: new MediaStream([...video, ...audioTracks]),
			audioContext,
			analyser,
		};
	}
	// System sound + mic: one mixed track.
	const destination = audioContext.createMediaStreamDestination();
	for (const track of audioTracks) {
		audioContext
			.createMediaStreamSource(new MediaStream([track]))
			.connect(destination);
	}
	const mixed = destination.stream.getAudioTracks();
	return {
		...granted,
		stream: new MediaStream([...video, ...mixed]),
		audioContext,
		analyser,
	};
}

/** RMS level (0..1) of the analyser on every animation frame; returns stop. */
export function watchLevel(
	analyser: AnalyserNode,
	onLevel: (level: number) => void,
): () => void {
	const samples = new Uint8Array(analyser.fftSize);
	let frame = 0;
	const tick = (): void => {
		analyser.getByteTimeDomainData(samples);
		let sum = 0;
		for (const sample of samples) {
			const value = (sample - 128) / 128;
			sum += value * value;
		}
		onLevel(Math.sqrt(sum / samples.length));
		frame = requestAnimationFrame(tick);
	};
	tick();
	return () => cancelAnimationFrame(frame);
}
