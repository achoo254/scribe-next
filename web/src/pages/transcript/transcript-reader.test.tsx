import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
	TranscriptDetail,
	TranscriptSegment,
} from "../../features/transcript-docs/transcript-docs-api";
import { fakeFetch, jsonResponse } from "../../test/fake-fetch";
import { renderWithProviders } from "../../test/render-with-providers";
import { TranscriptReader } from "./transcript-reader";

const KEY = "s:aaaaaaaa-1111";
const PATH = "/api/transcript-docs/s%3Aaaaaaaaa-1111";

function segment(
	fileId: string,
	index: number,
	offsetS: number,
	extra: Partial<TranscriptSegment> = {},
): TranscriptSegment {
	return {
		fileId,
		index,
		offsetS,
		durationS: 60,
		hasVideo: false,
		status: "done",
		progress: 100,
		error: null,
		runId: `run-${fileId}`,
		edited: false,
		editedSentences: 0,
		sentences: [
			{ start: 0, end: 30, text: `${fileId} một.` },
			{ start: 30, end: 59, text: `${fileId} hai.` },
		],
		...extra,
	};
}

// Segment 2 (index 1) never reached the server; segment 4 is not transcribed.
const DETAIL: TranscriptDetail = {
	key: KEY,
	title: null,
	name: null,
	createdAt: new Date(2026, 9, 16, 9, 41).toISOString(),
	segments: [
		segment("a", 0, 0),
		segment("c", 2, 60, { edited: true, editedSentences: 1 }),
		segment("d", 3, 120, { status: "none", runId: null, sentences: [] }),
	],
	missingIndexes: [1],
};

function renderReader(routes: Parameters<typeof fakeFetch>[0] = {}) {
	const fake = fakeFetch({ [`GET ${PATH}`]: jsonResponse(DETAIL), ...routes });
	vi.stubGlobal("fetch", fake.fetcher);
	const { queryClient } = renderWithProviders(
		<TranscriptReader
			transcriptKey={KEY}
			active
			canTranscribe
			onBack={() => undefined}
		/>,
	);
	return { ...fake, queryClient };
}

afterEach(() => {
	vi.unstubAllGlobals();
});

describe("TranscriptReader", () => {
	it("reads every segment in order with session-wide clocks and gaps", async () => {
		renderReader();
		expect(
			await screen.findByRole("heading", {
				name: "Bản ghi trên iPhone 09:41 16/10/2026",
			}),
		).toBeInTheDocument();
		const second = screen.getByRole("button", { name: /c hai\./ });
		expect(second).toHaveTextContent("1:30");
		expect(screen.getByText("Thiếu đoạn 2")).toBeInTheDocument();
		expect(screen.getByText("Đoạn này chưa chép lời.")).toBeInTheDocument();
		expect(screen.getByText(/Còn 1 đoạn chưa chép/)).toBeInTheDocument();
	});

	it("plays the tapped sentence's segment", async () => {
		renderReader();
		expect(await screen.findByText("Đang ở đoạn 1")).toBeInTheDocument();
		await userEvent.click(screen.getByRole("button", { name: /c một\./ }));
		expect(screen.getByText("Đang ở đoạn 3")).toBeInTheDocument();
	});

	it("saves one segment's edits against the run they replace", async () => {
		const fake = renderReader({
			[`PUT ${PATH}/segments/a`]: jsonResponse({ ok: true }),
		});
		const header = await screen.findByText(/Đoạn 1 ·/);
		const section = header.closest("section") as HTMLElement;
		await userEvent.click(within(section).getByRole("button", { name: "Sửa" }));
		const field = screen.getByRole("textbox", { name: "Câu 1" });
		await userEvent.clear(field);
		await userEvent.type(field, "Một đã sửa.");
		await userEvent.click(screen.getByRole("button", { name: "Lưu" }));
		await waitFor(() =>
			expect(fake.calls.find((c) => c.method === "PUT")?.body).toEqual({
				runId: "run-a",
				texts: ["Một đã sửa.", "a hai."],
			}),
		);
	});

	it("shows the conflict when the segment was transcribed again", async () => {
		renderReader({
			[`PUT ${PATH}/segments/a`]: jsonResponse(
				{ error: "đoạn này vừa được chép lại, tải lại để sửa tiếp" },
				409,
			),
		});
		const header = await screen.findByText(/Đoạn 1 ·/);
		const section = header.closest("section") as HTMLElement;
		await userEvent.click(within(section).getByRole("button", { name: "Sửa" }));
		await userEvent.click(screen.getByRole("button", { name: "Lưu" }));
		expect(
			await screen.findByText(
				"đoạn này vừa được chép lại, tải lại để sửa tiếp",
			),
		).toBeInTheDocument();
	});

	it("asks before transcribing an edited segment again", async () => {
		renderReader();
		const header = await screen.findByText(/Đoạn 3 ·/);
		const section = header.closest("section") as HTMLElement;
		await userEvent.click(
			within(section).getByRole("button", { name: "Chép lại đoạn này" }),
		);
		expect(
			await screen.findByText("Bỏ 1 câu đã sửa ở đoạn 3?"),
		).toBeInTheDocument();
	});

	it("never saves a draft onto a run newer than the one it was made from", async () => {
		const rerun: TranscriptDetail = {
			...DETAIL,
			segments: DETAIL.segments.map((s) =>
				s.fileId === "a" ? { ...s, runId: "run-a2" } : s,
			),
		};
		const fake = renderReader({
			[`GET ${PATH}`]: [jsonResponse(DETAIL), jsonResponse(rerun)],
			[`PUT ${PATH}/segments/a`]: jsonResponse({ ok: true }),
		});
		const header = await screen.findByText(/Đoạn 1 ·/);
		const section = header.closest("section") as HTMLElement;
		await userEvent.click(within(section).getByRole("button", { name: "Sửa" }));
		// Segment 1 is transcribed again elsewhere; the reader reloads.
		await fake.queryClient.invalidateQueries();
		expect(
			await screen.findByText(
				/vừa được chép lại nên bản đang sửa không còn khớp/,
			),
		).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Lưu" })).toBeDisabled();
		expect(fake.calls.some((c) => c.method === "PUT")).toBe(false);
	});
});
