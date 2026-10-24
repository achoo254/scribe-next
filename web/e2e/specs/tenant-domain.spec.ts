import { expect, test } from "@playwright/test";
import {
	SIGNED_IN_USER,
	apiFixtures,
	countOf,
	stubApi,
} from "../support/api-stubs";
import {
	COLOR_SCHEMES,
	fontsReady,
	preparePage,
	seriousA11yViolations,
} from "../support/page-helpers";

/*
 * Scribe on a domain other than hoandat.com (`GET /api/idp` names the
 * tenant of the host). Before its owner adds an AI key the domain shows
 * "Scribe chưa bật trên domain này" and calls no signed-in API; once on,
 * it signs in at its own IdP and has no chat link (chat.hoandat.com is the
 * primary tenant's).
 */

const ACME = {
	displayName: "Acme Login",
	domain: "acme.vn",
	issuer: "https://auth.acme.vn",
	primary: false,
};

function onAcme(scribe: "enabled" | "disabled", signedIn: boolean) {
	return apiFixtures({
		user: signedIn ? SIGNED_IN_USER : null,
		replies: { "GET /api/idp": { status: 200, body: { ...ACME, scribe } } },
	});
}

test.describe("a domain whose owner has not turned Scribe on", () => {
	for (const scheme of COLOR_SCHEMES) {
		test(`shows the not enabled page in ${scheme} with no serious a11y violation`, async ({
			page,
		}) => {
			await preparePage(page, scheme);
			const api = await stubApi(page, onAcme("disabled", false));
			await page.goto("/");

			const notEnabled = page.getByTestId("tenant-disabled");
			await expect(
				notEnabled.getByRole("heading", {
					level: 1,
					name: "Scribe chưa bật trên domain này",
				}),
			).toBeVisible();
			await expect(page.getByTestId("tenant-disabled-domain")).toHaveText(
				"Domain: acme.vn",
			);
			await expect(page.getByTestId("tenant-disabled-admin")).toHaveAttribute(
				"href",
				"https://auth.acme.vn/admin",
			);
			await fontsReady(page);
			await expect(page).toHaveScreenshot(`tenant-disabled-${scheme}.png`, {
				fullPage: true,
			});
			expect(await seriousA11yViolations(page)).toEqual([]);
			// Neither the session check nor any signed-in API runs.
			expect(countOf(api, "GET /api/me")).toBe(0);
			expect(api.unexpected).toEqual([]);
		});
	}

	test("shows the same page on /login and on a signed-in path", async ({
		page,
	}) => {
		const api = await stubApi(page, onAcme("disabled", false));
		for (const path of ["/login", "/transcripts", "/login?error=not_enabled"]) {
			await page.goto(path);
			await expect(page.getByTestId("tenant-disabled")).toBeVisible();
			await expect(page.getByTestId("login-submit")).toHaveCount(0);
		}
		expect(countOf(api, "GET /api/me")).toBe(0);
	});
});

test.describe("a domain with Scribe on", () => {
	test("signs in by its own account name", async ({ page }) => {
		await stubApi(page, onAcme("enabled", false));
		await page.goto("/login");
		await expect(
			page.getByRole("button", { name: "Đăng nhập bằng Acme Login" }),
		).toBeVisible();
	});

	test("links the account at its IdP and has no chat link", async ({
		page,
	}) => {
		const api = await stubApi(page, onAcme("enabled", true));
		await page.goto("/record");
		await expect(page.getByTestId("app-footer-account")).toHaveAttribute(
			"href",
			"https://auth.acme.vn/account",
		);
		await page
			.getByRole("button", { name: `${SIGNED_IN_USER.email} · Tài khoản` })
			.click();
		await expect(page.getByTestId("account-menu")).toBeVisible();
		await expect(page.getByTestId("account-menu-chat-link")).toHaveCount(0);
		expect(countOf(api, "GET /api/chat/link")).toBe(0);
		expect(api.unexpected).toEqual([]);
	});
});
