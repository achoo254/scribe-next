import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { TranscriptSummary } from "../../features/transcript-docs/transcript-docs-api";
import { fakeFetch, jsonResponse } from "../../test/fake-fetch";
import { renderWithProviders } from "../../test/render-with-providers";
import { TranscriptList } from "./transcript-list";

vi.mock("../../auth/auth-context", () => ({
	useAuth: () => ({ user: { email: "a@b.c", canTranscribe: true } }),
}));

const DOC: TranscriptSummary = {
	key: "s:aaaaaaaa-1111",
	title: "Phỏng vấn khách hàng",
	name: null,
	createdAt: "2026-10-16T08:00:00+07:00",
	durationS: 120,
	segments: 2,
	done: 2,
	failed: 0,
	pending: 0,
	untranscribed: 0,
	preview: "Câu đầu.",
};

afterEach(() => {
	vi.unstubAllGlobals();
});

const DOC_PATH = `/api/transcript-docs/${encodeURIComponent(DOC.key)}`;

function renderList(
	replies: Response[],
	extra: Parameters<typeof fakeFetch>[0] = {},
) {
	const fake = fakeFetch({ "GET /api/transcript-docs": replies, ...extra });
	vi.stubGlobal("fetch", fake.fetcher);
	const onOpen = vi.fn();
	renderWithProviders(<TranscriptList onOpen={onOpen} />);
	return { onOpen, calls: fake.calls };
}

async function openRowMenu(): Promise<void> {
	await userEvent.click(
		await screen.findByRole("button", {
			name: "Thao tác với “Phỏng vấn khách hàng”",
		}),
	);
}

describe("TranscriptList", () => {
	it("has a page heading and sends an empty list to the record page", async () => {
		renderList([jsonResponse({ items: [] })]);
		expect(
			screen.getByRole("heading", { level: 1, name: "Lời đã chép" }),
		).toBeInTheDocument();
		expect(
			await screen.findByRole("link", { name: "Mở trang Ghi âm" }),
		).toHaveAttribute("href", "/record");
	});

	it("opens a transcript from its row", async () => {
		const { onOpen } = renderList([jsonResponse({ items: [DOC] })]);
		await userEvent.click(
			await screen.findByRole("button", { name: /^Phỏng vấn khách hàng/ }),
		);
		expect(onOpen).toHaveBeenCalledWith(DOC.key);
		expect(
			screen.getByText("2:00 · 2 đoạn", { exact: false }),
		).toBeInTheDocument();
	});

	it("puts transcripts of the same day under one day heading", async () => {
		const SESSION: TranscriptSummary = {
			...DOC,
			key: "s:bbbbbbbb-2222",
			title: null,
			// Local times: the list groups by the device's day.
			createdAt: new Date(2026, 9, 16, 9, 30).toISOString(),
		};
		const OLDER: TranscriptSummary = {
			...DOC,
			key: "s:cccccccc-3333",
			title: "Họp tuần",
			createdAt: new Date(2026, 9, 14, 10, 0).toISOString(),
		};
		const named = {
			...DOC,
			createdAt: new Date(2026, 9, 16, 8, 0).toISOString(),
		};
		renderList([jsonResponse({ items: [SESSION, named, OLDER] })]);
		const days = await screen.findAllByRole("heading", { level: 2 });
		expect(days).toHaveLength(2);
		expect(days[0]).toHaveTextContent("16/10/2026");
		expect(days[1]).toHaveTextContent("14/10/2026");
		expect(screen.getByText("2 bản chép lời · 4:00")).toBeInTheDocument();
		// The day heading carries the date, so the default name drops it.
		expect(
			screen.getByRole("button", { name: /^Bản ghi trên iPhone \d\d:\d\d / }),
		).not.toHaveAccessibleName(/16\/10\/2026/);
	});

	it("offers a retry when the list fails to load", async () => {
		renderList([
			jsonResponse({ error: "Máy chủ bận" }, 503),
			jsonResponse({ items: [DOC] }),
		]);
		expect(
			await screen.findByText("Không tải được danh sách bản chép lời"),
		).toBeInTheDocument();
		await userEvent.click(screen.getByRole("button", { name: "Thử lại" }));
		expect(
			await screen.findByRole("button", { name: /^Phỏng vấn khách hàng/ }),
		).toBeInTheDocument();
	});

	it("offers download formats, rename and delete from the row menu", async () => {
		renderList([jsonResponse({ items: [DOC] })]);
		await openRowMenu();
		for (const format of ["TXT", "SRT", "VTT", "Markdown", "Word (.docx)"]) {
			expect(
				screen.getByRole("menuitem", { name: format }),
			).toBeInTheDocument();
		}
		expect(
			screen.getByRole("menuitem", { name: "Đổi tên" }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("menuitem", { name: "Xoá bản chép lời" }),
		).toBeInTheDocument();
	});

	it("hides download in the row menu until a segment is transcribed", async () => {
		renderList([jsonResponse({ items: [{ ...DOC, done: 0, pending: 2 }] })]);
		await openRowMenu();
		expect(screen.queryByRole("menuitem", { name: "TXT" })).toBeNull();
		expect(
			screen.getByRole("menuitem", { name: "Đổi tên" }),
		).toBeInTheDocument();
	});

	it("renames a transcript from the row menu", async () => {
		const { calls } = renderList(
			[
				jsonResponse({ items: [DOC] }),
				jsonResponse({ items: [{ ...DOC, title: "Họp ISO" }] }),
			],
			{ [`PATCH ${DOC_PATH}`]: jsonResponse({ title: "Họp ISO" }) },
		);
		await openRowMenu();
		await userEvent.click(screen.getByRole("menuitem", { name: "Đổi tên" }));
		const input = await screen.findByRole("textbox", {
			name: "Tên bản chép lời",
		});
		expect(input).toHaveFocus();
		await userEvent.clear(input);
		await userEvent.type(input, "Họp ISO");
		await userEvent.click(screen.getByRole("button", { name: "Lưu tên" }));
		expect(
			await screen.findByRole("button", { name: /^Họp ISO/ }),
		).toBeInTheDocument();
		expect(calls.find((c) => c.method === "PATCH")?.body).toEqual({
			title: "Họp ISO",
		});
	});

	it("deletes a transcript from the row menu after confirming", async () => {
		const { calls } = renderList(
			[jsonResponse({ items: [DOC] }), jsonResponse({ items: [] })],
			{ [`DELETE ${DOC_PATH}`]: new Response(null, { status: 204 }) },
		);
		await openRowMenu();
		await userEvent.click(
			screen.getByRole("menuitem", { name: "Xoá bản chép lời" }),
		);
		await userEvent.click(await screen.findByRole("button", { name: "Xoá" }));
		expect(
			await screen.findByText("Chưa có bản chép lời nào"),
		).toBeInTheDocument();
		expect(calls.some((c) => c.method === "DELETE")).toBe(true);
	});
});
