/**
 * Live transcript while recording in the iOS app ("Chép trực tiếp"). Swift
 * streams the audio to the realtime provider the backend picks (ElevenLabs
 * Scribe v2 Realtime by default) and keeps the sentences next to the
 * session (web/ios/App/App/Recorder/RealtimeTranscriber.swift); this is
 * only a preview — the full transcript still runs after the upload.
 *
 * Swift is the source of truth: `transcript` events arrive while the page
 * is on screen, and when it comes back (the WebView is suspended while the
 * phone is locked) the state is REPLACED by `getTranscript()`, never merged.
 */
import type { RecorderNativePlugin } from "./native-recorder";

export interface LiveSentence {
	/** Provider sentence id, or `gap-…` for a stretch the stream missed. */
	id: string;
	/** Session position (seconds of recorded audio) where it starts. */
	atS: number;
	text: string;
	/** A missed stretch ends here (`text` empty): the full transcript fills it. */
	gapUntilS?: number;
	/** Still being transcribed; replaced by the finished sentence. */
	partial?: boolean;
}

export interface LiveTranscriptState {
	/** Recording session these sentences belong to; null when none. */
	sessionId: string | null;
	/** Sorted by `atS`. */
	sentences: LiveSentence[];
	connected: boolean;
	/** Session position where the stream was lost; null while fine. */
	disconnectedAtS: number | null;
	/** Why the preview does not work (shown as is), empty when fine. */
	error: string;
	/** The stream ended for this recording. */
	stopped: boolean;
}

export const EMPTY_LIVE: LiveTranscriptState = {
	sessionId: null,
	sentences: [],
	connected: false,
	disconnectedAtS: null,
	error: "",
	stopped: false,
};

function finite(value: unknown): number | null {
	return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function record(value: unknown): Record<string, unknown> {
	return typeof value === "object" && value !== null
		? (value as Record<string, unknown>)
		: {};
}

/** A sentence or gap from Swift, or null when malformed. */
export function parseSentence(raw: unknown): LiveSentence | null {
	const data = record(raw);
	const atS = finite(data.atS);
	if (typeof data.id !== "string" || !data.id || atS === null) return null;
	const text = typeof data.text === "string" ? data.text : "";
	const gapUntilS = finite(data.gapUntilS);
	if (gapUntilS !== null) return { id: data.id, atS, text, gapUntilS };
	return text ? { id: data.id, atS, text } : null;
}

function statusOf(
	raw: unknown,
): Pick<
	LiveTranscriptState,
	"connected" | "disconnectedAtS" | "error" | "stopped"
> {
	const data = record(raw);
	return {
		connected: data.connected === true,
		disconnectedAtS: finite(data.disconnectedAtS),
		error: typeof data.error === "string" ? data.error : "",
		stopped: data.stopped === true,
	};
}

/** Inserts or replaces by id, keeping the list sorted by position. */
function upsert(list: LiveSentence[], next: LiveSentence): LiveSentence[] {
	const rest = list.filter((s) => s.id !== next.id);
	// A finished sentence never goes back to partial (late delta).
	if (next.partial && rest.length < list.length) {
		const current = list.find((s) => s.id === next.id);
		if (current && !current.partial) return list;
	}
	return [...rest, next].sort((a, b) => a.atS - b.atS);
}

/** State from a `getTranscript()` reply (replaces the page's copy). */
export function fromSnapshot(raw: unknown): LiveTranscriptState {
	const data = record(raw);
	const sessionId =
		typeof data.sessionId === "string" && data.sessionId
			? data.sessionId
			: null;
	const entries = Array.isArray(data.entries) ? data.entries : [];
	const sentences = entries
		.map(parseSentence)
		.filter((s): s is LiveSentence => s !== null)
		.sort((a, b) => a.atS - b.atS);
	return { sessionId, sentences, ...statusOf(data.status) };
}

/** Applies one `transcript` event. An event of another session starts afresh. */
export function applyEvent(
	state: LiveTranscriptState,
	raw: Record<string, unknown>,
): LiveTranscriptState {
	const sessionId =
		typeof raw.sessionId === "string" && raw.sessionId ? raw.sessionId : null;
	if (!sessionId) return state;
	const base =
		state.sessionId === sessionId ? state : { ...EMPTY_LIVE, sessionId };
	switch (raw.type) {
		case "status":
			return { ...base, ...statusOf(raw) };
		case "sentence": {
			const sentence = parseSentence(raw);
			return sentence
				? { ...base, sentences: upsert(base.sentences, sentence) }
				: base;
		}
		case "partial": {
			const sentence = parseSentence(raw);
			return sentence
				? {
						...base,
						sentences: upsert(base.sentences, { ...sentence, partial: true }),
					}
				: base;
		}
		default:
			return base;
	}
}

/** External store over the plugin, read with `useSyncExternalStore`. */
export class LiveTranscript {
	private state: LiveTranscriptState = EMPTY_LIVE;
	private readonly listeners = new Set<() => void>();
	private remove: (() => Promise<void>) | null = null;
	private generation = 0;

	constructor(
		private readonly plugin: Pick<
			RecorderNativePlugin,
			"addListener" | "getTranscript"
		>,
	) {}

	getState = (): LiveTranscriptState => this.state;

	subscribe = (listener: () => void): (() => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	attach = async (): Promise<void> => {
		const generation = this.generation;
		const handle = await this.plugin.addListener("transcript", (data) =>
			this.set(applyEvent(this.state, data)),
		);
		if (generation !== this.generation) {
			void handle.remove();
			return;
		}
		this.remove = handle.remove;
		await this.refresh();
	};

	/** Replaces the state with Swift's (after the WebView was suspended). */
	refresh = async (): Promise<void> => {
		try {
			this.set(fromSnapshot(await this.plugin.getTranscript()));
		} catch {
			// App builds without the live transcript: nothing to show.
		}
	};

	/** A new recording starts: drop the previous one's sentences. */
	reset = (): void => this.set(EMPTY_LIVE);

	dispose = (): void => {
		this.generation += 1;
		const remove = this.remove;
		this.remove = null;
		if (remove) void remove();
	};

	private set(next: LiveTranscriptState): void {
		this.state = next;
		for (const listener of this.listeners) listener();
	}
}
