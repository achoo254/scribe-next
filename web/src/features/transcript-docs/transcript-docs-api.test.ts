import { describe, expect, it } from "vitest";
import { ApiError } from "../../lib/api";
import { fakeFetch, jsonResponse } from "../../test/fake-fetch";
import {
	deleteTranscript,
	fetchTranscriptDoc,
	fetchTranscriptStatus,
	listTranscriptDocs,
	renameTranscript,
	saveSegmentTexts,
	transcriptKey,
} from "./transcript-docs-api";

const KEY = "s:6ad09d56-a5e5";
const PATH = "/api/transcript-docs/s%3A6ad09d56-a5e5";

describe("transcript-docs api", () => {
	it("builds the key the backend groups by", () => {
		expect(transcriptKey({ session: "6ad09d56-a5e5" })).toBe(KEY);
		expect(transcriptKey({ fileId: "65f0a1b2c3d4e5f60718293a" })).toBe(
			"f:65f0a1b2c3d4e5f60718293a",
		);
	});

	it("lists, reads and polls with the encoded key", async () => {
		const { fetcher, calls } = fakeFetch({
			"GET /api/transcript-docs": jsonResponse({ items: [{ key: KEY }] }),
			[`GET ${PATH}`]: jsonResponse({ key: KEY, segments: [] }),
			[`GET ${PATH}/status`]: jsonResponse({ items: [{ fileId: "a" }] }),
		});
		expect(await listTranscriptDocs(fetcher)).toEqual([{ key: KEY }]);
		expect((await fetchTranscriptDoc(KEY, fetcher)).key).toBe(KEY);
		expect(await fetchTranscriptStatus(KEY, fetcher)).toEqual([
			{ fileId: "a" },
		]);
		expect(calls.every((call) => call.credentials === "include")).toBe(true);
	});

	it("sends an empty title as null (back to the default name)", async () => {
		const { fetcher, calls } = fakeFetch({
			[`PATCH ${PATH}`]: jsonResponse({ title: null }),
		});
		expect(await renameTranscript(KEY, "   ", fetcher)).toBeNull();
		expect(calls[0]?.body).toEqual({ title: null });
	});

	it("saves one segment's texts with the run they replace", async () => {
		const { fetcher, calls } = fakeFetch({
			[`PUT ${PATH}/segments/f1`]: jsonResponse({ ok: true }),
		});
		await saveSegmentTexts(KEY, "f1", "run-1", ["Một.", "Hai."], fetcher);
		expect(calls[0]?.body).toEqual({ runId: "run-1", texts: ["Một.", "Hai."] });
	});

	it("surfaces the backend's conflict message", async () => {
		const { fetcher } = fakeFetch({
			[`PUT ${PATH}/segments/f1`]: jsonResponse(
				{ error: "đoạn này vừa được chép lại, tải lại để sửa tiếp" },
				409,
			),
		});
		const error = await saveSegmentTexts(KEY, "f1", "old", [], fetcher).catch(
			(cause: unknown) => cause,
		);
		expect(error).toBeInstanceOf(ApiError);
		expect((error as ApiError).status).toBe(409);
	});

	it("deletes the transcript", async () => {
		const { fetcher, calls } = fakeFetch({
			[`DELETE ${PATH}`]: jsonResponse({ ok: true }),
		});
		await deleteTranscript(KEY, fetcher);
		expect(calls[0]?.method).toBe("DELETE");
	});
});
