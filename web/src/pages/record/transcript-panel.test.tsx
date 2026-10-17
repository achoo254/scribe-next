import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { fakeFetch, jsonResponse } from "../../test/fake-fetch";
import { renderWithProviders } from "../../test/render-with-providers";
import { TranscriptPanel } from "./transcript-panel";

const auth = vi.hoisted(() => ({
	user: {
		email: "a@b.c",
		canTranscribe: true,
		transcription: {
			plan: "free",
			usedMinutes: 100,
			limitMinutes: 120,
			resetsAt: "2026-10-31T17:00:00Z",
			capReached: false,
		},
	},
	refresh: async () => undefined,
}));
vi.mock("../../auth/auth-context", () => ({ useAuth: () => auth }));

const sentence = { start: 0, end: 1, text: "Câu." };

function done(fileId: string) {
	return { fileId, status: "done", progress: 100, sentences: [sentence] };
}

function seg(fileId: string, edited: boolean, sentences: number) {
	return {
		fileId,
		index: 0,
		offsetS: 0,
		durationS: 60,
		hasVideo: false,
		status: "done",
		progress: 100,
		error: null,
		runId: "r",
		edited,
		// Edited segments here differ from the machine text in every sentence.
		editedSentences: edited ? sentences : 0,
		sentences: Array.from({ length: sentences }, () => sentence),
	};
}

afterEach(() => {
	vi.unstubAllGlobals();
});

describe("TranscriptPanel", () => {
	it("says how many edited segments and sentences a full re-run discards", async () => {
		const fake = fakeFetch({
			"GET /api/transcripts": jsonResponse({
				items: [done("a"), done("b"), done("c")],
			}),
			"GET /api/transcript-docs/s%3Aaaaaaaaa-1111": jsonResponse({
				key: "s:aaaaaaaa-1111",
				segments: [seg("a", true, 2), seg("b", false, 1), seg("c", true, 1)],
				missingIndexes: [],
			}),
		});
		vi.stubGlobal("fetch", fake.fetcher);
		renderWithProviders(
			<TranscriptPanel
				parts={[{ fileId: "a" }, { fileId: "b" }, { fileId: "c" }]}
				requested
				name="Phiên"
				session="aaaaaaaa-1111"
				onSeek={() => undefined}
				data-id="panel"
			/>,
		);
		await userEvent.click(
			await screen.findByRole("button", { name: "Chép lại" }),
		);
		expect(
			await screen.findByText("Bỏ phần đã sửa ở 2 đoạn (3 câu)?"),
		).toBeInTheDocument();
		expect(fake.calls.some((c) => c.method === "POST")).toBe(false);
	});

	it("sends every segment of an iPhone session when it is transcribed as a whole", async () => {
		const before = auth.user.transcription;
		auth.user.transcription = {
			...before,
			wholeSession: true,
		} as typeof before;
		const fake = fakeFetch({
			// Segment b came late: only a and c were transcribed.
			"GET /api/transcripts": jsonResponse({ items: [done("a"), done("c")] }),
			"POST /api/transcripts": jsonResponse({ queued: 3, skipped: 0 }),
		});
		vi.stubGlobal("fetch", fake.fetcher);
		renderWithProviders(
			<TranscriptPanel
				parts={[
					{ fileId: "a", durationS: 60 },
					{ fileId: "b", durationS: 60 },
					{ fileId: "c", durationS: 30 },
				]}
				requested
				name="Phiên"
				session="aaaaaaaa-1111"
				onSeek={() => undefined}
				data-id="panel"
			/>,
		);
		await userEvent.click(
			await screen.findByRole("button", { name: "Chép phần còn thiếu" }),
		);
		expect(fake.calls.find((c) => c.method === "POST")?.body).toEqual({
			fileIds: ["a", "b", "c"],
		});
		auth.user.transcription = before;
	});
});
