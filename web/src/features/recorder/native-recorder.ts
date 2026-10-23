/**
 * Recording in the iOS app: the Swift engine (web/ios/App/App/Recorder)
 * records 1-minute m4a segments that keep going on the lock screen and
 * uploads each one as it closes. Swift is the source of truth (plan D2):
 * the WebView is suspended while the phone is locked, so this store
 * re-reads `getStatus()` whenever the page becomes visible.
 *
 * Same store shape as the browser `Recorder` (`getState`, `subscribe`,
 * `getLevel`, `subscribeLevel`) so the UI reads both the same way.
 */
import { registerPlugin } from "@capacitor/core";
import { activeTranslator } from "../../i18n/active-translator";
import { errorMessage } from "../../lib/api";

type NativeState = "idle" | "starting" | "recording" | "stopping";

interface NativeStatus {
	state: NativeState;
	/**
	 * Paused by the user (lock screen, AirPods, the app). Absent from app
	 * builds that cannot pause: the page then shows no pause button.
	 */
	paused?: boolean;
	elapsedS: number;
	/**
	 * Seconds of audio kept in the session (closed segments plus the open
	 * one): where the transcript is now. Absent from app builds before it.
	 */
	positionS?: number;
	/** Session being recorded; null when idle. */
	sessionId?: string | null;
	uploadsWaiting: number;
	uploadsFailed: number;
	notice: string;
}

/**
 * A bookmark pressed on the lock screen, in Control Center or with AirPods
 * while the page was suspended; Swift keeps it on disk until acknowledged.
 */
export interface NativeBookmark {
	/** 24 hex digits: the server saves it once however often it is sent. */
	id: string;
	/** Its transcript is `s:<sessionId>`. */
	sessionId: string;
	/** Session position when pressed, from the recorder's own clock. */
	positionS: number;
	createdAt: string;
}

export interface RecorderNativePlugin {
	start(options: {
		userKey: string;
		segmentMinutes?: number;
		/** Also stream the audio for a live transcript ("Chép trực tiếp"). */
		realtime?: boolean;
	}): Promise<{
		sessionId: string;
	}>;
	stop(): Promise<NativeStatus>;
	/** Stops writing without ending the session (only when `paused` is reported). */
	pause(): Promise<NativeStatus>;
	/** Writes again, into a new segment. */
	resume(): Promise<NativeStatus>;
	getStatus(): Promise<NativeStatus>;
	syncUploads(options: { userKey: string }): Promise<{
		lostSegments: number;
		lostSecondsMax: number;
	}>;
	/**
	 * Stops recording and uploads. `discardUserKey` (the account was deleted):
	 * also removes the segments, live transcripts and bookmarks the phone keeps
	 * for that account, so a later sign-in never uploads them.
	 */
	signOut(options?: { discardUserKey?: string }): Promise<void>;
	/** Bookmarks saved by Swift and not acknowledged yet (absent from older app builds). */
	takePendingBookmarks(options: { userKey: string }): Promise<{
		bookmarks: unknown[];
	}>;
	/** Swift forgets these bookmarks: the web queue took care of them. */
	ackBookmarks(options: { userKey: string; ids: string[] }): Promise<void>;
	/**
	 * Live transcript of the current or last recording that asked for it
	 * (absent from older app builds): `{ sessionId, entries, status }`.
	 */
	getTranscript(): Promise<{
		sessionId: string | null;
		entries: unknown[];
		status: unknown;
	}>;
	addListener(
		event:
			| "state"
			| "paused"
			| "elapsed"
			| "level"
			| "segment"
			| "notice"
			| "failure"
			| "uploads"
			| "uploaded"
			| "needsSignIn"
			| "quotaFull"
			| "bookmarkAdded"
			| "transcript",
		handler: (data: Record<string, unknown>) => void,
	): Promise<{ remove: () => Promise<void> }>;
}

export const NativeRecorderPlugin =
	registerPlugin<RecorderNativePlugin>("Recorder");

