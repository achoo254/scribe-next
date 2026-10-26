import { render, screen } from "@testing-library/react";
import { RouterProvider, createMemoryRouter } from "react-router";
import { describe, expect, it, vi } from "vitest";
import { TestProviders } from "../test/test-providers";
import { LoginPage } from "./login-page";

vi.mock("../auth/auth-context", () => ({
	useAuth: () => ({ login: vi.fn(), accountDeleted: undefined }),
}));

function renderAt(url: string): void {
	const router = createMemoryRouter(
		[{ path: "/login", element: <LoginPage /> }],
		{
			initialEntries: [url],
		},
	);
	render(
		<TestProviders>
			<RouterProvider router={router} />
		</TestProviders>,
	);
}

/*
 * A web sign-in the backend refused comes back to /login?error=<code>; the
 * page says why (the wording per code is locked by auth/login-error.test.ts).
 */
describe("LoginPage", () => {
	it("says why a sign-in came back with ?error=", async () => {
		renderAt("/login?error=forbidden");
		expect(await screen.findByRole("alert")).toHaveTextContent(
			"Tài khoản này chưa được phép dùng Scribe.",
		);
	});

	it("shows no error on a plain visit", async () => {
		renderAt("/login");
		expect(
			await screen.findByRole("button", { name: /^Đăng nhập/ }),
		).toBeVisible();
		expect(screen.queryByRole("alert")).toBeNull();
	});
});
