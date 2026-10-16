import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { SearchReply } from "../../features/search/search-api";
import { fakeFetch, jsonResponse } from "../../test/fake-fetch";
import { renderWithProviders } from "../../test/render-with-providers";
import { SearchPage } from "./search-page";

const REPLY: SearchReply = {
	results: [
		{
			key: "s:aaaaaaaa-1111",
			title: null,
			createdAt: new Date(2026, 9, 16, 9, 41).toISOString(),
			fileId: "a",
			at: 61.5,
			text: "Hai bên ký hợp đồng thuê nhà.",
			before: "Bàn tiếp phần giá.",
			after: null,
		},
	],
	total: 3,
};

afterEach(() => {
	vi.unstubAllGlobals();
});

describe("SearchPage", () => {
	it("searches after typing stops, marks the match and opens the reader at its second", async () => {
		const fake = fakeFetch({ "GET /api/search": jsonResponse(REPLY) });
		vi.stubGlobal("fetch", fake.fetcher);
		const { router } = renderWithProviders(<SearchPage />, {
			routes: [{ path: "/transcripts/:key", element: <p>trang đọc</p> }],
		});
		expect(screen.getByText(/Gõ vài từ/)).toBeInTheDocument();
		await userEvent.type(
			screen.getByRole("searchbox", { name: "Tìm trong bản chép lời" }),
			"hop dong",
		);
		await waitFor(() => expect(fake.calls).toHaveLength(1));
		expect(fake.calls[0]?.url).toBe("/api/search?q=hop+dong");
		expect(router.state.location.search).toBe("?q=hop+dong");

		const mark = await screen.findByText("hợp đồng");
		expect(mark.tagName).toBe("MARK");
		expect(screen.getByText("1 câu khớp nhất trong 3")).toBeInTheDocument();
		expect(screen.getByText("Bàn tiếp phần giá.")).toBeInTheDocument();
		const row = screen.getByRole("button", {
			name: /Bản ghi trên iPhone 09:41 16\/10\/2026.*· 1:01/,
		});
		await userEvent.click(row);
		expect(router.state.location.pathname).toBe(
			"/transcripts/s%3Aaaaaaaaa-1111",
		);
		expect(router.state.location.search).toBe("?t=61.5");
	});

	it("says when nothing matches", async () => {
		const fake = fakeFetch({
			"GET /api/search": jsonResponse({ results: [], total: 0 }),
		});
		vi.stubGlobal("fetch", fake.fetcher);
		renderWithProviders(<SearchPage />);
		await userEvent.type(screen.getByRole("searchbox"), "xyz");
		expect(
			await screen.findByText("Không có câu nào khớp"),
		).toBeInTheDocument();
	});
});