export interface NativeRecorderState {
	recording: boolean;
	starting: boolean;
	finalizing: boolean;
	/** Recording paused by the user; the clock stands still. */
	paused: boolean;
	/** This app build can pause (it reports `paused` in its status). */
	canPause: boolean;
	elapsedS: number;
	/** Session being recorded (its transcript is `s:<sessionId>`); null when idle. */
	sessionId: string | null;
	/** Segments recorded but not uploaded yet (kept on the phone meanwhile). */
	uploadsWaiting: number;
	/** Segments the server refused for good; their files stay on the phone. */
	uploadsFailed: number;
	/** Interruption notices ("Tạm dừng: cuộc gọi…"), not errors. */
	notice: string;
	error: string;
}

const IDLE: NativeRecorderState = {
	recording: false,
	starting: false,
	finalizing: false,
	paused: false,
	canPause: false,
	elapsedS: 0,
	sessionId: null,
	uploadsWaiting: 0,
	uploadsFailed: 0,
	notice: "",
	error: "",
};

function numberOf(value: unknown): number {
	return typeof value === "number" && Number.isFinite(value) ? value : 0;
}

function textOf(value: unknown): string {
	return typeof value === "string" ? value : "";
}

/** A session id from the plugin, or null for anything else. */
function sessionOf(value: unknown): string | null {
	return typeof value === "string" && value ? value : null;
}

function flagsOf(
	state: string,
): Pick<NativeRecorderState, "recording" | "starting" | "finalizing"> {
	return {
		recording: state === "recording",
		starting: state === "starting",
		finalizing: state === "stopping",
	};
}

/** `paused` and whether the build can pause, from a status of Swift. */
function pauseOf(
	status: NativeStatus,
): Pick<NativeRecorderState, "paused" | "canPause"> {
	const canPause = typeof status.paused === "boolean";
	return { paused: canPause && status.paused === true, canPause };
}

/** Stable per-account folder name on the phone: SHA-256 of the email. */
export async function userKeyOf(email: string): Promise<string> {
	const bytes = new TextEncoder().encode(email.trim().toLowerCase());
	const digest = await crypto.subtle.digest("SHA-256", bytes);
	return [...new Uint8Array(digest)]
		.slice(0, 12)
		.map((b) => b.toString(16).padStart(2, "0"))
		.join("");
}

export interface NativeRecorderEvents {
	/** A segment reached the server: the history should refresh. */
	onUploaded?: () => void;
	/** The session cookie expired: uploads wait for a new sign-in. */
	onNeedsSignIn?: () => void;
	onQuotaFull?: (message: string) => void;
}

export class NativeRecorder {
	private state: NativeRecorderState = IDLE;
	private level = 0;
	private readonly listeners = new Set<() => void>();
	private readonly levelListeners = new Set<() => void>();
	private readonly removers: Array<() => Promise<void>> = [];
	/** Bumped by `dispose`: an `attach` still awaiting listeners from an
	 * earlier mount drops them instead of leaking them. */
	private generation = 0;

	constructor(
		private readonly plugin: RecorderNativePlugin = NativeRecorderPlugin,
		private readonly events: NativeRecorderEvents = {},
	) {}

	getState = (): NativeRecorderState => this.state;
	getLevel = (): number => this.level;

