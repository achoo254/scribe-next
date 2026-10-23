import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { RecordingList } from "../../features/files/files-api";
import { PENDING_DELETES_KEY } from "../../features/files/use-undoable-delete";
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
	window.localStorage.clear();
});

function renderHistory(routes: Parameters<typeof fakeFetch>[0]) {
	const fake = fakeFetch(routes);
	vi.stubGlobal("fetch", fake.fetcher);
	renderWithProviders(<RecordingHistory />);
	return fake;
}

const SESSION_LIST: RecordingList = {
	retentionDays: 7,
	items: [
		...LIST.items,
		{
			id: "s0",
			name: "aaaaaaaa-1111#0",
			durationS: 60,
			hasVideo: false,
			size: 2_097_152,
			createdAt: new Date().toISOString(),
		},
		{
			id: "s1",
			name: "aaaaaaaa-1111#1",
			durationS: 60,
			hasVideo: false,
			size: 2_097_152,
			createdAt: new Date().toISOString(),
		},
		{
			id: "u1",
			kind: "upload",
			name: "phong-van.mp3",
			durationS: 30,
			hasVideo: false,
			size: 1_048_576,
			createdAt: new Date().toISOString(),
		},
	],
};

const deletes = (calls: { method: string; url: string }[]) =>
	calls.filter((c) => c.method === "DELETE").map((c) => c.url);

