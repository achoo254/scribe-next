import { describe, expect, it } from "vitest";
import {
	type RecordedCall,
	fakeFetch,
	jsonResponse,
} from "../../test/fake-fetch";
import { ChunkedUpload } from "./chunked-upload";

const CHUNK = 4;

function fileOf(size: number): File {
	return new File([new Uint8Array(size)], "meeting.mp3", {
		type: "audio/mpeg",
	});
}

/** Chunk replies that echo `offset + chunk size` like the backend. */
function echoWritten(call: RecordedCall): Response {
	const query = new URL(call.url, "http://x").searchParams;
	const offset = Number(query.get("offset"));
	const size = call.body instanceof Blob ? call.body.size : 0;
	return jsonResponse({ written: offset + size });
}

const offsetsOf = (calls: RecordedCall[]) =>
	calls
		.filter((call) => call.url.startsWith("/api/upload/chunk"))
		.map((call) =>
			Number(new URL(call.url, "http://x").searchParams.get("offset")),
		);

describe("ChunkedUpload", () => {
	it("inits, sends every chunk at the written offset and finalizes", async () => {
		const { fetcher, calls } = fakeFetch({
			"POST /api/upload/init": jsonResponse({ sessionId: "s1" }),
			"GET /api/upload/status": jsonResponse({ written: 0 }),
			"POST /api/upload/chunk": echoWritten,
			"POST /api/upload/finalize": jsonResponse({
				fileId: "f1",
				name: "meeting.mp3",
				durationS: 61,
			}),
		});
		const progress: number[] = [];
		const result = await new ChunkedUpload(fetcher, CHUNK).upload(
			fileOf(10),
			(percent) => progress.push(percent),
		);

		expect(result).toEqual({
			fileId: "f1",
			name: "meeting.mp3",
			durationS: 61,
		});
		expect(calls[0]).toMatchObject({
			url: "/api/upload/init",
			body: { name: "meeting.mp3", size: 10 },
		});
		expect(calls[1]?.url).toBe("/api/upload/status?session=s1");
		expect(offsetsOf(calls)).toEqual([0, 4, 8]);
		expect(calls.at(-1)).toMatchObject({
			url: "/api/upload/finalize",
			body: { session: "s1" },
		});
		expect(progress).toEqual([0, 40, 80, 100, 100]);
	});

	it("resumes the same file from the offset the backend reports", async () => {
		const { fetcher, calls } = fakeFetch({
			"POST /api/upload/init": jsonResponse({ sessionId: "s1" }),
			"GET /api/upload/status": [
				jsonResponse({ written: 0 }),
				jsonResponse({ written: 4 }),
			],
			"POST /api/upload/chunk": [
				echoWritten,
				new TypeError("Failed to fetch"),
				echoWritten,
			],
			"POST /api/upload/finalize": jsonResponse({
				fileId: "f1",
				name: "meeting.mp3",
				durationS: 1,
			}),
		});
		const upload = new ChunkedUpload(fetcher, CHUNK);
		const file = fileOf(10);

		await expect(upload.upload(file)).rejects.toMatchObject({ status: 0 });
		await upload.upload(file);

		expect(
			calls.filter((call) => call.url === "/api/upload/init"),
		).toHaveLength(1);
		// 0 ok, 4 lost, then status says 4 → 4, 8.
		expect(offsetsOf(calls)).toEqual([0, 4, 4, 8]);
	});

	it("starts a new session for another file", async () => {
		const { fetcher, calls } = fakeFetch({
			"POST /api/upload/init": [
				jsonResponse({ sessionId: "s1" }),
				jsonResponse({ sessionId: "s2" }),
			],
			"GET /api/upload/status": jsonResponse({ written: 0 }),
			"POST /api/upload/chunk": [new TypeError("Failed to fetch"), echoWritten],
			"POST /api/upload/finalize": jsonResponse({
				fileId: "f2",
				name: "b",
				durationS: 1,
			}),
		});
		const upload = new ChunkedUpload(fetcher, CHUNK);
		await expect(upload.upload(fileOf(3))).rejects.toBeTruthy();
		await upload.upload(fileOf(3));
		expect(
			calls.filter((call) => call.url === "/api/upload/init"),
		).toHaveLength(2);
		expect(calls.at(-1)?.body).toEqual({ session: "s2" });
	});

	it("asks for a history file only when made for transcription", async () => {
		const { fetcher, calls } = fakeFetch({
			"POST /api/upload/init": jsonResponse({ sessionId: "s1" }),
			"GET /api/upload/status": jsonResponse({ written: 3 }),
			"POST /api/upload/finalize": jsonResponse({
				fileId: "f1",
				name: "meeting.mp3",
				durationS: 1,
			}),
		});
		await new ChunkedUpload(fetcher, CHUNK, "transcribe").upload(fileOf(3));
		expect(calls[0]?.body).toEqual({
			name: "meeting.mp3",
			size: 3,
			purpose: "transcribe",
		});
	});

	it("reports the backend's message when init is refused", async () => {
		const { fetcher } = fakeFetch({
			"POST /api/upload/init": jsonResponse(
				{ error: "Vượt dung lượng 5 GB" },
				413,
			),
		});
		await expect(
			new ChunkedUpload(fetcher, CHUNK).upload(fileOf(3)),
		).rejects.toMatchObject({ message: "Vượt dung lượng 5 GB" });
	});
});
