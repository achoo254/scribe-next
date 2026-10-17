// @vitest-environment node
import { describe, expect, it } from "vitest";
import {
	type RecordedCall,
	fakeFetch,
	jsonResponse,
} from "../../test/fake-fetch";
import { CHUNK_RETRIES, RecordingChunkQueue } from "./recording-chunks";

const noWait = () => Promise.resolve();

function chunk(size: number): Blob {
	return new Blob([new Uint8Array(size)]);
}

function echoWritten(call: RecordedCall): Response {
	const offset = Number(
		new URL(call.url, "http://x").searchParams.get("offset"),
	);
	const size = call.body instanceof ArrayBuffer ? call.body.byteLength : 0;
	return jsonResponse({ written: offset + size });
}

const offsetsOf = (calls: RecordedCall[]) =>
	calls.map((call) =>
		Number(new URL(call.url, "http://x").searchParams.get("offset")),
	);

describe("RecordingChunkQueue", () => {
	it("sends chunks in order at strictly increasing offsets", async () => {
		const { fetcher, calls } = fakeFetch({
			"POST /api/recordings/chunk": echoWritten,
		});
		const queue = new RecordingChunkQueue("r1", fetcher, noWait);
		queue.enqueue(chunk(3));
		queue.enqueue(chunk(5));
		queue.enqueue(chunk(2));
		await queue.drain();

		expect(offsetsOf(calls)).toEqual([0, 3, 8]);
		expect(queue.offset).toBe(10);
		expect(queue.error).toBeNull();
		expect(calls[0]).toMatchObject({
			url: "/api/recordings/chunk?session=r1&offset=0",
			contentType: "application/octet-stream",
			credentials: "include",
		});
	});

	it("retries a failed chunk at the same offset, never sending it twice after success", async () => {
		const { fetcher, calls } = fakeFetch({
			"POST /api/recordings/chunk": [
				echoWritten,
				new TypeError("Failed to fetch"),
				echoWritten,
			],
		});
		const queue = new RecordingChunkQueue("r1", fetcher, noWait);
		queue.enqueue(chunk(4));
		queue.enqueue(chunk(4));
		queue.enqueue(chunk(4));
		await queue.drain();

		expect(offsetsOf(calls)).toEqual([0, 4, 4, 8]);
		expect(queue.offset).toBe(12);
	});

	it("stops sending after the last retry and keeps the error", async () => {
		const { fetcher, calls } = fakeFetch({
			"POST /api/recordings/chunk": [
				echoWritten,
				jsonResponse({ error: "Phiên ghi đã hết hạn" }, 410),
			],
		});
		const queue = new RecordingChunkQueue("r1", fetcher, noWait);
		queue.enqueue(chunk(2));
		queue.enqueue(chunk(2));
		queue.enqueue(chunk(2));
		await queue.drain();

		expect(calls).toHaveLength(1 + CHUNK_RETRIES);
		expect(queue.offset).toBe(2);
		expect(queue.error).toBe("Phiên ghi đã hết hạn");
	});

	it("stops at once when storage is full, keeping what was sent", async () => {
		const full =
			"Đã hết dung lượng lưu trữ (200 MB). Xoá bớt bản ghi hoặc tệp để ghi và tải lên tiếp.";
		const { fetcher, calls } = fakeFetch({
			"POST /api/recordings/chunk": [
				echoWritten,
				jsonResponse({ error: full }, 413),
			],
		});
		const refused: string[] = [];
		const queue = new RecordingChunkQueue("r1", fetcher, noWait, (message) =>
			refused.push(message),
		);
		queue.enqueue(chunk(2));
		queue.enqueue(chunk(2));
		queue.enqueue(chunk(2));
		await queue.drain();

		// No retry of the refused chunk, nothing sent after it.
		expect(calls).toHaveLength(2);
		expect(queue.offset).toBe(2);
		expect(queue.full).toBe(true);
		expect(queue.error).toBe(full);
		expect(refused).toEqual([full]);
	});

	it("skips empty chunks and anything queued after close()", async () => {
		const { fetcher, calls } = fakeFetch({
			"POST /api/recordings/chunk": echoWritten,
		});
		const queue = new RecordingChunkQueue("r1", fetcher, noWait);
		queue.enqueue(chunk(0));
		queue.close();
		queue.enqueue(chunk(3));
		await queue.drain();
		expect(calls).toHaveLength(0);
	});
});
