import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { CurrentUser } from "../auth/auth-api";
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
				</Routes>
			</MemoryRouter>
		</TestProviders>,
	);
	return { beforeSignOut, calls: fake.calls };
}

async function openMenu() {
	screen
		.getByRole("button", { name: "minh.anh@example.com · Tài khoản" })
		.focus();
	await userEvent.keyboard("{Enter}");
	return screen.findByRole("menu");
}

async function chooseSignOut() {
	await openMenu();
	await userEvent.click(screen.getByRole("menuitem", { name: "Đăng xuất" }));
}

afterEach(() => {
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
	auth.logout.mockClear();
});

describe("account menu", () => {
	it("shows the email and lists the entries in order", async () => {
		renderControls();
		const menu = await openMenu();
		expect(menu).toHaveTextContent("minh.anh@example.com");
		const items = screen
			.getAllByRole("menuitem")
			.map((item) => item.getAttribute("data-id"));
		expect(items).toEqual([
			"account-menu-account",
			"account-menu-apps",
			"account-menu-chat-link",
			"topbar-logout",
		]);
	});

	it("has no chat entry for an account that cannot transcribe", async () => {
		const { calls } = renderControls(undefined, undefined, {
			user: { email: USER.email, canTranscribe: false },
			routes: {},
		});
		await openMenu();
		expect(screen.queryByRole("menuitem", { name: /chat/ })).toBeNull();
		expect(calls).toEqual([]);
	});

	it("offers no linking when the server takes no links", async () => {
		renderControls(undefined, undefined, {
			routes: {
				"GET /api/chat/link": jsonResponse({
					linked: false,
					linkedAt: null,
					enabled: false,
				}),
			},
		});
		await waitFor(async () => {
			const menu = await openMenu();
			expect(menu).not.toHaveTextContent("Liên kết chat.hoandat.com");
		});
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
		await openMenu();
		await userEvent.click(
			await screen.findByRole("menuitem", {
				name: "Liên kết chat.hoandat.com",
			}),
		);
		const open = await screen.findByRole("button", {
			name: /^Mở chat\.hoandat\.com/,
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
		await openMenu();
		await userEvent.click(
			await screen.findByRole("menuitem", {
				name: "Liên kết chat.hoandat.com",
			}),
		);
		const dialog = await screen.findByRole("dialog", {
			name: "Liên kết chat.hoandat.com",
		});
		expect(dialog).toHaveTextContent("Gửi /start");
		expect(
			await screen.findByText(
				"Đường dẫn chỉ dùng một lần, hết hạn lúc 10:10 16/10.",
			),
		).toBeInTheDocument();
		await userEvent.click(
			screen.getByRole("button", { name: /^Mở chat\.hoandat\.com/ }),
		);
		expect(open).toHaveBeenCalledWith(
			"https://chat.hoandat.com/b/bot_scribe_bot?start=tok1",
			"_blank",
			"noopener,noreferrer",
		);
	});

	it("re-reads the link when the window gets the focus back", async () => {
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
		expect(menu).toHaveTextContent("Đã liên kết với An Nguyễn");
		expect(menu).toHaveTextContent("Đã liên kết chat lúc 09:30 16/10");
		await userEvent.click(
			await screen.findByRole("menuitem", { name: "Gỡ liên kết chat" }),
		);
		expect(await screen.findByText("Đã gỡ liên kết chat")).toBeInTheDocument();
		expect(calls.map((call) => call.method)).toContain("DELETE");
	});

	it.each([
		["Tài khoản hoandat", "https://auth.hoandat.com/account"],
		["Ứng dụng", "https://auth.hoandat.com/account#account-apps"],
	])("opens '%s' of the IdP in a new tab", async (label, url) => {
		const open = vi.spyOn(window, "open").mockReturnValue(null);
		renderControls();
		await openMenu();
		await userEvent.click(
			screen.getByRole("menuitem", {
				// jsdom puts a space before the sr-only suffix; browsers do not.
				name: new RegExp(`^${label} ?, mở trang hoandat ở thẻ mới$`),
			}),
		);
		expect(open).toHaveBeenCalledWith(url, "_blank", "noopener,noreferrer");
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
