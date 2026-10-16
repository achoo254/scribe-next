import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { CurrentUser } from "../auth/auth-api";
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
) {
	render(
		<MemoryRouter initialEntries={["/"]}>
			<Routes>
				<Route
					path="/"
					element={
						<AccountControls
							user={USER}
							activity={activity}
							beforeSignOut={beforeSignOut}
						/>
					}
				/>
				<Route path="/login" element={<p>trang đăng nhập</p>} />
			</Routes>
		</MemoryRouter>,
	);
	return { beforeSignOut };
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
			"topbar-logout",
		]);
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
