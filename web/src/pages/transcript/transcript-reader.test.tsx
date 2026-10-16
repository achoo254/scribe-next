import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
	Bookmark,
	TranscriptDetail,
	TranscriptMinutes,
	TranscriptSegment,
} from "../../features/transcript-docs/transcript-docs-api";
import { fakeFetch, jsonResponse } from "../../test/fake-fetch";
import { renderWithProviders } from "../../test/render-with-providers";
import { TranscriptReader } from "./transcript-reader";

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
	const { queryClient, router } = renderWithProviders(
		<TranscriptReader
			transcriptKey={KEY}
			canTranscribe
			onBack={() => undefined}
		/>,
		{ routes: [{ path: "/split", element: <p>trang cắt file</p> }] },
	);
	return { ...fake, queryClient, router };
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

	it("opened at a second (a search result), shows and plays it", async () => {
		const fake = fakeFetch({ [`GET ${PATH}`]: jsonResponse(DETAIL) });
		vi.stubGlobal("fetch", fake.fetcher);
		renderWithProviders(
			<TranscriptReader
				transcriptKey={KEY}
				startAt={61}
				canTranscribe
				onBack={() => undefined}
			/>,
		);
		// 1:01 of the session is 0:01 into the third segment.
		expect(await screen.findByText("Đang ở đoạn 3")).toBeInTheDocument();
		await waitFor(() =>
			expect(
				document.querySelector('[data-id="reader-segment-2-line-0"]'),
			).toHaveFocus(),
		);
		// "Hỏi Scribe" asks about this transcript only.
		expect(screen.getByText(/Hỏi về bản ghi này/)).toBeInTheDocument();
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

	it("asks before a long transcription and shows the minutes left", async () => {
		const long: TranscriptDetail = {
			...DETAIL,
			segments: DETAIL.segments.map((s) =>
				s.fileId === "d" ? { ...s, durationS: 30 * 60 } : s,
			),
		};
		const fake = renderReader({ [`GET ${PATH}`]: jsonResponse(long) });
		await userEvent.click(
			await screen.findByRole("button", { name: "Chép 1 đoạn còn lại" }),
		);
		const dialog = await screen.findByRole("dialog");
		expect(dialog).toHaveTextContent("khoảng 30 phút");
		expect(dialog).toHaveTextContent("20 phút còn lại");
		// 30 phút > 20 phút còn lại: không cho gửi.
		expect(
			within(dialog).getByRole("button", { name: "Chép lời" }),
		).toBeDisabled();
		expect(fake.calls.some((c) => c.method === "POST")).toBe(false);
	});

	it("asks before leaving the page with an unsaved edit", async () => {
		const { router } = renderReader();
		const header = await screen.findByText(/Đoạn 1 ·/);
		const section = header.closest("section") as HTMLElement;
		await userEvent.click(within(section).getByRole("button", { name: "Sửa" }));

		await act(() => router.navigate("/split"));
		const dialog = await screen.findByRole("dialog", { name: "Rời trang" });
		expect(router.state.location.pathname).toBe("/");
		// Stay: the draft is still open.
		await userEvent.click(within(dialog).getByRole("button", { name: "Huỷ" }));
		await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
		expect(screen.getByRole("textbox", { name: "Câu 1" })).toBeInTheDocument();
		expect(router.state.location.pathname).toBe("/");

		await act(() => router.navigate("/split"));
		await userEvent.click(
			await screen.findByRole("button", { name: "Vẫn rời trang" }),
		);
		expect(await screen.findByText("trang cắt file")).toBeInTheDocument();
		expect(router.state.location.pathname).toBe("/split");
	});

	it("leaves without asking when nothing is being edited", async () => {
		const { router } = renderReader();
		await screen.findByText(/Đoạn 1 ·/);
		await act(() => router.navigate("/split"));
		expect(await screen.findByText("trang cắt file")).toBeInTheDocument();
		expect(screen.queryByRole("dialog")).toBeNull();
	});
});

describe("TranscriptReader header", () => {
	it("deletes from the ⋮ menu, says so and goes back without a history entry", async () => {
		const fake = fakeFetch({
			[`GET ${PATH}`]: jsonResponse(DETAIL),
			[`DELETE ${PATH}`]: new Response(null, { status: 204 }),
		});
		vi.stubGlobal("fetch", fake.fetcher);
		const onBack = vi.fn();
		renderWithProviders(
			<TranscriptReader transcriptKey={KEY} canTranscribe onBack={onBack} />,
		);
		const title = "Bản ghi trên iPhone 09:41 16/10/2026";
		expect(
			await screen.findByRole("heading", { level: 1, name: title }),
		).toHaveAttribute("aria-current", "page");
		expect(
			screen.getByRole("navigation", { name: "Đường dẫn" }),
		).toHaveTextContent(`Lời đã chép/${title}`);

		screen.getByRole("button", { name: "Thao tác với bản chép lời" }).focus();
		await userEvent.keyboard("{Enter}");
		expect(
			await screen.findByRole("menuitem", { name: "Tải .txt" }),
		).toBeInTheDocument();
		await userEvent.click(
			screen.getByRole("menuitem", { name: "Xoá bản chép lời" }),
		);
		const dialog = await screen.findByRole("dialog", {
			name: "Xoá bản chép lời",
		});
		await userEvent.click(within(dialog).getByRole("button", { name: "Xoá" }));

		expect(
			await screen.findByText(`Đã xoá bản chép lời “${title}”`),
		).toBeVisible();
		await waitFor(() => expect(onBack).toHaveBeenCalledWith({ replace: true }));
		expect(fake.calls.some((c) => c.method === "DELETE")).toBe(true);
	});
});

