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

	it("asks for a second character before searching a lone letter", async () => {
		const fake = fakeFetch({ "GET /api/search": jsonResponse(REPLY) });
		vi.stubGlobal("fetch", fake.fetcher);
		const { router } = renderWithProviders(<SearchPage />);
		await userEvent.type(screen.getByRole("searchbox"), "h");
		await waitFor(() => expect(router.state.location.search).toBe("?q=h"));
		expect(screen.getByText("Gõ ít nhất 2 ký tự để tìm.")).toBeInTheDocument();
		expect(fake.calls).toHaveLength(0);
	});

	it("keeps marking the previous query while the next one loads", async () => {
		let calls = 0;
		const fetcher = vi.fn(async () => {
			calls += 1;
			if (calls === 1) return jsonResponse(REPLY);
			return new Promise<Response>(() => undefined);
		});
		vi.stubGlobal("fetch", fetcher);
		renderWithProviders(<SearchPage />);
		const field = screen.getByRole("searchbox");
		await userEvent.type(field, "hop dong");
		expect((await screen.findByText("hợp đồng")).tagName).toBe("MARK");
		await userEvent.clear(field);
		await userEvent.type(field, "nha");
		await waitFor(() => expect(fetcher).toHaveBeenCalledTimes(2));
		// The sentence on screen still answers "hop dong": "nhà" is not marked.
		expect(screen.getByText("hợp đồng").tagName).toBe("MARK");
		expect(screen.queryByText("nhà")).not.toBeInTheDocument();
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
