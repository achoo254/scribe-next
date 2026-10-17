import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../lib/api";
import { BookmarkQueue } from "../transcript-docs/bookmark-queue";
import { NativeBookmarkSync, parseNativeBookmark } from "./native-bookmarks";

type Handler = (data: Record<string, unknown>) => void;

const ID_A = "aaaaaaaaaaaaaaaaaaaaaaaa";
const ID_B = "bbbbbbbbbbbbbbbbbbbbbbbb";

function saved(id: string, positionS: number, sessionId = "sess-1") {
	return { id, sessionId, positionS, createdAt: "2026-10-17T01:00:00Z" };
}

/** Swift's side: a disk store that forgets only acknowledged ids. */
function fakePlugin(initial: unknown[] = []) {
	let stored = [...initial];
	const handlers = new Map<string, Set<Handler>>();
	const plugin = {
		takePendingBookmarks: vi.fn(async () => ({ bookmarks: [...stored] })),
		ackBookmarks: vi.fn(async ({ ids }: { userKey: string; ids: string[] }) => {
			stored = stored.filter(
				(item) =>
					!(
						typeof item === "object" &&
						item !== null &&
						"id" in item &&
						ids.includes(String(item.id))
					),
			);
		}),
		addListener: vi.fn(async (event: string, handler: Handler) => {
			const set = handlers.get(event) ?? new Set<Handler>();
			set.add(handler);
			handlers.set(event, set);
			return { remove: async () => void set.delete(handler) };
		}),
	};
	return {
		plugin,
		stored: () => stored,
		add: (item: unknown) => stored.push(item),
		emit: (event: string, data: Record<string, unknown> = {}) => {
			for (const handler of handlers.get(event) ?? []) handler(data);
		},
	};
}

async function settle(): Promise<void> {
	for (let i = 0; i < 10; i += 1) await Promise.resolve();
}

afterEach(() => {
	vi.useRealTimers();
});

describe("parseNativeBookmark", () => {
	it("keeps well-formed bookmarks and rejects the rest", () => {
		expect(parseNativeBookmark(saved(ID_A, 12.5))).toEqual(saved(ID_A, 12.5));
		expect(parseNativeBookmark(saved("not-an-id", 1))).toBeNull();
		expect(parseNativeBookmark(saved(ID_A, Number.NaN))).toBeNull();
		expect(parseNativeBookmark(saved(ID_A, 1, ""))).toBeNull();
		expect(parseNativeBookmark(null)).toBeNull();
	});
});

describe("NativeBookmarkSync", () => {
	it("sends Swift's bookmarks at launch with their own ids, then acknowledges them", async () => {
		const swift = fakePlugin([saved(ID_A, 42), saved(ID_B, 90, "sess-2")]);
		const send = vi.fn(async () => ({}));
		const sync = new NativeBookmarkSync(
			swift.plugin,
			new BookmarkQueue(send),
			"user-1",
		);
		sync.start();
		await settle();
		expect(send.mock.calls).toEqual([
			["s:sess-1", 42, ID_A],
			["s:sess-2", 90, ID_B],
		]);
		expect(swift.plugin.takePendingBookmarks).toHaveBeenCalledWith({
			userKey: "user-1",
		});
		expect(swift.stored()).toEqual([]);
		sync.stop();
	});

	it("keeps a bookmark on the phone until the server could be reached", async () => {
		vi.useFakeTimers();
		const swift = fakePlugin([saved(ID_A, 5)]);
		const send = vi
			.fn<(key: string, at: number, id: string) => Promise<unknown>>()
			.mockRejectedValueOnce(new ApiError(0, "mất mạng"))
			.mockResolvedValue({});
		const sync = new NativeBookmarkSync(
			swift.plugin,
			new BookmarkQueue(send, [1_000]),
			"user-1",
		);
		sync.start();
		await vi.advanceTimersByTimeAsync(0);
		expect(swift.plugin.ackBookmarks).not.toHaveBeenCalled();
		expect(swift.stored()).toHaveLength(1);
		// Pulled again meanwhile (page visible): queued once, not twice.
		await sync.pull();
		await vi.advanceTimersByTimeAsync(1_000);
		expect(send).toHaveBeenCalledTimes(2);
		expect(swift.stored()).toEqual([]);
		sync.stop();
	});

	it("pulls when Swift says one was added and when the recording stops", async () => {
		const swift = fakePlugin();
		const send = vi.fn(async () => ({}));
		const sync = new NativeBookmarkSync(
			swift.plugin,
			new BookmarkQueue(send),
			"user-1",
		);
		sync.start();
		await settle();
		swift.add(saved(ID_A, 3));
		swift.emit("bookmarkAdded", saved(ID_A, 3));
		await settle();
		expect(send).toHaveBeenCalledWith("s:sess-1", 3, ID_A);

		swift.add(saved(ID_B, 8));
		swift.emit("state", { state: "recording" });
		await settle();
		expect(send).toHaveBeenCalledTimes(1);
		swift.emit("state", { state: "idle" });
		await settle();
		expect(send).toHaveBeenLastCalledWith("s:sess-1", 8, ID_B);
		expect(swift.stored()).toEqual([]);
		sync.stop();
	});

	it("drops malformed entries from Swift instead of pulling them forever", async () => {
		const swift = fakePlugin([saved("zz", 1), saved(ID_A, 2)]);
		const send = vi.fn(async () => ({}));
		const sync = new NativeBookmarkSync(
			swift.plugin,
			new BookmarkQueue(send),
			"user-1",
		);
		sync.start();
		await settle();
		expect(send).toHaveBeenCalledTimes(1);
		expect(swift.stored()).toEqual([]);
		sync.stop();
	});

	it("leaves the bookmarks on the phone when the app build cannot hand them over", async () => {
		const swift = fakePlugin();
		swift.plugin.takePendingBookmarks.mockRejectedValue(
			new Error("not implemented"),
		);
		const queue = { mark: vi.fn() };
		const sync = new NativeBookmarkSync(swift.plugin, queue, "user-1");
		sync.start();
		await settle();
		expect(queue.mark).not.toHaveBeenCalled();
		sync.stop();
		// Stopped: no listener or pull left behind.
		swift.emit("bookmarkAdded");
		await settle();
		expect(swift.plugin.takePendingBookmarks).toHaveBeenCalledTimes(1);
	});
});
