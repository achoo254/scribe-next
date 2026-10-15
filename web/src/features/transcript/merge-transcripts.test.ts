import { describe, expect, it } from "vitest";
import { mergeTranscripts } from "./merge-transcripts";
import type { Transcript } from "./transcript-api";

function transcript(
	fileId: string,
	status: Transcript["status"],
	extra: Partial<Transcript> = {},
): Transcript {
	return {
		fileId,
		status,
		progress: 0,
		sentences: [],
		...extra,
	};
}

describe("mergeTranscripts", () => {
	it("joins the files of a session with running times", () => {
		const merged = mergeTranscripts(
			[
				{ fileId: "a", durationS: 60 },
				{ fileId: "b", durationS: 60 },
			],
			[
				transcript("b", "done", {
					sentences: [{ start: 5, end: 8, text: "Câu hai." }],
				}),
				transcript("a", "done", {
					sentences: [{ start: 1, end: 3, text: "Câu một." }],
				}),
			],
		);
		expect(merged.lines).toEqual([
			{ fileId: "a", start: 1, at: 1, text: "Câu một." },
			{ fileId: "b", start: 5, at: 65, text: "Câu hai." },
		]);
		expect(merged).toMatchObject({
			done: 2,
			total: 2,
			pending: 0,
			progress: 100,
			untouched: false,
		});
	});

	it("keeps the offset of a file that is not transcribed yet", () => {
		const merged = mergeTranscripts(
			[
				{ fileId: "a", durationS: 60 },
				{ fileId: "b", durationS: 60 },
				{ fileId: "c", durationS: 30 },
			],
			[
				transcript("a", "running", { progress: 50 }),
				transcript("b", "failed", { error: "OpenAI: API key không hợp lệ" }),
				transcript("c", "done", {
					sentences: [{ start: 2, end: 4, text: "Cuối." }],
				}),
			],
		);
		expect(merged.lines).toEqual([
			{ fileId: "c", start: 2, at: 122, text: "Cuối." },
		]);
		expect(merged).toMatchObject({
			done: 1,
			pending: 1,
			failed: 1,
			progress: 50,
			error: "OpenAI: API key không hợp lệ",
		});
	});

	it("knows when nothing was asked yet", () => {
		const merged = mergeTranscripts([{ fileId: "a", durationS: 10 }], []);
		expect(merged).toMatchObject({ untouched: true, done: 0, progress: 0 });
	});
});
