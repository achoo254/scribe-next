import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { RouterProvider, createMemoryRouter } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

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
vi.mock("../components/quota-bar", () => ({ QuotaBar: () => null }));
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
	render(<RouterProvider router={router} />);
	return router;
}

afterEach(() => {
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
		screen.getByRole("button", { name: "a@b.c · Tài khoản" }).focus();
		await userEvent.keyboard("{Enter}");
		await userEvent.click(
			await screen.findByRole("menuitem", { name: "Đăng xuất" }),
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
		screen.getByRole("button", { name: "a@b.c · Tài khoản" }).focus();
		await userEvent.keyboard("{Enter}");
		await userEvent.click(
			await screen.findByRole("menuitem", { name: "Đăng xuất" }),
		);
		expect(await screen.findByText("trang đăng nhập")).toBeInTheDocument();
		expect(auth.stop).not.toHaveBeenCalled();
	});
});
