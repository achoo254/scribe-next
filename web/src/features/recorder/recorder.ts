import { activeTranslator } from "../../i18n/active-translator";
/**
 * Browser recording (Mode A), independent of React. MediaRecorder emits a
 * chunk every second; each chunk is uploaded in the background
 * (`RecordingChunkQueue`), so an abrupt stop loses at most a second. The
 * clock is a timer of its own, not a chunk count. Stopping drains the
 * queue and finalizes even after chunk errors, so the recording is kept.
 *
 * `getState()`/`subscribe()` change a few times per recording; the audio
 * level changes every frame and has its own `getLevel()`/`subscribeLevel()`.
 */
import { apiJson, errorMessage } from "../../lib/api";
import {
	type Capture,
	captureSources,
	pickMimeType,
	watchLevel,
} from "./media-capture";
import { type RecordSources, startErrorMessage } from "./recorder-messages";
import { RecordingChunkQueue } from "./recording-chunks";
import { ScreenWakeLock } from "./wake-lock";

export interface RecordResult {
	fileId: string;
	name: string;
	durationS: number;
}

export interface RecorderState {
	recording: boolean;
	starting: boolean;
	finalizing: boolean;
	elapsedS: number;
	/** Screen preview stream while recording a screen; null otherwise. */
	previewStream: MediaStream | null;
	/**
	 * Id the backend keeps for the file being recorded (its transcript is
	 * `f:<fileId>`, so bookmarks can be saved before it exists); null when
	 * not recording or when the backend does not say.
	 */
	fileId: string | null;
	error: string;
}

/** Bitrate caps keep a 1 s chunk far below the backend's 8 MB body limit. */
const SCREEN_VIDEO_BPS = 2_500_000;
const AUDIO_BPS = 128_000;

const IDLE: RecorderState = {
	recording: false,
	starting: false,
	finalizing: false,
	elapsedS: 0,
	previewStream: null,
	fileId: null,
	error: "",
};

export class Recorder {
	private state: RecorderState = IDLE;
	private level = 0;
	private readonly listeners = new Set<() => void>();
	private readonly levelListeners = new Set<() => void>();
	private readonly wakeLock = new ScreenWakeLock();

	private recorder: MediaRecorder | null = null;
	private capture: Capture | null = null;
	private stopMeter: (() => void) | null = null;
	private timer: ReturnType<typeof setInterval> | null = null;
	private sessionId: string | null = null;
	private chunks: RecordingChunkQueue | null = null;
	private interruptNote: string | null = null;

	constructor(private readonly fetcher: typeof fetch = fetch) {}

	getState = (): RecorderState => this.state;
	getLevel = (): number => this.level;

