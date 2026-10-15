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
import { errorMessage } from "../../lib/api";

type NativeState = "idle" | "starting" | "recording" | "stopping";

interface NativeStatus {
	state: NativeState;
	elapsedS: number;
	uploadsWaiting: number;
	uploadsFailed: number;
	notice: string;
}

export interface RecorderNativePlugin {
	start(options: { userKey: string; segmentMinutes?: number }): Promise<{
		sessionId: string;
	}>;
	stop(): Promise<NativeStatus>;
	getStatus(): Promise<NativeStatus>;
	syncUploads(options: { userKey: string }): Promise<{
		lostSegments: number;
		lostSecondsMax: number;
	}>;
	signOut(): Promise<void>;
	addListener(
		event:
			| "state"
			| "elapsed"
			| "level"
			| "notice"
			| "failure"
			| "uploads"
			| "uploaded"
			| "needsSignIn"
			| "quotaFull",
		handler: (data: Record<string, unknown>) => void,
	): Promise<{ remove: () => Promise<void> }>;
}

export const NativeRecorderPlugin =
	registerPlugin<RecorderNativePlugin>("Recorder");

export interface NativeRecorderState {
	recording: boolean;
	starting: boolean;
	finalizing: boolean;
	elapsedS: number;
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
	elapsedS: 0,
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

function flagsOf(
	state: string,
): Pick<NativeRecorderState, "recording" | "starting" | "finalizing"> {
	return {
		recording: state === "recording",
		starting: state === "starting",
		finalizing: state === "stopping",
	};
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
		await on("state", (data) =>
			this.set({ ...this.state, ...flagsOf(textOf(data.state)) }),
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
			elapsedS: numberOf(status.elapsedS),
			uploadsWaiting: numberOf(status.uploadsWaiting),
			uploadsFailed: numberOf(status.uploadsFailed),
			notice: textOf(status.notice),
		});
	};

	start = async (userKey: string, segmentMinutes: number): Promise<void> => {
		if (this.state.recording || this.state.starting) return;
		this.set({
			...this.state,
			starting: true,
			error: "",
			notice: "",
			elapsedS: 0,
		});
		try {
			await this.plugin.start({ userKey, segmentMinutes });
		} catch (cause) {
			this.set({
				...this.state,
				starting: false,
				error: errorMessage(cause, "Không bắt đầu ghi được."),
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
