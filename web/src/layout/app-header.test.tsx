import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, useLocation } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { CurrentUser } from "../auth/auth-api";
import { accountTrigger } from "../test/account-menu";
import { fakeFetch, jsonResponse } from "../test/fake-fetch";
import { TestProviders } from "../test/test-providers";

vi.mock("../auth/auth-context", () => ({
	useAuth: () => ({ logout: async () => undefined }),
}));

const { AppHeader } = await import("./app-header");

function user(canTranscribe: boolean): CurrentUser {
	return { email: "minh.anh@example.com", canTranscribe };
}

function Location() {
	return <output data-testid="location">{useLocation().pathname}</output>;
}

function renderHeader(canTranscribe = true, path = "/record") {
	return render(
		<TestProviders>
			<MemoryRouter initialEntries={[path]}>
				<AppHeader user={user(canTranscribe)} />
				<Location />
			</MemoryRouter>
		</TestProviders>,
	);
}

function location(): string | null {
	return screen.getByTestId("location").textContent;
}

/** The wide nav and the compact row under the bar (one shows per width). */
function navs() {
	return screen.getAllByRole("navigation", { name: "Chức năng" });
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
});

describe("AppHeader", () => {
	it("puts the app switcher right before the account menu", () => {
		renderHeader();
		const switcher = screen.getByRole("button", { name: "Chuyển ứng dụng" });
		const account = accountTrigger();
		expect(
			switcher.compareDocumentPosition(account) &
				Node.DOCUMENT_POSITION_FOLLOWING,
		).toBeTruthy();
	});

	it("links the wordmark to Ghi âm", async () => {
		renderHeader(true, "/split");
		const home = screen.getByRole("link", { name: "Scribe, về trang Ghi âm" });
		expect(home).toHaveAttribute("href", "/record");
		await userEvent.click(home);
		expect(location()).toBe("/record");
	});

	it("marks the page of the URL in both navs, also under /transcripts/:key", () => {
		renderHeader(true, "/transcripts/s%3Aaaaaaaaa-1111");
		expect(navs()).toHaveLength(2);
		for (const nav of navs()) {
			expect(
				within(nav).getByRole("link", { name: "Lời đã chép" }),
			).toHaveAttribute("aria-current", "page");
			expect(
				within(nav).getByRole("link", { name: "Ghi âm" }),
			).not.toHaveAttribute("aria-current");
		}
	});

	it("changes the URL from the nav", async () => {
		renderHeader();
		const [wide] = navs();
		if (!wide) throw new Error("no nav");
		await userEvent.click(within(wide).getByRole("link", { name: "Cắt file" }));
		expect(location()).toBe("/split");
		expect(
			within(wide).getByRole("link", { name: "Cắt file" }),
		).toHaveAttribute("aria-current", "page");
		await userEvent.click(
			within(wide).getByRole("link", { name: "Lời đã chép" }),
		);
		expect(location()).toBe("/transcripts");
	});

	it("hides Transcript from users who may not transcribe", () => {
		renderHeader(false);
		for (const nav of navs()) {
			expect(within(nav).getAllByRole("link")).toHaveLength(2);
			expect(
				within(nav).queryByRole("link", { name: "Lời đã chép" }),
			).toBeNull();
		}
	});

	it("puts the account menu on the right and no sign-out button in the bar", () => {
		renderHeader();
		expect(accountTrigger()).toHaveAttribute(
			"aria-label",
			"minh.anh@example.com · Tài khoản",
		);
		// Sign-out sits in the closed account menu, not in the bar.
		const signOut = screen.getByRole("button", { name: "Đăng xuất" });
		expect(signOut.closest("details")).not.toHaveAttribute("open");
		expect(screen.queryByRole("group", { name: "Giao diện" })).toBeNull();
	});
});