	subscribe = (listener: () => void): (() => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	subscribeLevel = (listener: () => void): (() => void) => {
		this.levelListeners.add(listener);
		return () => this.levelListeners.delete(listener);
	};

	/** Starts recording; failures end up in `state.error` (never throws). */
	start = async (sources: RecordSources): Promise<void> => {
		const { recording, starting, finalizing } = this.state;
		if (recording || starting || finalizing) return;
		this.set({ ...IDLE, starting: true });
		this.interruptNote = null;
		const name = activeTranslator().t("recorder.defaultName", {
			date: new Date().toLocaleString("vi-VN"),
		});
		try {
			// Media first, inside the click's user activation: on iOS WebKit
			// any await before getUserMedia loses it (NotAllowedError). The
			// backend session comes after, so a denied permission leaves none.
			this.capture = await captureSources(sources, {
				onShareEnded: () => void this.stop(),
				onMicEnded: () =>
					this.interrupt(activeTranslator().t("recorder.interrupt.micEnded")),
			});
			const { stream, analyser } = this.capture;
			if (analyser) this.stopMeter = watchLevel(analyser, this.setLevel);
			if (stream.getTracks().length === 0)
				throw new Error(activeTranslator().t("recorder.noSource"));

			const { sessionId, fileId } = await apiJson<{
				sessionId: string;
				fileId?: string;
			}>(
				"/api/recordings/init",
				{
					method: "POST",
					json: { name },
					fallbackError: activeTranslator().t("errors.requestFailed"),
				},
				this.fetcher,
			);
			this.sessionId = sessionId;
			const chunks = new RecordingChunkQueue(sessionId, this.fetcher);
			this.chunks = chunks;

			const hasVideo = stream.getVideoTracks().length > 0;
			const options: MediaRecorderOptions = {
				mimeType: pickMimeType(hasVideo),
				audioBitsPerSecond: AUDIO_BPS,
			};
			if (hasVideo) options.videoBitsPerSecond = SCREEN_VIDEO_BPS;
			const recorder = new MediaRecorder(stream, options);
			recorder.ondataavailable = (event) => {
				if (event.data.size > 0) chunks.enqueue(event.data);
			};
			recorder.onerror = () => {
				this.set({
					...this.state,
					error: activeTranslator().t("recorder.mediaError"),
				});
				void this.stop();
			};
			recorder.start(1000);
			this.recorder = recorder;

			this.timer = setInterval(() => {
				this.set({ ...this.state, elapsedS: this.state.elapsedS + 1 });
			}, 1000);
			void this.wakeLock.acquire();
			document.addEventListener("visibilitychange", this.onVisibility);
			this.set({
				...this.state,
				recording: true,
				previewStream: stream,
				fileId: typeof fileId === "string" && fileId ? fileId : null,
			});
		} catch (cause) {
			this.releaseMedia();
			this.chunks?.close();
			this.chunks = null;
			this.sessionId = null;
			this.recorder = null;
			this.set({ ...IDLE, error: startErrorMessage(cause, sources) });
		} finally {
			if (this.state.starting) this.set({ ...this.state, starting: false });
		}
	};

	/** Stops, waits for the upload queue, finalizes. Null on failure. */
	stop = async (): Promise<RecordResult | null> => {
		const recorder = this.recorder;
		if (!recorder || !this.state.recording) return null;
		this.clearTimer();
		this.set({ ...this.state, recording: false });
		// Wait for MediaRecorder to flush its last chunk.
		await new Promise<void>((resolve) => {
			recorder.onstop = () => resolve();
			recorder.stop();
		});
		this.releaseMedia();

		this.set({ ...this.state, finalizing: true });
		try {
			await this.chunks?.drain();
			if (!this.sessionId)
				throw new Error(activeTranslator().t("recorder.invalidSession"));
			// The client clock goes along: MediaRecorder webm often lacks a duration.
			const result = await apiJson<RecordResult>(
				"/api/recordings/finalize",
				{
					method: "POST",
					json: { session: this.sessionId, durationS: this.state.elapsedS },
					fallbackError: activeTranslator().t("errors.requestFailed"),
				},
				this.fetcher,
			);
			const note =
				this.interruptNote ??
				(this.chunks?.error
					? activeTranslator().t("recorder.chunksLost")
					: null);
			if (note) this.set({ ...this.state, error: note });
			return result;
		} catch (cause) {
			this.set({
				...this.state,
				error: errorMessage(
					cause,
					activeTranslator().t("recorder.finalizeFailed"),
				),
			});
			return null;
		} finally {
			this.sessionId = null;
			this.chunks = null;
			this.recorder = null;
			this.interruptNote = null;
			this.set({ ...this.state, finalizing: false });
		}
	};

	/** Releases mic/screen and timers (page left mid-recording). */
	dispose = (): void => {
		this.clearTimer();
		this.recorder = null;
		this.releaseMedia();
		if (this.state.recording) this.set({ ...this.state, recording: false });
	};

	private set(next: RecorderState): void {
		this.state = next;
		for (const listener of this.listeners) listener();
	}

	private setLevel = (level: number): void => {
		this.level = level;
		for (const listener of this.levelListeners) listener();
	};

	/** A source died mid-recording: stop, keep what was recorded, say why. */
	private interrupt(note: string): void {
		if (!this.state.recording) return;
		this.interruptNote = note;
		void this.stop();
	}

	/**
	 * Back in the foreground. iOS suspends the page on lock screen or app
	 * switch and takes the mic away; when every audio track ended meanwhile
	 * ("ended" may not fire during suspension), stop and save. Live tracks
	 * (desktop tab switch) only need the wake lock again.
	 */
	private onVisibility = (): void => {
		const audio = this.capture?.sourceAudio;
		if (
			document.visibilityState !== "visible" ||
			!this.state.recording ||
			!audio
		) {
			return;
		}
		if (
			audio.length > 0 &&
			audio.every((track) => track.readyState === "ended")
		) {
			this.interrupt(activeTranslator().t("recorder.interrupt.hidden"));
		} else {
			void this.wakeLock.acquire();
		}
	};

	private clearTimer(): void {
		if (this.timer !== null) clearInterval(this.timer);
		this.timer = null;
	}

	private releaseMedia(): void {
		document.removeEventListener("visibilitychange", this.onVisibility);
		void this.wakeLock.release();
		this.stopMeter?.();
		this.stopMeter = null;
		this.setLevel(0);
		if (this.capture) {
			for (const track of this.capture.stream.getTracks()) track.stop();
			for (const track of this.capture.sourceTracks) track.stop();
			void this.capture.audioContext?.close().catch(() => undefined);
			this.capture = null;
		}
		if (this.state.previewStream)
			this.set({ ...this.state, previewStream: null });
	}
}
