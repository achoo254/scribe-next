import type { MarkOptions } from "../transcript-docs/bookmark-queue";
import { transcriptKey } from "../transcript-docs/transcript-docs-api";
import type { NativeBookmark, RecorderNativePlugin } from "./native-recorder";

/**
 * Bookmarks pressed on the lock screen, in Control Center or with AirPods
 * while recording in the iOS app. The page is suspended then, so Swift saves
 * each one to disk; this pulls them into the page's bookmark queue (same
 * transcript key and idempotent id path as the "Đánh dấu" button) when the
 * page can run: at start (next launch), when it becomes visible again, when
 * Swift says one was added while on screen, and when the recording stops.
 * Swift forgets a bookmark only once the queue has settled it (sent, refused
 * or dropped), so a kill before the send never loses it — it is pulled again
 * and resent with the same id, which the server saves once.
 */

type Plugin = Pick<
	RecorderNativePlugin,
	"takePendingBookmarks" | "ackBookmarks" | "addListener"
>;

export interface BookmarkSink {
	mark(key: string, at: number, options?: MarkOptions): void;
}

const ID = /^[0-9a-f]{24}$/;

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null;
}

/** A well-formed bookmark from Swift, or null. */
export function parseNativeBookmark(value: unknown): NativeBookmark | null {
	if (!isRecord(value)) return null;
	const { id, sessionId, positionS, createdAt } = value;
	if (typeof id !== "string" || !ID.test(id)) return null;
	if (typeof sessionId !== "string" || !sessionId) return null;
	if (typeof positionS !== "number" || !Number.isFinite(positionS)) return null;
	return {
		id,
		sessionId,
		positionS: Math.max(0, positionS),
		createdAt: typeof createdAt === "string" ? createdAt : "",
	};
}

export class NativeBookmarkSync {
	private running: Promise<void> | null = null;
	private again = false;
	private stopped = true;
	private readonly removers: Array<() => Promise<void>> = [];
	/** Bumped by `stop`: listeners still being added are removed at once. */
	private generation = 0;

	constructor(
		private readonly plugin: Plugin,
		private readonly queue: BookmarkSink,
		private readonly userKey: string,
	) {}

	start = (): void => {
		if (!this.stopped) return;
		this.stopped = false;
		const generation = this.generation;
		const on = async (
			event: Parameters<Plugin["addListener"]>[0],
			handler: (data: Record<string, unknown>) => void,
		) => {
			const handle = await this.plugin.addListener(event, handler);
			if (generation === this.generation) this.removers.push(handle.remove);
			else void handle.remove();
		};
		// An older app build without these events rejects: the other triggers still pull.
		on("bookmarkAdded", () => void this.pull()).catch(() => undefined);
		on("state", (data) => {
			if (data.state === "idle") void this.pull();
		}).catch(() => undefined);
		document.addEventListener("visibilitychange", this.onVisible);
		void this.pull();
	};

	stop = (): void => {
		this.stopped = true;
		this.generation += 1;
		document.removeEventListener("visibilitychange", this.onVisible);
		for (const remove of this.removers.splice(0)) void remove();
	};

	/**
	 * Moves Swift's pending bookmarks into the queue. Calls during a pull
	 * run one more pull after it, so a bookmark added meanwhile is not missed.
	 */
	pull = (): Promise<void> => {
		if (this.stopped) return Promise.resolve();
		if (this.running) {
			this.again = true;
			return this.running;
		}
		this.running = (async () => {
			try {
				do {
					this.again = false;
					await this.pullOnce();
				} while (this.again && !this.stopped);
			} finally {
				this.running = null;
			}
		})();
		return this.running;
	};

	private onVisible = (): void => {
		if (document.visibilityState === "visible") void this.pull();
	};

	private async pullOnce(): Promise<void> {
		let pending: unknown[];
		try {
			({ bookmarks: pending } = await this.plugin.takePendingBookmarks({
				userKey: this.userKey,
			}));
		} catch {
			// Older app build, or Swift busy: the bookmarks stay on the phone
			// and the next trigger pulls them.
			return;
		}
		if (this.stopped || !Array.isArray(pending)) return;
		const malformed: string[] = [];
		for (const raw of pending) {
			const bookmark = parseNativeBookmark(raw);
			if (!bookmark) {
				// Could never be sent: forget it rather than pull it forever.
				if (isRecord(raw) && typeof raw.id === "string") malformed.push(raw.id);
				continue;
			}
			this.queue.mark(
				transcriptKey({ session: bookmark.sessionId }),
				bookmark.positionS,
				{ id: bookmark.id, onSettled: () => this.ack([bookmark.id]) },
			);
		}
		if (malformed.length > 0) this.ack(malformed);
	}

	/** A failed ack leaves them on the phone; the next pull settles them again. */
	private ack(ids: string[]): void {
		this.plugin
			.ackBookmarks({ userKey: this.userKey, ids })
			.catch(() => undefined);
	}
}
