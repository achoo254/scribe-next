import { expect, test } from "@playwright/test";
import { apiFixtures, stubApi } from "../support/api-stubs";
import {
	COLOR_SCHEMES,
	fontsReady,
	preparePage,
	seriousA11yViolations,
} from "../support/page-helpers";

const GUEST = apiFixtures({ user: null });

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

	test("Đăng nhập and Bắt đầu miễn phí open the sign-in card", async ({
		page,
	}) => {
		await stubApi(page, GUEST);
		await page.goto("/");
		await page.getByTestId("landing-hero-start").click();
		await expect(page).toHaveURL(/\/login$/);
		await expect(page.getByTestId("login-submit")).toBeVisible();
		await page.goBack();
		await page.getByTestId("public-sign-in").click();
		await expect(page).toHaveURL(/\/login$/);
	});

	test("the footer is the account app's: language, hoandat.com, privacy, terms", async ({
		page,
	}) => {
		await stubApi(page, GUEST);
		await page.goto("/");
		const footer = page.getByTestId("public-footer");
		await expect(
			footer.getByRole("navigation", { name: "Ngôn ngữ" }).getByRole("link"),
		).toHaveText(["Tiếng Việt"]);
		await expect(footer.getByTestId("lang-vi")).toHaveAttribute(
			"aria-current",
			"true",
		);
		await expect(
			footer.getByRole("link", { name: "Một app của hoandat.com" }),
		).toHaveAttribute("href", "https://hoandat.com");
		await expect(
			footer.getByRole("link", { name: "Quyền riêng tư" }),
		).toHaveAttribute("href", "/privacy");
		await expect(
			footer.getByRole("link", { name: "Điều khoản" }),
		).toHaveAttribute("href", "/terms");
	});

	test("Xem tính năng scrolls to the features", async ({ page }) => {
		await stubApi(page, GUEST);
		await page.goto("/");
		await page.getByTestId("landing-hero-features").click();
		await expect(page).toHaveURL(/#features$/);
		await expect(
			page.getByRole("heading", { level: 2, name: "Một chỗ cho cả cuộc họp" }),
		).toBeInViewport();
	});

	test("a signed-in user at / goes straight to the app", async ({ page }) => {
		await stubApi(page);
		await page.goto("/");
		await expect(page).toHaveURL(/\/record$/);
		await expect(page.getByTestId("landing")).toHaveCount(0);
	});
});
