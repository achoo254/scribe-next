import { expect, test } from "@playwright/test";
import { apiFixtures, stubApi } from "../support/api-stubs";
import {
	COLOR_SCHEMES,
	fontsReady,
	preparePage,
	seriousA11yViolations,
} from "../support/page-helpers";

const GUEST = apiFixtures({ user: null });

test.describe("login page", () => {
	for (const scheme of COLOR_SCHEMES) {
		test(`renders in ${scheme} with no serious a11y violation`, async ({
			page,
		}) => {
			await preparePage(page, scheme);
			const api = await stubApi(page, GUEST);
			await page.goto("/login");

			await expect(
				page.getByRole("button", { name: "Đăng nhập" }),
			).toBeVisible();
			await fontsReady(page);
			await expect(page).toHaveScreenshot(`login-${scheme}.png`, {
				fullPage: true,
			});
			expect(await seriousA11yViolations(page)).toEqual([]);
			expect(api.unexpected).toEqual([]);
		});
	}

	test("a visitor sent to another app page lands on /login", async ({
		page,
	}) => {
		await stubApi(page, GUEST);
		await page.goto("/record");
		await expect(page).toHaveURL(/\/login$/);
	});

	test("Đăng nhập hands over to the backend sign-in", async ({ page }) => {
		const api = await stubApi(page, GUEST);
		await page.goto("/login");
		await page.getByTestId("login-submit").click();

		await expect(page).toHaveURL(/\/api\/auth\/login$/);
		expect(api.requests).toContain("GET /api/auth/login");
	});

	test("shows why a sign-in came back with ?error=", async ({ page }) => {
		await stubApi(page, GUEST);
		await page.goto("/login?error=forbidden");
		await expect(page.getByTestId("login-error")).toHaveText(
			"Tài khoản này chưa được phép dùng Scribe.",
		);
	});
});