	subscribe = (listener: () => void): (() => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	subscribeLevel = (listener: () => void): (() => void) => {
		this.levelListeners.add(listener);
		return () => this.levelListeners.delete(listener);
	};

	/** Starts listening to the plugin and reads the current status. */
	attach = async (): Promise<void> => {
		const generation = this.generation;
		const on = async (
			event: Parameters<RecorderNativePlugin["addListener"]>[0],
			handler: (data: Record<string, unknown>) => void,
		) => {
			const handle = await this.plugin.addListener(event, handler);
			if (generation === this.generation) this.removers.push(handle.remove);
			else void handle.remove();
		};
		await on("state", (data) => {
			const flags = flagsOf(textOf(data.state));
			// A session that ended (or has not begun) is never paused.
			const paused = flags.recording && this.state.paused;
			this.set({ ...this.state, ...flags, paused });
		});
		await on("paused", (data) =>
			this.set({ ...this.state, paused: data.paused === true }),
		);
		await on("elapsed", (data) =>
			this.set({ ...this.state, elapsedS: numberOf(data.elapsedS) }),
		);
		await on("level", (data) => this.setLevel(numberOf(data.level)));
		await on("notice", (data) =>
			this.set({ ...this.state, notice: textOf(data.message) }),
		);
		await on("failure", (data) =>
			this.set({
				...this.state,
				recording: false,
				starting: false,
				paused: false,
				error: textOf(data.message),
			}),
		);
		await on("uploads", (data) =>
			this.set({
				...this.state,
				uploadsWaiting: numberOf(data.waiting),
				uploadsFailed: numberOf(data.failed),
			}),
		);
		await on("uploaded", () => this.events.onUploaded?.());
		await on("needsSignIn", () => this.events.onNeedsSignIn?.());
		await on("quotaFull", (data) =>
			this.events.onQuotaFull?.(textOf(data.message)),
		);
		if (generation === this.generation) await this.refresh();
	};

	/** Re-reads Swift's status (after the WebView was suspended). */
	refresh = async (): Promise<void> => {
		const status = await this.plugin.getStatus();
		this.set({
			...this.state,
			...flagsOf(status.state),
			...pauseOf(status),
			elapsedS: numberOf(status.elapsedS),
			sessionId: sessionOf(status.sessionId),
			uploadsWaiting: numberOf(status.uploadsWaiting),
			uploadsFailed: numberOf(status.uploadsFailed),
			notice: textOf(status.notice),
		});
	};

	start = async (
		userKey: string,
		segmentMinutes: number,
		realtime = false,
	): Promise<void> => {
		if (this.state.recording || this.state.starting) return;
		this.set({
			...this.state,
			starting: true,
			error: "",
			notice: "",
			elapsedS: 0,
			sessionId: null,
		});
		try {
			const { sessionId } = await this.plugin.start({
				userKey,
				segmentMinutes,
				...(realtime ? { realtime } : {}),
			});
			this.set({ ...this.state, sessionId: sessionOf(sessionId) });
		} catch (cause) {
			this.set({
				...this.state,
				starting: false,
				error: errorMessage(
					cause,
					activeTranslator().t("recorder.startError.failed"),
				),
			});
		}
	};

	/**
	 * Seconds from the start of the session, as the transcript counts them,
	 * read from Swift at the moment of asking. The WebView's own copy of the
	 * clock goes stale while the phone is locked (Swift stops sending
	 * `elapsed` then, and queued events replay late), so it is not used.
	 * Older app builds without `positionS`: Swift's recording clock. Never
	 * throws: if Swift cannot answer, the last clock the page saw.
	 */
	position = async (): Promise<number> => {
		try {
			const status = await this.plugin.getStatus();
			return typeof status.positionS === "number" &&
				Number.isFinite(status.positionS)
				? status.positionS
				: numberOf(status.elapsedS);
		} catch {
			return this.state.elapsedS;
		}
	};

	/** Pauses when recording, resumes when paused (builds that can pause). */
	togglePause = async (): Promise<void> => {
		if (!this.state.recording || !this.state.canPause) return;
		try {
			const status = this.state.paused
				? await this.plugin.resume()
				: await this.plugin.pause();
			this.set({ ...this.state, ...flagsOf(status.state), ...pauseOf(status) });
		} catch (cause) {
			this.set({
				...this.state,
				error: errorMessage(cause, activeTranslator().t("recorder.pauseError")),
			});
		}
	};

	stop = async (): Promise<void> => {
		if (!this.state.recording) return;
		const status = await this.plugin.stop();
		this.set({
			...this.state,
			recording: false,
			finalizing: false,
			paused: false,
			uploadsWaiting: numberOf(status.uploadsWaiting),
		});
		this.setLevel(0);
	};

	dispose = (): void => {
		this.generation += 1;
		for (const remove of this.removers.splice(0)) void remove();
	};

	private set(next: NativeRecorderState): void {
		this.state = next;
		for (const listener of this.listeners) listener();
	}

	private setLevel(level: number): void {
		this.level = level;
		for (const listener of this.levelListeners) listener();
	}
}
