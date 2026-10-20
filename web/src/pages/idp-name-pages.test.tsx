import { act, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { loadIdpName } from "../lib/idp-name";
import { fakeFetch, jsonResponse } from "../test/fake-fetch";
import { renderWithProviders } from "../test/render-with-providers";
import { LegalPage } from "./legal-page";
import { LoginPage } from "./login-page";

vi.mock("../auth/auth-context", () => ({
	useAuth: () => ({ login: vi.fn(), accountDeleted: "done" }),
}));
// No support email (web/.env may set one): "Liên hệ" names the account.
vi.mock("../lib/legal", async (importOriginal) => ({
	...(await importOriginal<typeof import("../lib/legal")>()),
	SUPPORT_EMAIL: null,
}));

/*
 * The sign-in page and the legal pages name the account people sign in
 * with by the IdP's tenant name: the default first, then what
 * `GET /api/idp` says (read once per page load, here in the middle).
 */
describe("pages naming the sign-in account", () => {
	it("use the default, then the backend's name", async () => {
		renderWithProviders(<LoginPage />);
		expect(
			await screen.findByRole("button", { name: "Đăng nhập bằng Auth Next" }),
		).toBeVisible();

		const { fetcher } = fakeFetch({
			"GET /api/idp": jsonResponse({ displayName: "Acme" }),
		});
		await act(() => loadIdpName(fetcher));
		expect(
			screen.getByRole("button", { name: "Đăng nhập bằng Acme" }),
		).toBeVisible();
		expect(
			screen.getByText("Ghi âm, cắt file và chép lời bằng tài khoản Acme."),
		).toBeVisible();
		expect(
			screen.getByText("Dùng tài khoản Acme (Google hoặc GitHub)."),
		).toBeVisible();
		expect(
			screen.getByText(/Tài khoản Acme vẫn còn ở auth\.hoandat\.com\./),
		).toBeVisible();
	});

	it("fill the name into the privacy policy and the terms", async () => {
		const { fetcher } = fakeFetch({
			"GET /api/idp": jsonResponse({ displayName: "Acme" }),
		});
		await loadIdpName(fetcher);

		const privacy = renderWithProviders(<LegalPage doc="privacy" />);
		const article = await screen.findByRole("article");
		expect(article).toHaveTextContent("Acme (auth.hoandat.com): đăng nhập.");
		expect(article).toHaveTextContent("mã định danh tài khoản Acme");
		expect(article).toHaveTextContent("We collect your Acme account email");
		expect(article).toHaveTextContent(
			"người vận hành Scribe qua tài khoản Acme",
		);
		expect(article).not.toHaveTextContent("{idpName}");
		privacy.unmount();

		renderWithProviders(<LegalPage doc="terms" />);
		const terms = await screen.findByRole("article");
		expect(terms).toHaveTextContent("Bạn đăng nhập bằng tài khoản Acme");
		expect(terms).toHaveTextContent("Sign in with your Acme account");
		expect(terms).not.toHaveTextContent("{idpName}");
	});
});
