import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { CurrentUser, OwnKeys } from "../../auth/auth-api";
import { fakeFetch, jsonResponse } from "../../test/fake-fetch";
import { ALL_OWN_KEYS, NO_OWN_KEYS } from "../../test/own-keys";
import { renderWithProviders } from "../../test/render-with-providers";
import { TranscriptPanel } from "./transcript-panel";

const auth = vi.hoisted(() => ({
	user: null as CurrentUser | null,
	refresh: async () => undefined,
}));
vi.mock("../../auth/auth-context", () => ({ useAuth: () => auth }));

/** Signed in with these own keys (OpenAI transcribes file by file). */
function withKeys(ownKeys: OwnKeys): void {
	auth.user = { email: "a@b.c", canTranscribe: true, ownKeys };
}

const OPENAI_KEYS: OwnKeys = { ...ALL_OWN_KEYS, transcribe: "openai" };

/** An iPhone session of three segments where b came late (a, c done). */
function renderSession(routes: Parameters<typeof fakeFetch>[0]) {
	const fake = fakeFetch({
		"GET /api/transcripts": jsonResponse({ items: [done("a"), done("c")] }),
		"GET /api/transcript-docs/s%3Aaaaaaaaa-1111": jsonResponse({
			key: "s:aaaaaaaa-1111",
			segments: [seg("a", false, 1), seg("c", false, 1)],
			missingIndexes: [],
			speakers: { names: { s0: "Người 1", s1: "Người 2" } },
		}),
		...routes,
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
	return fake;
}

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

beforeEach(() => {
	withKeys(OPENAI_KEYS);
});

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

	it("sends every segment of an iPhone session on an ElevenLabs key", async () => {
		withKeys(ALL_OWN_KEYS);
		const fake = renderSession({
			"POST /api/transcripts": jsonResponse({ queued: 3, skipped: 0 }),
		});
		await userEvent.click(
			await screen.findByRole("button", { name: "Chép phần còn thiếu" }),
		);
		await waitFor(() =>
			expect(fake.calls.find((c) => c.method === "POST")?.body).toEqual({
				fileIds: ["a", "b", "c"],
			}),
		);
	});

	it("sends only the missing segment on a per-file (OpenAI) key", async () => {
		const fake = renderSession({
			"POST /api/transcripts": jsonResponse({ queued: 1, skipped: 0 }),
		});
		await userEvent.click(
			await screen.findByRole("button", { name: "Chép phần còn thiếu" }),
		);
		await waitFor(() =>
			expect(fake.calls.find((c) => c.method === "POST")?.body).toEqual({
				fileIds: ["b"],
			}),
		);
	});

	it("explains that transcribing needs the user's key, without sending", async () => {
		withKeys(NO_OWN_KEYS);
		const fake = renderSession({
			"GET /api/me/keys": jsonResponse({ available: true, keys: [] }),
		});
		await userEvent.click(
			await screen.findByRole("button", { name: "Chép phần còn thiếu" }),
		);
		const dialog = await screen.findByRole("dialog", {
			name: "Cần API key của bạn",
		});
		expect(dialog).toHaveTextContent(
			"Chép lời chạy bằng API key của bạn. Thêm key ElevenLabs, OpenAI hoặc Google Gemini trong Key riêng.",
		);
		await userEvent.click(screen.getByRole("button", { name: "Mở Key riêng" }));
		expect(
			await screen.findByRole("dialog", { name: "Key riêng" }),
		).toBeVisible();
		expect(fake.calls.some((c) => c.method === "POST")).toBe(false);
	});

	it("shows the server's NO_USER_KEY refusal with a way to Key riêng", async () => {
		renderSession({
			"POST /api/transcripts": jsonResponse(
				{
					error: "Key chép lời không còn hợp lệ. Thêm key trong Key riêng.",
					code: "NO_USER_KEY",
				},
				403,
			),
		});
		await userEvent.click(
			await screen.findByRole("button", { name: "Chép phần còn thiếu" }),
		);
		const dialog = await screen.findByRole("dialog", {
			name: "Cần API key của bạn",
		});
		expect(dialog).toHaveTextContent(
			"Key chép lời không còn hợp lệ. Thêm key trong Key riêng.",
		);
		expect(screen.getByRole("button", { name: "Mở Key riêng" })).toBeVisible();
	});

	it("asks before a whole-session run resets renamed speakers or drops edits", async () => {
		withKeys(ALL_OWN_KEYS);
		const fake = fakeFetch({
			"GET /api/transcripts": jsonResponse({ items: [done("a"), done("c")] }),
			"GET /api/transcript-docs/s%3Aaaaaaaaa-1111": jsonResponse({
				key: "s:aaaaaaaa-1111",
				segments: [seg("a", false, 1), seg("c", false, 1)],
				missingIndexes: [],
				speakers: { names: { s0: "Lan", s1: "Minh" } },
			}),
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
		expect(
			await screen.findByText("Đặt lại tên 2 người nói đã đổi?"),
		).toBeInTheDocument();
		expect(screen.getByText(/tên đã đặt về Người 1/)).toBeInTheDocument();
		expect(fake.calls.some((c) => c.method === "POST")).toBe(false);
	});
});
