import { describe, expect, it } from "vitest";
import {
	activeSentence,
	editedCounts,
	gapsBefore,
	needsReload,
	nextSegment,
	overallProgress,
	shouldPreload,
	summaryStatus,
	transcriptTitle,
	untranscribed,
} from "./reader-model";
import type {
	TranscriptSegment,
	TranscriptSummary,
} from "./transcript-docs-api";

function segment(
	fileId: string,
	index: number,
	extra: Partial<TranscriptSegment> = {},
): TranscriptSegment {
	return {
		fileId,
		index,
		offsetS: index * 60,
		durationS: 60,
		hasVideo: false,
		status: "done",
		progress: 100,
		error: null,
		runId: `run-${fileId}`,
		edited: false,
		editedSentences: 0,
		sentences: [
			{ start: 0, end: 20, text: "Một." },
			{ start: 20, end: 41.5, text: "Hai." },
			{ start: 41.5, end: 59, text: "Ba." },
		],
		...extra,
	};
}

describe("reader model", () => {
	it("finds the sentence being heard, including segment edges", () => {
		const s = segment("a", 0);
		expect(activeSentence(s, 0)).toBe(0);
		expect(activeSentence(s, 19.99)).toBe(0);
		expect(activeSentence(s, 20)).toBe(1);
		expect(activeSentence(s, 59.9)).toBe(2);
		expect(activeSentence({ ...s, sentences: [] }, 10)).toBe(-1);
		expect(activeSentence({ ...s, sentences: s.sentences.slice(1) }, 5)).toBe(
			-1,
		);
	});

	it("moves to the next segment and stops after the last", () => {
		const segments = [segment("a", 0), segment("b", 1), segment("c", 3)];
		expect(nextSegment(segments, "a")?.fileId).toBe("b");
		expect(nextSegment(segments, "b")?.fileId).toBe("c");
		expect(nextSegment(segments, "c")).toBeNull();
		expect(nextSegment(segments, "zzz")).toBeNull();
	});

	it("preloads only in the last seconds of a known duration", () => {
		expect(shouldPreload(49, 60)).toBe(false);
		expect(shouldPreload(50, 60)).toBe(true);
		expect(shouldPreload(5, 0)).toBe(false);
	});

	it("puts missing segment numbers before the segment that follows the gap", () => {
		const segments = [segment("a", 0), segment("c", 3), segment("d", 4)];
		const gaps = gapsBefore({ segments, missingIndexes: [1, 2] });
		expect([...gaps.entries()]).toEqual([["c", [1, 2]]]);
		const leading = gapsBefore({
			segments: [segment("b", 2)],
			missingIndexes: [0, 1],
		});
		expect(leading.get("b")).toEqual([0, 1]);
	});

	it("counts the edits a full re-transcription would discard", () => {
		const segments = [
			segment("a", 0, { edited: true, editedSentences: 1 }),
			segment("b", 1),
			segment("c", 2, { edited: true, editedSentences: 2 }),
		];
		// Only the sentences that really differ, not every sentence of the segment.
		expect(editedCounts(segments)).toEqual({ segments: 2, sentences: 3 });
		expect(editedCounts([segment("a", 0)])).toEqual({
			segments: 0,
			sentences: 0,
		});
	});

	it("lists segments still to send", () => {
		const segments = [
			segment("a", 0),
			segment("b", 1, { status: "none" }),
			segment("c", 2, { status: "failed" }),
			segment("d", 3, { status: "running" }),
		];
		expect(untranscribed(segments).map((s) => s.fileId)).toEqual(["b", "c"]);
	});

	it("reloads only when a status or run changed, not for progress", () => {
		const segments = [
			segment("a", 0),
			segment("b", 1, { status: "running", progress: 10 }),
		];
		const same = segments.map((s) => ({
			fileId: s.fileId,
			status: s.status,
			progress: s.progress + 5,
			runId: s.runId,
		}));
		expect(needsReload(segments, same)).toBe(false);
		expect(
			needsReload(segments, [
				same[0] ?? never(),
				{ ...(same[1] ?? never()), status: "done" },
			]),
		).toBe(true);
		expect(
			needsReload(segments, [
				{ ...(same[0] ?? never()), runId: "new" },
				same[1] ?? never(),
			]),
		).toBe(true);
		expect(needsReload(segments, same.slice(0, 1))).toBe(true);
	});

	it("averages progress over the segments that were sent", () => {
		const segments = [
			segment("a", 0),
			segment("b", 1, { status: "running", progress: 0 }),
			segment("c", 2, { status: "none", progress: 0 }),
		];
		expect(overallProgress(segments)).toBe(50);
		expect(
			overallProgress(segments, [
				{ fileId: "b", status: "running", progress: 50, runId: "x" },
			]),
		).toBe(75);
		expect(overallProgress([segment("c", 0, { status: "none" })])).toBe(0);
	});
});

describe("list labels", () => {
	const start = new Date(2026, 9, 16, 9, 41).toISOString();
	const summary: TranscriptSummary = {
		key: "s:aaaaaaaa-1111",
		title: null,
		name: null,
		createdAt: start,
		durationS: 2880,
		segments: 49,
		done: 49,
		failed: 0,
		pending: 0,
		untranscribed: 0,
		preview: "Xin chào.",
	};

	it("prefers the user's title, then a dated session name or the file name", () => {
		expect(transcriptTitle(summary)).toBe(
			"Bản ghi trên iPhone 09:41 16/10/2026",
		);
		expect(transcriptTitle({ ...summary, title: "Họp tuần" })).toBe("Họp tuần");
		expect(
			transcriptTitle({
				...summary,
				key: "f:65f0a1b2c3d4e5f60718293a",
				name: "Bản ghi 10:00:00",
			}),
		).toBe("Bản ghi 10:00:00");
	});

	it("shows running, failed, unsent, then done", () => {
		expect(summaryStatus({ ...summary, done: 10, pending: 39 })).toEqual({
			label: "Đang chép 10/49",
			tone: "info",
		});
		expect(summaryStatus({ ...summary, done: 47, failed: 2 }).label).toBe(
			"Lỗi 2 đoạn",
		);
		expect(
			summaryStatus({ ...summary, done: 48, untranscribed: 1 }).label,
		).toBe("Còn 1 đoạn chưa chép");
		expect(summaryStatus(summary).label).toBe("Đã chép xong");
	});
});

function never(): never {
	throw new Error("missing fixture");
}
