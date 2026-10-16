import { ApiError, errorMessage } from "../../lib/api";
import { addBookmark } from "./transcript-docs-api";

/**
 * Bookmarks pressed while recording. They must never get in the way of the
 * recording: `mark` returns at once, requests go one at a time in the
 * background, and a failure the server may get over (no network, 429,
 * 5xx) is retried with a growing delay. A refusal (400, 404, 409 full) is
 * kept as "failed" with the server's reason, so the user knows.
 *
 * Counts are of the current recording (`reset` when one starts); items of
 * an earlier one keep retrying in the background.
 */

/** Waits before each retry; the last one repeats. */
export const RETRY_DELAYS_MS = [2_000, 5_000, 10_000, 30_000] as const;

export interface BookmarkQueueState {
	/** Pressed during the current recording (saved or still waiting). */
	marked: number;
	/** Not saved yet; retried in the background. */
	waiting: number;
	/** Refused by the server for good. */
	failed: number;
	/** Reason of the last refusal; empty when none. */
	lastError: string;
}

const EMPTY: BookmarkQueueState = {
	marked: 0,
	waiting: 0,
	failed: 0,
	lastError: "",
};

type ItemStatus = "pending" | "sent" | "failed";

interface Item {
	key: string;
	at: number;
	recording: number;
	status: ItemStatus;
}

export type SendBookmark = (key: string, at: number) => Promise<unknown>;

/** True when trying again later may work. */
function isTransient(error: unknown): boolean {
	if (!(error instanceof ApiError)) return true;
	return error.status === 0 || error.status === 429 || error.status >= 500;
}

export class BookmarkQueue {
	private items: Item[] = [];
	private state: BookmarkQueueState = EMPTY;
	private recording = 0;
	private lastError = "";
	private attempt = 0;
	private sending = false;
	private timer: ReturnType<typeof setTimeout> | null = null;
	private disposed = false;
	private readonly listeners = new Set<() => void>();

	constructor(
		private readonly send: SendBookmark = (key, at) => addBookmark(key, at),
		private readonly delays: readonly number[] = RETRY_DELAYS_MS,
	) {}

	getState = (): BookmarkQueueState => this.state;

	subscribe = (listener: () => void): (() => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	/** A new recording starts: its counts start from zero. */
	reset = (): void => {
		this.recording += 1;
		this.lastError = "";
		this.items = this.items.filter((item) => item.status === "pending");
		this.publish();
	};

	/** Queues a bookmark at `at` seconds of the transcript `key`. */
	mark = (key: string, at: number): void => {
		if (this.disposed || !Number.isFinite(at)) return;
		this.items.push({
			key,
			at: Math.max(0, at),
			recording: this.recording,
			status: "pending",
		});
		this.publish();
		this.kick();
	};

	/** Stops sending (signed out, unmounted); `resume` undoes it. */
	dispose = (): void => {
		this.disposed = true;
		if (this.timer !== null) clearTimeout(this.timer);
		this.timer = null;
	};

	/** Sends again after `dispose` (a remount in React's strict mode). */
	resume = (): void => {
		this.disposed = false;
		this.kick();
	};

	private kick(): void {
		if (this.sending || this.timer !== null || this.disposed) return;
		void this.drain();
	}

	private async drain(): Promise<void> {
		this.sending = true;
		try {
			for (;;) {
				const next = this.items.find((item) => item.status === "pending");
				if (!next || this.disposed) return;
				try {
					await this.send(next.key, next.at);
					next.status = "sent";
					this.attempt = 0;
				} catch (cause) {
					if (isTransient(cause)) {
						this.retryLater();
						return;
					}
					next.status = "failed";
					if (next.recording === this.recording) {
						this.lastError = errorMessage(cause);
					}
				}
				this.publish();
			}
		} finally {
			this.sending = false;
		}
	}

	private retryLater(): void {
		if (this.disposed) return;
		const delay =
			this.delays[Math.min(this.attempt, this.delays.length - 1)] ?? 0;
		this.attempt += 1;
		this.timer = setTimeout(() => {
			this.timer = null;
			this.kick();
		}, delay);
	}

	private publish(): void {
		// Saved bookmarks of earlier recordings are no longer counted anywhere.
		this.items = this.items.filter(
			(item) => item.recording === this.recording || item.status === "pending",
		);
		const current = this.items.filter(
			(item) => item.recording === this.recording,
		);
		const count = (status: ItemStatus) =>
			current.filter((item) => item.status === status).length;
		this.state = {
			marked: count("sent") + count("pending"),
			waiting: count("pending"),
			failed: count("failed"),
			lastError: this.lastError,
		};
		for (const listener of this.listeners) listener();
	}
}
