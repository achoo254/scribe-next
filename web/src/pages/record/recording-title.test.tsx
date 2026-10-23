import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
	RecordingItem,
	RecordingList,
} from "../../features/files/files-api";
import { sessionTitle } from "../../features/files/recording-groups";
import { fakeFetch, jsonResponse } from "../../test/fake-fetch";
import { renderWithProviders } from "../../test/render-with-providers";
import { RecordingHistory } from "./recording-history";

vi.mock("../../auth/auth-context", () => ({
	useAuth: () => ({ user: { email: "a@b.c", canTranscribe: false } }),
}));

const NOW = new Date().toISOString();

const FILE: RecordingItem = {
	id: "f1",
	name: "Họp nhóm.webm",
	durationS: 65,
	hasVideo: false,
	size: 1_048_576,
	createdAt: NOW,
};

function list(...items: RecordingItem[]): RecordingList {
	return { retentionDays: 14, items };
}

const PATCH_FILE = "PATCH /api/transcript-docs/f%3Af1";

afterEach(() => {
	vi.unstubAllGlobals();
	window.localStorage.clear();
});

/** Renders the history; PATCH replies wait for `release()` when `gated`. */
function renderHistory(
	routes: Parameters<typeof fakeFetch>[0],
	{ gated = false } = {},
) {
	const fake = fakeFetch(routes);
	let release = () => {};
	const gate = new Promise<void>((resolve) => {
		release = resolve;
	});
	vi.stubGlobal(
		"fetch",
		async (input: RequestInfo | URL, init?: RequestInit) => {
			if (gated && init?.method === "PATCH") await gate;
			return fake.fetcher(input, init);
		},
	);
	renderWithProviders(<RecordingHistory />);
	return { ...fake, release };
}

const patches = (calls: { method: string; url: string; body: unknown }[]) =>
	calls
		.filter((call) => call.method === "PATCH")
		.map((call) => [call.url, call.body]);

async function openEditor(name: string): Promise<HTMLInputElement> {
	await userEvent.click(
		await screen.findByRole("button", { name: `Đổi tên ${name}` }),
	);
	const input = screen.getByRole("textbox", { name: "Tên bản ghi" });
	if (!(input instanceof HTMLInputElement)) throw new Error("not an input");
	return input;
}

