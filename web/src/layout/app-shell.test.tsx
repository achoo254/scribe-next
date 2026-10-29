import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { RouterProvider, createMemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { openAccountMenu } from "../test/account-menu";
import { fakeFetch, jsonResponse } from "../test/fake-fetch";
import { TestProviders } from "../test/test-providers";

const auth = vi.hoisted(() => ({
	recording: false,
	logout: vi.fn<() => Promise<void>>(async () => undefined),
	stop: vi.fn<() => Promise<void>>(async () => undefined),
}));

vi.mock("../auth/auth-context", () => ({
	useAuth: () => ({
		user: { email: "a@b.c", canTranscribe: true },
		logout: auth.logout,
	}),
}));
vi.mock("../components/usage-card", () => ({ UsageCard: () => null }));
// Stands in for `RecordStateProvider` with a browser recording running.
vi.mock("../pages/record/record-state", () => ({
	useOptionalBrowserRecordState: () =>
		auth.recording
			? {
					rec: { recording: true, starting: false, finalizing: false },
					stopAndSave: auth.stop,
				}
			: null,
	useOptionalNativeRecordState: () => null,
}));

const { AppShell } = await import("./app-shell");

function renderShell() {
	const router = createMemoryRouter(
		[
			{
				path: "/",
				element: <AppShell />,
				children: [{ path: "record", element: <p>record page</p> }],
			},
			{ path: "/login", element: <p>trang đăng nhập</p> },
		],
		{ initialEntries: ["/record"] },
	);
	render(
		<TestProviders>
			<RouterProvider router={router} />
		</TestProviders>,
	);
	return router;
}

beforeEach(() => {
	vi.stubGlobal(
		"fetch",
		fakeFetch({ "GET /api/chat/link": jsonResponse({ linked: false }) })
			.fetcher,
	);
});

afterEach(() => {
	vi.unstubAllGlobals();
	auth.recording = false;
	auth.logout.mockClear();
	auth.stop.mockClear();
});

describe("AppShell", () => {
	it("lays out skip link, top bar, the page of the URL and footer", () => {
		renderShell();
		expect(
			screen.getByRole("link", { name: "Bỏ qua tới nội dung chính" }),
		).toHaveAttribute("href", "#main");
		expect(screen.getByRole("banner")).toBeInTheDocument();
		expect(screen.getByRole("main")).toHaveAttribute("id", "main");
		expect(screen.getByRole("main")).toHaveTextContent("record page");
		expect(screen.getByRole("contentinfo")).toBeInTheDocument();
	});

	it("asks before signing out while a recording runs, saves it, then signs out", async () => {
		auth.recording = true;
		renderShell();
		const menu = await openAccountMenu();
		await userEvent.click(
			within(menu).getByRole("button", { name: "Đăng xuất" }),
		);
		await userEvent.click(
			await screen.findByRole("button", { name: "Vẫn đăng xuất" }),
		);
		expect(await screen.findByText("trang đăng nhập")).toBeInTheDocument();
		expect(auth.stop).toHaveBeenCalledTimes(1);
		expect(auth.stop.mock.invocationCallOrder[0]).toBeLessThan(
			auth.logout.mock.invocationCallOrder[0] ?? 0,
		);
	});

	it("signs out without asking when nothing runs", async () => {
		renderShell();
		const menu = await openAccountMenu();
		await userEvent.click(
			within(menu).getByRole("button", { name: "Đăng xuất" }),
		);
		expect(await screen.findByText("trang đăng nhập")).toBeInTheDocument();
		expect(auth.stop).not.toHaveBeenCalled();
	});
});
