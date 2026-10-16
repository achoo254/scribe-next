import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../lib/api";
import { BookmarkQueue, newBookmarkId } from "./bookmark-queue";

afterEach(() => {
	vi.useRealTimers();
});

/** Lets the queue's awaited sends settle. */
async function settle(): Promise<void> {
	for (let i = 0; i < 5; i += 1) await Promise.resolve();
}

describe("bookmark queue", () => {
	it("sends bookmarks in order and counts them", async () => {
		const sent: Array<[string, number]> = [];
		const queue = new BookmarkQueue(async (key, at) => {
			sent.push([key, at]);
		});
		queue.mark("f:abc", 12);
		queue.mark("f:abc", 30.5);
		expect(queue.getState()).toMatchObject({ marked: 2, waiting: 2 });
		await settle();
		expect(sent).toEqual([
			["f:abc", 12],
			["f:abc", 30.5],
		]);
		expect(queue.getState()).toEqual({
			marked: 2,
			waiting: 0,
			failed: 0,
			dropped: 0,
			lastError: "",
		});
	});

	it("retries a network failure later without losing the bookmark", async () => {
		vi.useFakeTimers();
		const send = vi
			.fn<(key: string, at: number, id: string) => Promise<unknown>>()
			.mockRejectedValueOnce(new ApiError(0, "mất mạng"))
			.mockRejectedValueOnce(new ApiError(503, "bận"))
			.mockResolvedValue({});
		const queue = new BookmarkQueue(send, [1_000, 2_000]);
		queue.mark("s:x", 5);
		await settle();
		expect(queue.getState()).toMatchObject({ marked: 1, waiting: 1 });
		await vi.advanceTimersByTimeAsync(1_000);
		expect(send).toHaveBeenCalledTimes(2);
		expect(queue.getState().waiting).toBe(1);
		await vi.advanceTimersByTimeAsync(2_000);
		expect(send).toHaveBeenCalledTimes(3);
		expect(queue.getState()).toMatchObject({
			marked: 1,
			waiting: 0,
			failed: 0,
		});
		// Every retry sends the same id: the server saves the bookmark once.
		const ids = send.mock.calls.map((call) => call[2]);
		expect(ids[0]).toMatch(/^[0-9a-f]{24}$/);
		expect(new Set(ids).size).toBe(1);
	});

	it("gives each bookmark its own ObjectId-shaped id", () => {
		const ids = new Set(Array.from({ length: 50 }, () => newBookmarkId()));
		expect(ids.size).toBe(50);
		for (const id of ids) expect(id).toMatch(/^[0-9a-f]{24}$/);
	});

	it("drops bookmarks of a recording that is gone (cut after recording) quietly", async () => {
		const send = vi
			.fn<(key: string, at: number, id: string) => Promise<unknown>>()
			.mockRejectedValueOnce(new ApiError(404, "transcript không tồn tại"))
			.mockResolvedValue({});
		const queue = new BookmarkQueue(send);
		queue.mark("f:old", 1);
		queue.mark("f:old", 2);
		await settle();
		expect(send).toHaveBeenCalledTimes(2);
		expect(queue.getState()).toEqual({
			marked: 1,
			waiting: 0,
			failed: 0,
			dropped: 1,
			lastError: "",
		});
	});

	it("keeps a refusal as failed with the server's reason and goes on", async () => {
		const send = vi
			.fn<(key: string, at: number, id: string) => Promise<unknown>>()
			.mockRejectedValueOnce(
				new ApiError(409, "Mỗi bản ghi có tối đa 200 dấu mốc."),
			)
			.mockResolvedValue({});
		const queue = new BookmarkQueue(send);
		queue.mark("s:x", 1);
		queue.mark("s:x", 2);
		await settle();
		expect(queue.getState()).toEqual({
			marked: 1,
			waiting: 0,
			failed: 1,
			dropped: 0,
			lastError: "Mỗi bản ghi có tối đa 200 dấu mốc.",
		});
	});

	it("counts from zero for a new recording", async () => {
		const queue = new BookmarkQueue(async () => undefined);
		queue.mark("s:old", 1);
		await settle();
		queue.reset();
		expect(queue.getState().marked).toBe(0);
		queue.mark("s:new", 3);
		expect(queue.getState().marked).toBe(1);
	});

	it("stops sending once disposed and carries on after resume", async () => {
		vi.useFakeTimers();
		const send = vi
			.fn<(key: string, at: number, id: string) => Promise<unknown>>()
			.mockRejectedValueOnce(new ApiError(0, "mất mạng"))
			.mockResolvedValue({});
		const queue = new BookmarkQueue(send, [1_000]);
		queue.mark("s:x", 1);
		await settle();
		queue.dispose();
		await vi.advanceTimersByTimeAsync(5_000);
		expect(send).toHaveBeenCalledTimes(1);
		queue.resume();
		await settle();
		expect(send).toHaveBeenCalledTimes(2);
		expect(queue.getState().waiting).toBe(0);
	});
});
