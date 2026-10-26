import { expect, test } from "@playwright/test";
import { apiFixtures, stubApi } from "../support/api-stubs";
import {
	COLOR_SCHEMES,
	fontsReady,
	preparePage,
	seriousA11yViolations,
} from "../support/page-helpers";

const GUEST = apiFixtures({ user: null });

/*
 * The landing page against its screenshots, with axe. Where its links lead,
 * its footer and the redirect of a signed-in user are locked by
 * src/routes.test.tsx.
 */

test.describe("landing page", () => {
	for (const scheme of COLOR_SCHEMES) {
		test(`renders in ${scheme} with no serious a11y violation`, async ({
			page,
		}) => {
			await preparePage(page, scheme);
			const api = await stubApi(page, GUEST);
			await page.goto("/");

			await expect(
				page.getByRole("heading", {
					level: 1,
					name: "Nói. Nhận bản chép lời.",
				}),
			).toBeVisible();
			const art = page.getByTestId("landing-hero-art");
			await expect
				.poll(() => art.evaluate((img: HTMLImageElement) => img.naturalWidth))
				.toBeGreaterThan(0);
			await fontsReady(page);
			// No horizontal scroll at any width the projects use.
			expect(
				await page.evaluate(
					() => document.documentElement.scrollWidth <= window.innerWidth,
				),
			).toBe(true);
			await expect(page).toHaveScreenshot(`landing-${scheme}.png`, {
				fullPage: true,
			});
			expect(await seriousA11yViolations(page)).toEqual([]);
			expect(api.unexpected).toEqual([]);
		});
	}
});
