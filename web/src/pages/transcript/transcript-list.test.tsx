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

function renderList(replies: Response[]) {
	vi.stubGlobal(
		"fetch",
		fakeFetch({ "GET /api/transcript-docs": replies }).fetcher,
	);
	const onOpen = vi.fn();
	renderWithProviders(<TranscriptList onOpen={onOpen} />);
	return onOpen;
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
		const onOpen = renderList([jsonResponse({ items: [DOC] })]);
		await userEvent.click(
			await screen.findByRole("button", { name: /Phỏng vấn khách hàng/ }),
		);
		expect(onOpen).toHaveBeenCalledWith(DOC.key);
		expect(screen.getByText("· 2 đoạn")).toBeInTheDocument();
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
			await screen.findByRole("button", { name: /Phỏng vấn khách hàng/ }),
		).toBeInTheDocument();
	});
});