describe("RecordingHistory", () => {
	afterEach(() => {
		vi.useRealTimers();
	});

	it("shows each file's own expiry, not one from the current plan", async () => {
		const DAY = 86_400_000;
		const [file] = LIST.items;
		if (!file) throw new Error("no fixture");
		renderHistory({
			"GET /api/files": jsonResponse({
				retentionDays: 14,
				items: [
					{
						...file,
						// Made on free (7 days) five days ago; the plan is plus now.
						createdAt: new Date(Date.now() - 5 * DAY).toISOString(),
						expiresAt: new Date(Date.now() + 2 * DAY).toISOString(),
					},
				],
			}),
		});
		expect(await screen.findByText(/tự xoá sau 2 ngày/)).toBeVisible();
		expect(screen.queryByText(/tự xoá sau 9 ngày/)).toBeNull();
	});

	it("deletes with one tap, offers undo, and sends the DELETE when the toast expires", async () => {
		vi.useFakeTimers({ shouldAdvanceTime: true });
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		const fake = renderHistory({
			"GET /api/files": [jsonResponse(LIST), jsonResponse({ items: [] })],
			"DELETE /api/files/f1": new Response(null, { status: 204 }),
		});
		// Playback stays a visible button, download is in the ⋮ menu, delete is one tap.
		expect(
			await screen.findByRole("button", { name: "Nghe lại" }),
		).toBeVisible();
		await user.click(screen.getByRole("button", { name: "Xoá Họp nhóm.webm" }));

		expect(screen.queryByText("Họp nhóm.webm")).toBeNull();
		expect(screen.queryByRole("dialog")).toBeNull();
		expect(await screen.findByText("Đã xoá “Họp nhóm.webm”")).toBeVisible();
		expect(screen.getByRole("button", { name: "Hoàn tác" })).toBeVisible();
		expect(deletes(fake.calls)).toEqual([]);

		await act(() => vi.advanceTimersByTimeAsync(5_100));
		await waitFor(() => expect(deletes(fake.calls)).toEqual(["/api/files/f1"]));
		expect(await screen.findByText("Chưa có bản ghi nào")).toBeVisible();
	});

	it("sends a delete the app was closed on during its undo window", async () => {
		// Left by an earlier run: the iOS app was swiped away in the background.
		window.localStorage.setItem(PENDING_DELETES_KEY, JSON.stringify(["f1"]));
		const fake = renderHistory({
			"GET /api/files": [jsonResponse(LIST), jsonResponse({ items: [] })],
			"DELETE /api/files/f1": new Response(null, { status: 204 }),
		});
		await waitFor(() => expect(deletes(fake.calls)).toEqual(["/api/files/f1"]));
		expect(await screen.findByText("Chưa có bản ghi nào")).toBeVisible();
		expect(screen.queryByText("Họp nhóm.webm")).toBeNull();
		expect(screen.queryByRole("button", { name: "Hoàn tác" })).toBeNull();
		await waitFor(() =>
			expect(window.localStorage.getItem(PENDING_DELETES_KEY)).toBeNull(),
		);
	});

	it("remembers a delete while it can be undone, and forgets it on undo", async () => {
		renderHistory({ "GET /api/files": jsonResponse(LIST) });
		await userEvent.click(
			await screen.findByRole("button", { name: "Xoá Họp nhóm.webm" }),
		);
		expect(window.localStorage.getItem(PENDING_DELETES_KEY)).toBe('["f1"]');
		await userEvent.click(screen.getByRole("button", { name: "Hoàn tác" }));
		expect(window.localStorage.getItem(PENDING_DELETES_KEY)).toBeNull();
	});

	it("restores the item on “Hoàn tác” and never deletes it", async () => {
		const fake = renderHistory({ "GET /api/files": jsonResponse(LIST) });
		await userEvent.click(
			await screen.findByRole("button", { name: "Xoá Họp nhóm.webm" }),
		);
		expect(screen.queryByText("Họp nhóm.webm")).toBeNull();
		await userEvent.click(screen.getByRole("button", { name: "Hoàn tác" }));

		expect(await screen.findByText("Họp nhóm.webm")).toBeVisible();
		await waitFor(() =>
			expect(screen.queryByRole("button", { name: "Hoàn tác" })).toBeNull(),
		);
		expect(deletes(fake.calls)).toEqual([]);
	});

	it("deletes a whole iPhone session in one tap", async () => {
		const fake = renderHistory({
			"GET /api/files": jsonResponse(SESSION_LIST),
			"DELETE /api/files/s0": new Response(null, { status: 204 }),
			"DELETE /api/files/s1": new Response(null, { status: 204 }),
		});
		const remove = await screen.findByRole("button", {
			name: /^Xoá cả phiên Bản ghi trên iPhone/,
		});
		// An uploaded file keeps its own row even next to a session.
		expect(screen.getByText("phong-van.mp3")).toBeVisible();
		expect(screen.getByText("Tệp tải lên")).toBeVisible();
		await userEvent.click(remove);
		expect(screen.queryByText(/^Bản ghi trên iPhone/)).toBeNull();
		// Closing the toast ends the undo window like the timer does.
		await userEvent.click(
			screen.getByRole("button", { name: "Đóng thông báo" }),
		);
		await waitFor(() =>
			expect(deletes(fake.calls)).toEqual(["/api/files/s0", "/api/files/s1"]),
		);
		// Sent together and kept alive: closing the tab now cannot cut the session in half.
		expect(
			fake.calls.filter((c) => c.method === "DELETE").map((c) => c.keepalive),
		).toEqual([true, true]);
	});

	it("sends pending deletes at once when the history leaves the page", async () => {
		const fake = fakeFetch({
			"GET /api/files": jsonResponse(LIST),
			"DELETE /api/files/f1": new Response(null, { status: 204 }),
		});
		vi.stubGlobal("fetch", fake.fetcher);
		const { router } = renderWithProviders(<RecordingHistory />, {
			routes: [{ path: "/split", element: <p>trang khác</p> }],
		});
		await userEvent.click(
			await screen.findByRole("button", { name: "Xoá Họp nhóm.webm" }),
		);
		expect(deletes(fake.calls)).toEqual([]);
		await act(() => router.navigate("/split"));
		await waitFor(() => expect(deletes(fake.calls)).toEqual(["/api/files/f1"]));
	});

	it("confirms “Xoá tất cả” with the count and total size", async () => {
		const fake = renderHistory({
			"GET /api/files": [
				jsonResponse(SESSION_LIST),
				jsonResponse({ items: [], retentionDays: 7 }),
			],
			"DELETE /api/files": jsonResponse({
				deleted: 3,
				skipped: 1,
				failed: 0,
				bytes: 5_242_880,
			}),
		});
		await userEvent.click(
			await screen.findByRole("button", { name: "Xoá tất cả" }),
		);
		const dialog = await screen.findByRole("dialog", {
			name: "Xoá toàn bộ lịch sử ghi",
		});
		expect(dialog).toHaveTextContent("Xoá 4 tệp, tổng 6.0 MB?");
		expect(deletes(fake.calls)).toEqual([]);
		await userEvent.click(
			within(dialog).getByRole("button", { name: "Xoá tất cả" }),
		);

		expect(await screen.findByText("Đã xoá 3 tệp")).toBeVisible();
		expect(
			screen.getByText("Giữ lại 1 tệp của bản ghi đang ghi hoặc đang cắt."),
		).toBeVisible();
		expect(deletes(fake.calls)).toEqual(["/api/files"]);
		expect(await screen.findByText("Chưa có bản ghi nào")).toBeVisible();
	});

	it("keeps the history when “Xoá tất cả” is cancelled", async () => {
		const fake = renderHistory({ "GET /api/files": jsonResponse(LIST) });
		await userEvent.click(
			await screen.findByRole("button", { name: "Xoá tất cả" }),
		);
		await userEvent.click(
			within(await screen.findByRole("dialog")).getByRole("button", {
				name: "Huỷ",
			}),
		);
		expect(screen.getByText("Họp nhóm.webm")).toBeVisible();
		expect(deletes(fake.calls)).toEqual([]);
	});

	it("shows each day as its own titled section with its count, duration and download", async () => {
		const now = new Date();
		const at = (daysAgo: number, hour: number) =>
			new Date(
				now.getFullYear(),
				now.getMonth(),
				now.getDate() - daysAgo,
				hour,
			).toISOString();
		const day = (daysAgo: number) =>
			new Date(
				now.getFullYear(),
				now.getMonth(),
				now.getDate() - daysAgo,
			).toLocaleDateString("vi-VN", {
				day: "2-digit",
				month: "2-digit",
				year: "numeric",
			});
		const item = (id: string, createdAt: string, durationS: number) => ({
			id,
			name: id,
			durationS,
			hasVideo: false,
			size: 1,
			createdAt,
		});
		renderHistory({
			"GET /api/files": jsonResponse({
				retentionDays: 14,
				items: [
					// One iPhone session (two segments) and one recording today.
					item("aaaaaaaa-1111#1", at(0, 10), 60),
					item("aaaaaaaa-1111#0", at(0, 10), 60),
					item("today-web", at(0, 9), 65),
					item("yesterday-web", at(1, 9), 30),
				],
			}),
		});
		const today = await screen.findByRole("region", {
			name: `Hôm nay ${day(0)}`,
		});
		expect(
			within(today).getByRole("heading", {
				level: 3,
				name: `Hôm nay ${day(0)}`,
			}),
		).toBeVisible();
		// Rows, not files: the session counts once.
		expect(today).toHaveTextContent("2 bản ghi · 3:05");
		expect(
			within(today).getByRole("button", { name: `Tải ngày ${day(0)}` }),
		).toBeVisible();
		expect(within(today).getByText("today-web")).toBeVisible();
		expect(within(today).queryByText("yesterday-web")).toBeNull();

		const yesterday = screen.getByRole("region", {
			name: `Hôm qua ${day(1)}`,
		});
		expect(yesterday).toHaveTextContent("1 bản ghi · 0:30");
		expect(within(yesterday).getByText("yesterday-web")).toBeVisible();
		// The retention notice stays above the days.
		expect(screen.getByText(/tự động xoá sau 14 ngày/)).toBeVisible();
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
