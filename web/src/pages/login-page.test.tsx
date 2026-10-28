import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { RouterProvider, createMemoryRouter } from "react-router";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { TestProviders } from "../test/test-providers";
import { LoginPage } from "./login-page";

const auth = vi.hoisted(() => ({ login: vi.fn(async () => undefined) }));

vi.mock("../auth/auth-context", () => ({
	useAuth: () => ({ login: auth.login, accountDeleted: undefined }),
}));

function renderAt(url: string) {
	const router = createMemoryRouter(
		[
			{ path: "/login", element: <LoginPage /> },
			{ path: "/", element: <p>home</p> },
		],
		{ initialEntries: [url] },
	);
	render(
		<TestProviders>
			<RouterProvider router={router} />
		</TestProviders>,
	);
	return router;
}

/** The element with `data-id`, the app's test id. */
function byDataId(id: string): HTMLElement {
	const element = document.querySelector<HTMLElement>(`[data-id="${id}"]`);
	if (!element) throw new Error(`no [data-id="${id}"]`);
	return element;
}

const footer = (): HTMLElement => byDataId("public-footer");

/** The sign-in card, once the page has rendered. */
async function signInCard(): Promise<HTMLElement> {
	await screen.findByRole("heading", { level: 1 });
	return byDataId("sign-in-card");
}

beforeEach(() => {
	auth.login.mockClear();
});

/*
 * /login is the shared sign-in card (@achoo254/app-shell) in the shared
 * frame; a web sign-in the backend refused comes back to /login?error=<code>
 * and the card says why (the wording per code is locked by
 * auth/login-error.test.ts).
 */
describe("LoginPage", () => {
	it("is the shared card: one call to action to the backend sign-in", async () => {
		renderAt("/login");
		const card = await signInCard();
		expect(
			within(card).getByRole("heading", { level: 1, name: "Đăng nhập" }),
		).toBeVisible();
		const cta = within(card).getByRole("link", {
			name: "Đăng nhập bằng hoandat.com",
		});
		expect(cta).toHaveAttribute("href", "/api/auth/login");
		expect(within(card).getAllByRole("link")).toHaveLength(1);
		expect(byDataId("public-sign-in").getAttribute("aria-current")).toBe(
			"page",
		);
		expect(screen.queryByRole("alert")).toBeNull();
		expect(document.documentElement.lang).toBe("vi");
	});

	it("signs in through the app's sign-in (system browser in the native apps)", async () => {
		const router = renderAt("/login");
		await userEvent.click(
			await screen.findByRole("link", { name: "Đăng nhập bằng hoandat.com" }),
		);
		expect(auth.login).toHaveBeenCalledTimes(1);
		expect(router.state.location.pathname).toBe("/");
	});

	it("says why a sign-in came back with ?error=", async () => {
		renderAt("/login?error=forbidden");
		expect(await screen.findByRole("alert")).toHaveTextContent(
			"Tài khoản này chưa được phép dùng Scribe.",
		);
	});

	it("has the shared language and theme switches in its footer", async () => {
		renderAt("/login?error=forbidden");
		await signInCard();
		const switches = within(footer());

		await userEvent.click(switches.getByRole("button", { name: "English" }));
		expect(document.cookie).toContain("hd_lang=en");
		expect(
			await screen.findByRole("heading", { level: 1, name: "Sign in" }),
		).toBeVisible();
		expect(
			screen.getByRole("link", { name: "Sign in with hoandat.com" }),
		).toBeVisible();
		expect(screen.getByRole("alert")).toHaveTextContent(
			"This account may not use Scribe yet.",
		);
		expect(document.documentElement.lang).toBe("en");

		await userEvent.click(switches.getByRole("button", { name: "Dark" }));
		expect(document.cookie).toContain("hd_theme=dark");
		expect(document.documentElement.dataset.theme).toBe("dark");
	});

	it("speaks the language chosen in another hoandat.com app", async () => {
		document.cookie = "hd_lang=en; Path=/";
		renderAt("/login");
		expect(
			await screen.findByRole("heading", { level: 1, name: "Sign in" }),
		).toBeVisible();
		expect(
			within(footer()).getByRole("button", { name: "English" }),
		).toHaveAttribute("aria-pressed", "true");
	});
});
