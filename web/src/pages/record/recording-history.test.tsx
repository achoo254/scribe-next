import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { RecordingList } from "../../features/files/files-api";
import { fakeFetch, jsonResponse } from "../../test/fake-fetch";
import { renderWithProviders } from "../../test/render-with-providers";
import { RecordingHistory } from "./recording-history";

vi.mock("../../auth/auth-context", () => ({
	useAuth: () => ({ user: { email: "a@b.c", canTranscribe: false } }),
}));

const LIST: RecordingList = {
	retentionDays: 30,
	items: [
		{
			id: "f1",
			name: "Họp nhóm.webm",
			durationS: 65,
			hasVideo: false,
			size: 1_048_576,
			createdAt: new Date().toISOString(),
			transcript: null,
		},
	],
};

afterEach(() => {
	vi.unstubAllGlobals();
});

function renderHistory(routes: Parameters<typeof fakeFetch>[0]) {
	const fake = fakeFetch(routes);
	vi.stubGlobal("fetch", fake.fetcher);
	renderWithProviders(<RecordingHistory />);
	return fake;
}

describe("RecordingHistory", () => {
	it("deletes a recording from its ⋮ menu after confirming, then says so", async () => {
		const fake = renderHistory({
			"GET /api/files": [jsonResponse(LIST), jsonResponse({ items: [] })],
			"DELETE /api/files/f1": new Response(null, { status: 204 }),
		});
		const menu = await screen.findByRole("button", {
			name: "Thao tác với Họp nhóm.webm",
		});
		// Playback stays a visible button; download and delete are in the menu.
		expect(screen.getByRole("button", { name: "Nghe lại" })).toBeVisible();
		expect(screen.queryByRole("button", { name: "Xoá" })).toBeNull();

		menu.focus();
		await userEvent.keyboard("{Enter}");
		expect(
			await screen.findByRole("menuitem", { name: "Tải về" }),
		).toBeInTheDocument();
		await userEvent.click(screen.getByRole("menuitem", { name: "Xoá" }));
		const dialog = await screen.findByRole("dialog", { name: "Xoá bản ghi" });
		expect(dialog).toHaveTextContent("Xoá “Họp nhóm.webm”?");
		await userEvent.click(within(dialog).getByRole("button", { name: "Xoá" }));

		expect(await screen.findByText("Đã xoá “Họp nhóm.webm”")).toBeVisible();
		expect(fake.calls.map((c) => `${c.method} ${c.url}`)).toContain(
			"DELETE /api/files/f1",
		);
		expect(await screen.findByText("Chưa có bản ghi nào")).toBeVisible();
	});

	it("offers a retry when the list fails to load", async () => {
		renderHistory({
			"GET /api/files": [
				jsonResponse({ error: "Máy chủ bận" }, 503),
				jsonResponse(LIST),
			],
		});
		expect(
			await screen.findByText("Không tải được lịch sử ghi"),
		).toBeInTheDocument();
		await userEvent.click(screen.getByRole("button", { name: "Thử lại" }));
		await waitFor(() =>
			expect(screen.getByText("Họp nhóm.webm")).toBeInTheDocument(),
		);
	});
});
