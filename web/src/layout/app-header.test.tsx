import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { describe, expect, it, vi } from "vitest";
import type { CurrentUser } from "../auth/auth-api";

vi.mock("../auth/auth-context", () => ({
	useAuth: () => ({ logout: async () => undefined }),
}));

const { AppHeader } = await import("./app-header");
const { IDLE_ACTIVITY } = await import("./session-activity");

function user(canTranscribe: boolean): CurrentUser {
	return { email: "minh.anh@example.com", canTranscribe };
}

function renderHeader(
	canTranscribe = true,
	tab: "record" | "split" = "record",
) {
	const onTabChange = vi.fn();
	const view = render(
		<MemoryRouter>
			<AppHeader
				user={user(canTranscribe)}
				tab={tab}
				onTabChange={onTabChange}
				activity={IDLE_ACTIVITY}
				beforeSignOut={async () => undefined}
			/>
		</MemoryRouter>,
	);
	return { onTabChange, ...view };
}

/** The wide nav and the compact row under the bar (one shows per width). */
function navs() {
	return screen.getAllByRole("navigation", { name: "Chức năng" });
}

describe("AppHeader", () => {
	it("links the wordmark home and goes back to Ghi âm", async () => {
		const { onTabChange } = renderHeader(true, "split");
		const home = screen.getByRole("link", { name: "Scribe, về trang Ghi âm" });
		expect(home).toHaveAttribute("href", "/");
		await userEvent.click(home);
		expect(onTabChange).toHaveBeenCalledWith("record");
	});

	it("marks the current page in both navs", () => {
		renderHeader(true, "split");
		expect(navs()).toHaveLength(2);
		for (const nav of navs()) {
			const current = within(nav).getByRole("button", { name: "Cắt file" });
			expect(current).toHaveAttribute("aria-current", "page");
			expect(
				within(nav).getByRole("button", { name: "Ghi âm" }),
			).not.toHaveAttribute("aria-current");
		}
	});

	it("switches pages from the nav", async () => {
		const { onTabChange } = renderHeader();
		const [wide] = navs();
		if (!wide) throw new Error("no nav");
		await userEvent.click(
			within(wide).getByRole("button", { name: "Transcript" }),
		);
		expect(onTabChange).toHaveBeenCalledWith("transcript");
	});

	it("hides Transcript from users who may not transcribe", () => {
		renderHeader(false);
		for (const nav of navs()) {
			expect(within(nav).getAllByRole("button")).toHaveLength(2);
			expect(
				within(nav).queryByRole("button", { name: "Transcript" }),
			).toBeNull();
		}
	});

	it("puts the account menu on the right and no sign-out button in the bar", () => {
		renderHeader();
		expect(
			screen.getByRole("button", { name: "minh.anh@example.com · Tài khoản" }),
		).toBeVisible();
		expect(screen.queryByRole("button", { name: "Đăng xuất" })).toBeNull();
		expect(screen.queryByRole("group", { name: "Giao diện" })).toBeNull();
	});
});
