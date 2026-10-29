import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { CurrentUser } from "../auth/auth-api";
import {
	accountMenuItems,
	accountTrigger,
	openAccountMenu,
} from "../test/account-menu";
import { fakeFetch, jsonResponse } from "../test/fake-fetch";
import { TestProviders } from "../test/test-providers";
import type { SessionActivity } from "./session-activity";

const auth = vi.hoisted(() => ({
	logout: vi.fn<() => Promise<void>>(async () => undefined),
}));

vi.mock("../auth/auth-context", () => ({
	useAuth: () => ({ logout: auth.logout }),
}));

const { AccountControls } = await import("./account-controls");
const { IDLE_ACTIVITY } = await import("./session-activity");

const USER: CurrentUser = {
	email: "minh.anh@example.com",
	name: "Minh Anh",
	picture: "https://lh3.googleusercontent.com/a/p1",
	canTranscribe: true,
};

function renderControls(
	activity: SessionActivity = IDLE_ACTIVITY,
	beforeSignOut = vi.fn<() => Promise<void>>(async () => undefined),
	{
		user = USER,
		routes = { "GET /api/chat/link": jsonResponse({ linked: false }) },
	}: {
		user?: CurrentUser;
		routes?: Parameters<typeof fakeFetch>[0];
	} = {},
) {
	const fake = fakeFetch(routes);
	vi.stubGlobal("fetch", fake.fetcher);
	render(
		<TestProviders>
			<MemoryRouter initialEntries={["/"]}>
				<Routes>
					<Route
						path="/"
						element={
							<AccountControls
								user={user}
								activity={activity}
								beforeSignOut={beforeSignOut}
							/>
						}
					/>
					<Route path="/login" element={<p>trang đăng nhập</p>} />
					<Route path="/privacy" element={<p>trang /privacy</p>} />
					<Route path="/terms" element={<p>trang /terms</p>} />
				</Routes>
			</MemoryRouter>
		</TestProviders>,
	);
	return { beforeSignOut, calls: fake.calls };
}

const openMenu = () => openAccountMenu();

/** An item (button or link) of the open menu by its accessible name. */
function item(menu: HTMLElement, name: string | RegExp): HTMLElement {
	return (
		within(menu).queryByRole("button", { name }) ??
		within(menu).getByRole("link", { name })
	);
}

async function chooseSignOut() {
	const menu = await openMenu();
	await userEvent.click(item(menu, "Đăng xuất"));
}

afterEach(() => {
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
	auth.logout.mockClear();
});