const MINUTES: TranscriptMinutes = {
	model: "gpt-4.1-mini",
	createdAt: "2026-10-16T03:00:00Z",
	summary: "Nhóm thống nhất lịch ra mắt.",
	decisions: [{ text: "Ra mắt thứ Hai.", at: 90 }],
	actions: [{ text: "Gửi báo giá", owner: "Lan", due: "thứ Sáu", at: 10 }],
	stale: false,
};

function renderDoc(
	extra: Partial<TranscriptDetail>,
	routes: Parameters<typeof fakeFetch>[0] = {},
) {
	return renderReader({
		[`GET ${PATH}`]: jsonResponse({ ...DETAIL, ...extra }),
		...routes,
	});
}

describe("TranscriptReader minutes", () => {
	it("makes the minutes, then a time plays and shows that sentence", async () => {
		const fake = renderReader({
			[`POST ${PATH}/summary`]: jsonResponse(MINUTES),
			"GET /api/chat/link": jsonResponse({ linked: false }),
		});
		await userEvent.click(
			await screen.findByRole("button", { name: "Tóm tắt" }),
		);
		expect(
			await screen.findByText("Nhóm thống nhất lịch ra mắt."),
		).toBeInTheDocument();
		expect(screen.getByText("Người làm: Lan · Hạn: thứ Sáu")).toBeVisible();
		expect(fake.calls.some((c) => c.method === "POST")).toBe(true);

		// 1:30 of the session is 0:30 into segment 3 ("c"), its 2nd sentence.
		await userEvent.click(screen.getByRole("button", { name: "Nghe từ 1:30" }));
		expect(screen.getByText("Đang ở đoạn 3")).toBeInTheDocument();
		expect(screen.getByRole("button", { name: /c hai\./ })).toHaveFocus();
	});

	it("reads the doc again when the reply is lost, to show minutes saved meanwhile", async () => {
		const fake = renderReader({
			[`GET ${PATH}`]: [
				jsonResponse(DETAIL),
				jsonResponse({ ...DETAIL, summary: MINUTES }),
			],
			[`POST ${PATH}/summary`]: jsonResponse(
				{ error: "A timeout occurred" },
				524,
			),
			"GET /api/chat/link": jsonResponse({ linked: false }),
		});
		await userEvent.click(
			await screen.findByRole("button", { name: "Tóm tắt" }),
		);
		expect(
			await screen.findByText("Nhóm thống nhất lịch ra mắt."),
		).toBeInTheDocument();
		const docReads = fake.calls.filter(
			(c) => c.method === "GET" && c.url.endsWith(PATH),
		);
		expect(docReads).toHaveLength(2);
	});

	it("says when the daily allowance opens again", async () => {
		const fake = renderReader({
			[`POST ${PATH}/summary`]: jsonResponse(
				{
					error: "Hôm nay đã dùng 10/10 lượt tóm tắt. Mở lại lúc 0 giờ.",
					code: "SUMMARY_DAILY_LIMIT",
					usedMinutes: 10,
					limitMinutes: 10,
					retryAt: "2026-10-16T17:00:00+00:00",
				},
				403,
			),
		});
		await userEvent.click(
			await screen.findByRole("button", { name: "Tóm tắt" }),
		);
		expect(
			await screen.findByText(
				"Hôm nay đã dùng 10/10 lượt tóm tắt. Mở lại lúc 00:00 17/10.",
			),
		).toBeInTheDocument();
		// A refusal is an answer: nothing was saved, the doc is not read again.
		expect(
			fake.calls.filter((c) => c.method === "GET" && c.url.endsWith(PATH)),
		).toHaveLength(1);
	});

	it("says when the account's monthly summary allowance opens again", async () => {
		renderReader({
			[`POST ${PATH}/summary`]: jsonResponse(
				{
					error: "Hết hạn mức tháng.",
					code: "SUMMARY_MONTHLY_LIMIT",
					usedMinutes: 60,
					limitMinutes: 60,
					retryAt: "2026-10-31T17:00:00+00:00",
				},
				403,
			),
		});
		await userEvent.click(
			await screen.findByRole("button", { name: "Tóm tắt" }),
		);
		expect(
			await screen.findByText(
				"Tháng này tài khoản đã dùng hết hạn mức tóm tắt và hỏi đáp (60/60 phút quy đổi). Mở lại lúc 00:00 01/11.",
			),
		).toBeInTheDocument();
	});

	it("offers no sending when the server takes no chat links", async () => {
		renderDoc(
			{ summary: MINUTES },
			{
				"GET /api/chat/link": jsonResponse({
					linked: false,
					linkedAt: null,
					enabled: false,
				}),
			},
		);
		expect(
			await screen.findByText("Nhóm thống nhất lịch ra mắt."),
		).toBeInTheDocument();
		await waitFor(() =>
			expect(
				screen.queryByRole("button", { name: "Liên kết chat" }),
			).toBeNull(),
		);
		expect(screen.queryByRole("button", { name: "Gửi sang chat" })).toBeNull();
	});

	it("waits for a transcribed segment before offering a summary", async () => {
		renderDoc({
			segments: [
				segment("d", 0, 0, { status: "none", runId: null, sentences: [] }),
			],
			missingIndexes: [],
		});
		expect(
			await screen.findByText("Chép lời xong ít nhất một đoạn để tóm tắt."),
		).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Tóm tắt" })).toBeDisabled();
	});

	it("marks stale minutes and sends them to the linked chat", async () => {
		const fake = renderDoc(
			{ summary: { ...MINUTES, stale: true } },
			{
				"GET /api/chat/link": jsonResponse({ linked: true }),
				[`POST ${PATH}/share`]: jsonResponse({ ok: true }),
			},
		);
		expect(await screen.findByText("Đã cũ")).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: "Tóm tắt lại" }),
		).toBeInTheDocument();
		await userEvent.click(
			await screen.findByRole("button", { name: "Gửi sang chat" }),
		);
		expect(
			await screen.findByText("Đã gửi biên bản sang chat"),
		).toBeInTheDocument();
		expect(fake.calls.some((c) => c.url.endsWith("/share"))).toBe(true);
	});

	it("opens the linking steps when the bot was blocked and the link dropped", async () => {
		renderDoc(
			{ summary: MINUTES },
			{
				"GET /api/chat/link": [
					jsonResponse({ linked: true }),
					jsonResponse({ linked: false }),
				],
				[`POST ${PATH}/share`]: jsonResponse(
					{ error: "Bot Scribe đã bị chặn.", code: "CHAT_BLOCKED" },
					409,
				),
				"POST /api/chat/link": jsonResponse({
					url: "https://chat.hoandat.com/b/bot_scribe_bot?start=t",
					expiresAt: "2026-10-16T03:10:00Z",
				}),
			},
		);
		await userEvent.click(
			await screen.findByRole("button", { name: "Gửi sang chat" }),
		);
		expect(
			await screen.findByRole("dialog", { name: "Liên kết chat.hoandat.com" }),
		).toBeInTheDocument();
		expect(screen.getByText("Bot Scribe đã bị chặn.")).toBeInTheDocument();
		// Under the dialog (hidden from the accessibility tree meanwhile).
		expect(
			screen.getByRole("button", { name: "Liên kết chat", hidden: true }),
		).toBeInTheDocument();
	});
});

