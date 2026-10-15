import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fakeFetch, jsonResponse } from "../../test/fake-fetch";
import {
	MAX_POLL_FAILS,
	POLL_INTERVAL_MS,
	SplitJob,
	type SplitJobState,
} from "./split-job";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

function track(fetcher: typeof fetch) {
	const states: SplitJobState[] = [];
	const job = new SplitJob((state) => states.push(state), fetcher);
	return { job, states };
}

describe("SplitJob", () => {
	it("creates the job with the same body as before and polls until done", async () => {
		const { fetcher, calls } = fakeFetch({
			"POST /api/jobs": jsonResponse({ jobId: "j1" }),
			"GET /api/jobs/j1": [
				jsonResponse({ status: "running", progress: 40 }),
				jsonResponse({
					status: "done",
					progress: 100,
					result: { segments: [{ name: "a_001.mp3", fileId: "s1" }] },
				}),
			],
		});
		const { job } = track(fetcher);
		await job.start({ inputFileId: "f1", criterion: "minutes", value: 10 });
		expect(calls[0]?.body).toEqual({
			inputFileId: "f1",
			criterion: "minutes",
			value: 10,
			recordingSplit: false,
		});
		await vi.runAllTimersAsync();

		expect(job.current).toMatchObject({
			jobId: "j1",
			status: "done",
			progress: 100,
			segments: [{ name: "a_001.mp3", fileId: "s1" }],
			error: "",
		});
		expect(calls.filter((call) => call.url === "/api/jobs/j1")).toHaveLength(2);
	});

	it("passes recordingSplit for cutting a recording", async () => {
		const { fetcher, calls } = fakeFetch({
			"POST /api/jobs": jsonResponse({ jobId: "j1" }),
			"GET /api/jobs/j1": jsonResponse({ status: "done", progress: 100 }),
		});
		await track(fetcher).job.start({
			inputFileId: "r1",
			criterion: "minutes",
			value: 5,
			recordingSplit: true,
		});
		expect(calls[0]?.body).toMatchObject({ recordingSplit: true });
	});

	it("shows the job's own failure message", async () => {
		const { fetcher } = fakeFetch({
			"POST /api/jobs": jsonResponse({ jobId: "j1" }),
			"GET /api/jobs/j1": jsonResponse({
				status: "failed",
				progress: 0,
				error: "File không có âm thanh",
			}),
		});
		const { job } = track(fetcher);
		await job.start({ inputFileId: "f1", criterion: "parts", value: 3 });
		await vi.runAllTimersAsync();
		expect(job.current).toMatchObject({
			status: "failed",
			error: "File không có âm thanh",
		});
	});

	it("rides out transient poll failures and gives up after several in a row", async () => {
		const { fetcher, calls } = fakeFetch({
			"POST /api/jobs": jsonResponse({ jobId: "j1" }),
			"GET /api/jobs/j1": [
				new TypeError("Failed to fetch"),
				jsonResponse({ status: "running", progress: 10 }),
				new TypeError("Failed to fetch"),
			],
		});
		const { job } = track(fetcher);
		await job.start({ inputFileId: "f1", criterion: "minutes", value: 1 });
		await vi.advanceTimersByTimeAsync(
			POLL_INTERVAL_MS * 2 * MAX_POLL_FAILS * 2,
		);
		const polls = calls.filter((call) => call.url === "/api/jobs/j1");
		// 1 failure, 1 success (counter resets), then MAX_POLL_FAILS failures.
		expect(polls).toHaveLength(2 + MAX_POLL_FAILS);
		expect(job.current.error).toMatch(/Không kết nối được máy chủ/);
		expect(job.current.status).toBe("running");
	});

	it("ignores a late poll after reset()", async () => {
		const { fetcher } = fakeFetch({
			"POST /api/jobs": jsonResponse({ jobId: "j1" }),
			"GET /api/jobs/j1": jsonResponse({ status: "running", progress: 50 }),
		});
		const { job } = track(fetcher);
		await job.start({ inputFileId: "f1", criterion: "minutes", value: 1 });
		job.reset();
		await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS * 3);
		expect(job.current).toMatchObject({
			jobId: null,
			status: null,
			progress: 0,
		});
	});
});