describe("account menu", () => {
	it("shows the IdP picture and name, then the email and the entries in order", async () => {
		renderControls();
		const trigger = accountTrigger();
		expect(trigger).toHaveAttribute("aria-label", "Minh Anh · Tài khoản");
		expect(trigger.querySelector("img")).toHaveAttribute(
			"src",
			"https://lh3.googleusercontent.com/a/p1",
		);
		const menu = await openMenu();
		expect(menu).toHaveTextContent("minh.anh@example.com");
		expect(accountMenuItems(menu)).toEqual([
			"account-menu-keys",
			"account-menu-chat-link",
			"account-menu-privacy",
			"account-menu-terms",
			"account-menu-delete",
			"account-menu-idp-email",
			"account-menu-idp-sign-in",
			"account-menu-sign-out",
		]);
	});

	it("draws the initial of the email when the backend has no name or picture yet", () => {
		renderControls(undefined, undefined, {
			user: { email: USER.email, canTranscribe: true },
		});
		const trigger = accountTrigger();
		expect(trigger).toHaveAttribute(
			"aria-label",
			"minh.anh@example.com · Tài khoản",
		);
		expect(trigger.querySelector("img")).toBeNull();
		expect(trigger.querySelector("[data-initial]")).toHaveAttribute(
			"data-initial",
			"M",
		);
	});

	it.each([
		["Quyền riêng tư", "/privacy"],
		["Điều khoản", "/terms"],
	])("opens '%s' inside the app", async (label, path) => {
		renderControls();
		const menu = await openMenu();
		await userEvent.click(item(menu, label));
		expect(await screen.findByText(`trang ${path}`)).toBeInTheDocument();
	});

	it("opens 'Xoá tài khoản', which waits while a recording runs", async () => {
		renderControls({ ...IDLE_ACTIVITY, recording: true });
		const menu = await openMenu();
		await userEvent.click(item(menu, "Xoá tài khoản"));
		const dialog = await screen.findByRole("dialog", {
			name: "Xoá tài khoản Scribe",
		});
		expect(dialog).toHaveTextContent("Dừng ghi và chờ tải xong");
		expect(
			within(dialog).getByRole("button", { name: "Xoá tài khoản" }),
		).toBeDisabled();
	});

	it("opens Key riêng from the menu for every account", async () => {
		const user = userEvent.setup();
		renderControls(undefined, undefined, {
			user: { email: USER.email, canTranscribe: false },
			routes: {
				"GET /api/me/keys": jsonResponse({
					available: true,
					keys: [],
					prefs: {},
					active: {},
					models: {},
				}),
			},
		});
		const menu = await openAccountMenu(user);
		await user.click(item(menu, "Key riêng"));
		expect(
			await screen.findByRole("dialog", { name: "Key riêng" }),
		).toBeVisible();
		expect(
			await screen.findByRole("region", { name: "ElevenLabs" }),
		).toBeVisible();
	});

	it("has no chat entry for an account that cannot transcribe", async () => {
		const { calls } = renderControls(undefined, undefined, {
			user: { email: USER.email, canTranscribe: false },
			routes: {},
		});
		const menu = await openMenu();
		expect(within(menu).queryByRole("button", { name: /chat/ })).toBeNull();
		expect(calls).toEqual([]);
	});

	it("offers no linking when the server takes no links", async () => {
		const { calls } = renderControls(undefined, undefined, {
			routes: {
				"GET /api/chat/link": jsonResponse({
					linked: false,
					linkedAt: null,
					enabled: false,
				}),
			},
		});
		await waitFor(() => expect(calls).toHaveLength(1));
		const menu = await openMenu();
		await waitFor(() =>
			expect(menu).not.toHaveTextContent("Liên kết chat.hoandat.com"),
		);
	});

	it("polls after the deep link opened until the link is confirmed in chat", async () => {
		vi.spyOn(window, "open").mockReturnValue(null);
		renderControls(undefined, undefined, {
			routes: {
				"GET /api/chat/link": [
					jsonResponse({ linked: false, linkedAt: null, enabled: true }),
					jsonResponse({ linked: false, linkedAt: null, enabled: true }),
					jsonResponse({
						linked: true,
						linkedAt: "2026-10-16T02:30:00Z",
						enabled: true,
					}),
				],
				"POST /api/chat/link": jsonResponse({
					url: "https://chat.hoandat.com/b/bot_scribe_bot?start=tok1",
					expiresAt: new Date(Date.now() + 600_000).toISOString(),
				}),
			},
		});
		const menu = await openMenu();
		await userEvent.click(item(menu, "Liên kết chat.hoandat.com"));
		const open = await screen.findByRole("button", {
			name: /^Mở chat/,
		});
		await waitFor(() => expect(open).toBeEnabled());
		await userEvent.click(open);
		expect(
			await screen.findByText(/bấm “Xác nhận liên kết” trong chat/),
		).toBeInTheDocument();
		// Two polls, 3 s apart, without any focus change.
		expect(
			await screen.findByText(
				"Đã liên kết. Biên bản sẽ được gửi vào chat riêng của bạn với bot Scribe.",
				undefined,
				{ timeout: 8000 },
			),
		).toBeInTheDocument();
	}, 12_000);

	it("opens the linking steps and the deep link from the button's click", async () => {
		const open = vi.spyOn(window, "open").mockReturnValue(null);
		renderControls(undefined, undefined, {
			routes: {
				"GET /api/chat/link": jsonResponse({ linked: false }),
				"POST /api/chat/link": jsonResponse({
					url: "https://chat.hoandat.com/b/bot_scribe_bot?start=tok1",
					expiresAt: "2026-10-16T03:10:00Z",
				}),
			},
		});
		const menu = await openMenu();
		await userEvent.click(item(menu, "Liên kết chat.hoandat.com"));
		expect(menu.open).toBe(false);
		const dialog = await screen.findByRole("dialog", {
			name: "Liên kết chat.hoandat.com",
		});
		expect(dialog).toHaveTextContent("Gửi /start");
		expect(
			await screen.findByText(
				"Đường dẫn chỉ dùng một lần, hết hạn lúc 10:10 16/10.",
			),
		).toBeInTheDocument();
		await userEvent.click(screen.getByRole("button", { name: /^Mở chat/ }));
		expect(open).toHaveBeenCalledWith(
			"https://chat.hoandat.com/b/bot_scribe_bot?start=tok1",
			"_blank",
			"noopener,noreferrer",
		);
	});

	it("re-reads the link when the window gets the focus back, names the chat and unlinks", async () => {
		const { calls } = renderControls(undefined, undefined, {
			routes: {
				"GET /api/chat/link": [
					jsonResponse({ linked: false, linkedAt: null, enabled: true }),
					jsonResponse({
						linked: true,
						linkedAt: "2026-10-16T02:30:00Z",
						chatName: "An Nguyễn",
						enabled: true,
					}),
				],
				"DELETE /api/chat/link": jsonResponse({ linked: false }),
			},
		});
		await waitFor(() => expect(calls).toHaveLength(1));
		window.dispatchEvent(new Event("focus"));
		await waitFor(() => expect(calls).toHaveLength(2));
		const menu = await openMenu();
		await userEvent.click(
			await within(menu).findByRole("button", {
				name: "Gỡ liên kết chat (An Nguyễn)",
			}),
		);
		expect(await screen.findByText("Đã gỡ liên kết chat")).toBeInTheDocument();
		expect(calls.map((call) => call.method)).toContain("DELETE");
	});

	it.each([
		["Email", "https://auth.hoandat.com/account#account-email"],
		[
			"Phương thức đăng nhập",
			"https://auth.hoandat.com/account#account-sign-in",
		],
	])("links '%s' of the IdP account in a new tab", async (label, url) => {
		renderControls();
		const menu = await openMenu();
		const link = within(menu).getByRole("link", {
			// jsdom puts a space before the sr-only suffix; browsers do not.
			name: new RegExp(
				`^${label} ?, mở trang tài khoản hoandat\\.com ở tab mới$`,
			),
		});
		expect(link).toHaveAttribute("href", url);
		expect(link).toHaveAttribute("target", "_blank");
		expect(menu).toHaveTextContent("Tài khoản hoandat.com · mở tab mới");
	});

	it("opens the IdP pages in the system browser inside the native app", async () => {
		vi.stubGlobal("Capacitor", { isNativePlatform: () => true });
		const open = vi.spyOn(window, "open").mockReturnValue(null);
		renderControls();
		const menu = await openMenu();
		await userEvent.click(within(menu).getByRole("link", { name: /^Email/ }));
		expect(open).toHaveBeenCalledWith(
			"https://auth.hoandat.com/account#account-email",
			"_blank",
			"noopener,noreferrer",
		);
	});

	it("signs out at once when nothing is running", async () => {
		const { beforeSignOut } = renderControls();
		await chooseSignOut();
		expect(await screen.findByText("trang đăng nhập")).toBeInTheDocument();
		expect(beforeSignOut).toHaveBeenCalledTimes(1);
		expect(auth.logout).toHaveBeenCalledTimes(1);
		expect(screen.queryByRole("dialog")).toBeNull();
	});

	it("asks first while recording, and keeps the session on cancel", async () => {
		renderControls({ ...IDLE_ACTIVITY, recording: true });
		await chooseSignOut();
		const dialog = await screen.findByRole("dialog", { name: "Đăng xuất" });
		expect(dialog).toHaveTextContent("Đăng xuất khi chưa xong việc?");
		expect(dialog).toHaveTextContent(
			"Bản ghi đang chạy sẽ dừng; phần đã ghi vẫn được lưu.",
		);
		// Destructive dialogs open on the safe choice.
		expect(screen.getByRole("button", { name: "Huỷ" })).toHaveFocus();
		await userEvent.click(screen.getByRole("button", { name: "Huỷ" }));
		await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
		expect(auth.logout).not.toHaveBeenCalled();
	});

	it("saves the recording before deleting the session once confirmed", async () => {
		const order: string[] = [];
		auth.logout.mockImplementationOnce(async () => {
			order.push("logout");
		});
		renderControls(
			{ ...IDLE_ACTIVITY, recording: true },
			vi.fn(async () => {
				order.push("save recording");
			}),
		);
		await chooseSignOut();
		await userEvent.click(
			await screen.findByRole("button", { name: "Vẫn đăng xuất" }),
		);
		expect(await screen.findByText("trang đăng nhập")).toBeInTheDocument();
		expect(order).toEqual(["save recording", "logout"]);
	});

	it("names the segments still waiting on the phone and a file upload", async () => {
		renderControls({
			recording: false,
			segmentsWaiting: 3,
			fileUploading: true,
		});
		await chooseSignOut();
		const dialog = await screen.findByRole("dialog");
		expect(dialog).toHaveTextContent(
			"3 đoạn chưa tải lên vẫn giữ trên máy và tải tiếp khi bạn đăng nhập lại.",
		);
		expect(dialog).toHaveTextContent(
			"Tệp đang tải lên ở Cắt file sẽ dừng giữa chừng.",
		);
		expect(dialog).not.toHaveTextContent("Bản ghi đang chạy");
	});
});