describe("TranscriptReader bookmarks", () => {
	const BOOKMARKS: Bookmark[] = [
		{ id: "b1", at: 95, label: "Chốt giá" },
		{ id: "b2", at: 5, label: null },
	];

	/** The "Dấu mốc" card, once the transcript is loaded. */
	async function bookmarkList(): Promise<HTMLElement> {
		const heading = await screen.findByRole("heading", { name: /^Dấu mốc/ });
		const card = heading.closest<HTMLElement>('[data-id="reader-bookmarks"]');
		if (!card) throw new Error("no bookmark list");
		return card;
	}

	it("lists them, shows them beside their sentence, and deletes one", async () => {
		const fake = renderReader({
			[`GET ${PATH}`]: [
				jsonResponse({ ...DETAIL, bookmarks: BOOKMARKS }),
				jsonResponse({ ...DETAIL, bookmarks: BOOKMARKS.slice(1) }),
			],
			[`DELETE ${PATH}/bookmarks/b1`]: jsonResponse({ ok: true }),
		});
		const list = await bookmarkList();
		expect(within(list).getByText("Chốt giá")).toBeInTheDocument();
		expect(within(list).getByText("Dấu mốc 2")).toBeInTheDocument();
		expect(screen.getByText("Dấu mốc 1:35 · Chốt giá")).toBeInTheDocument();
		expect(screen.getByText("Dấu mốc 0:05")).toBeInTheDocument();

		await userEvent.click(
			within(list).getByRole("button", { name: "Xoá dấu mốc 1:35" }),
		);
		expect(await screen.findByText("Đã xoá dấu mốc")).toBeInTheDocument();
		await waitFor(() =>
			expect(screen.queryByText("Dấu mốc 1:35 · Chốt giá")).toBeNull(),
		);
		expect(fake.calls.some((c) => c.method === "DELETE")).toBe(true);
	});

	it("jumps to a bookmark's time", async () => {
		renderDoc({ bookmarks: BOOKMARKS });
		const list = await bookmarkList();
		await userEvent.click(
			within(list).getByRole("button", { name: "Nghe từ 1:35" }),
		);
		expect(screen.getByText("Đang ở đoạn 3")).toBeInTheDocument();
	});
});