describe("RecordingTitle", () => {
	it("saves on Enter, shows the new name before the backend answers", async () => {
		const fake = renderHistory(
			{
				"GET /api/files": [
					jsonResponse(list(FILE)),
					jsonResponse(list({ ...FILE, title: "Họp tuần" })),
				],
				[PATCH_FILE]: jsonResponse({ title: "Họp tuần" }),
			},
			{ gated: true },
		);
		const input = await openEditor("Họp nhóm.webm");
		expect(input).toHaveFocus();
		expect(input).toHaveAttribute("placeholder", "Họp nhóm.webm");
		expect(input).toHaveAttribute("maxlength", "120");
		await userEvent.type(input, "  Họp   tuần {Enter}");

		// Optimistic: the backend has not answered yet.
		expect(await screen.findByText("Họp tuần")).toBeVisible();
		expect(screen.queryByRole("textbox")).toBeNull();
		expect(
			screen.getByRole("button", { name: "Đổi tên Họp tuần" }),
		).toHaveFocus();
		fake.release();
		await waitFor(() =>
			expect(patches(fake.calls)).toEqual([
				["/api/transcript-docs/f%3Af1", { title: "Họp tuần" }],
			]),
		);
		// The list re-reads after the rename and keeps the stored name.
		await waitFor(() =>
			expect(
				fake.calls.filter((call) => call.url === "/api/files"),
			).toHaveLength(2),
		);
		expect(screen.getByText("Họp tuần")).toBeVisible();
	});

	it("cancels on Esc without a request", async () => {
		const fake = renderHistory({ "GET /api/files": jsonResponse(list(FILE)) });
		const input = await openEditor("Họp nhóm.webm");
		await userEvent.type(input, "Tên khác{Escape}");
		expect(screen.queryByRole("textbox")).toBeNull();
		expect(screen.getByText("Họp nhóm.webm")).toBeVisible();
		expect(
			screen.getByRole("button", { name: "Đổi tên Họp nhóm.webm" }),
		).toHaveFocus();
		expect(patches(fake.calls)).toEqual([]);
	});

	it("saves when the field loses focus", async () => {
		const fake = renderHistory({
			"GET /api/files": jsonResponse(list(FILE)),
			[PATCH_FILE]: jsonResponse({ title: "Phỏng vấn" }),
		});
		const input = await openEditor("Họp nhóm.webm");
		await userEvent.type(input, "Phỏng vấn");
		await userEvent.click(document.body);
		await waitFor(() =>
			expect(patches(fake.calls)).toEqual([
				["/api/transcript-docs/f%3Af1", { title: "Phỏng vấn" }],
			]),
		);
	});

	it("goes back to the default name when the field is emptied", async () => {
		const fake = renderHistory({
			"GET /api/files": [
				jsonResponse(list({ ...FILE, title: "Họp tuần" })),
				jsonResponse(list(FILE)),
			],
			[PATCH_FILE]: jsonResponse({ title: null }),
		});
		const input = await openEditor("Họp tuần");
		expect(input).toHaveValue("Họp tuần");
		await userEvent.clear(input);
		await userEvent.type(input, "   {Enter}");
		expect(await screen.findByText("Họp nhóm.webm")).toBeVisible();
		await waitFor(() =>
			expect(patches(fake.calls)).toEqual([
				["/api/transcript-docs/f%3Af1", { title: null }],
			]),
		);
	});

	it("sends nothing when the name did not change", async () => {
		const fake = renderHistory({
			"GET /api/files": jsonResponse(list({ ...FILE, title: "Họp tuần" })),
		});
		const input = await openEditor("Họp tuần");
		await userEvent.type(input, " {Enter}");
		expect(screen.getByText("Họp tuần")).toBeVisible();
		expect(patches(fake.calls)).toEqual([]);
	});

	it("puts the old name back and says why when the backend refuses", async () => {
		renderHistory({
			"GET /api/files": jsonResponse(list(FILE)),
			[PATCH_FILE]: jsonResponse({ error: "tên tối đa 120 ký tự" }, 400),
		});
		const input = await openEditor("Họp nhóm.webm");
		await userEvent.type(input, "Tên mới{Enter}");
		expect(await screen.findByText("Không đổi được tên")).toBeVisible();
		expect(screen.getByText("tên tối đa 120 ký tự")).toBeVisible();
		await waitFor(() =>
			expect(screen.getByText("Họp nhóm.webm")).toBeVisible(),
		);
		expect(screen.queryByText("Tên mới")).toBeNull();
	});

	it("renames a whole iPhone session, whose parts share its name", async () => {
		const part = (index: number): RecordingItem => ({
			id: `s${index}`,
			name: `aaaaaaaa-1111#${index}`,
			durationS: 60,
			hasVideo: false,
			size: 2_097_152,
			createdAt: NOW,
		});
		const fake = renderHistory({
			"GET /api/files": [
				jsonResponse(list(part(1), part(0))),
				jsonResponse(
					list(
						{ ...part(1), title: "Họp khách hàng" },
						{ ...part(0), title: "Họp khách hàng" },
					),
				),
			],
			"PATCH /api/transcript-docs/s%3Aaaaaaaaa-1111": jsonResponse({
				title: "Họp khách hàng",
			}),
		});
		const input = await openEditor(sessionTitle(NOW));
		await userEvent.type(input, "Họp khách hàng{Enter}");
		expect(await screen.findByText("Họp khách hàng")).toBeVisible();
		expect(
			screen.getByRole("button", { name: "Xoá cả phiên Họp khách hàng" }),
		).toBeVisible();
		await waitFor(() =>
			expect(patches(fake.calls)).toEqual([
				["/api/transcript-docs/s%3Aaaaaaaaa-1111", { title: "Họp khách hàng" }],
			]),
		);
	});
});
