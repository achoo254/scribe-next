import { type Page, expect, test } from "@playwright/test";
import { SIGNED_IN_USER, apiFixtures, stubApi } from "../support/api-stubs";
import {
	SMOKE,
	accountMenuItem,
	accountTrigger,
	fontsReady,
} from "../support/page-helpers";
import { loadSecurityHeaders } from "./security-headers";

/*
 * The production build under the production headers (serve-dist.ts): each
 * page loads with no `securitypolicyviolation`. The other specs run under
 * the same headers too; their violations reach csp-teardown.ts.
 */

const HEADERS = loadSecurityHeaders();

const VIOLATIONS = "__scribeCspViolations";

/** Records violations in the page (init scripts are not subject to CSP). */
async function watchViolations(page: Page): Promise<void> {
	await page.addInitScript((key: string) => {
		const found: string[] = [];
		Object.defineProperty(window, key, { value: found });
		document.addEventListener("securitypolicyviolation", (event) => {
			found.push(
				`${event.effectiveDirective} blocked ${event.blockedURI || "inline"} (${event.sourceFile}:${event.lineNumber})`,
			);
		});
	}, VIOLATIONS);
}

async function violations(page: Page): Promise<string[]> {
	return page.evaluate(
		(key: string) => [...(Reflect.get(window, key) as string[])],
		VIOLATIONS,
	);
}

test(
	"the page carries every production security header",
	SMOKE,
	async ({ page }) => {
		await stubApi(page, apiFixtures({ user: null }));
		const response = await page.goto("/login");
		const sent = response?.headers() ?? {};
		for (const [name, value] of Object.entries(HEADERS)) {
			expect(sent[name.toLowerCase()], name).toContain(value);
		}
	},
);

test(
	"landing, sign-in and public pages load with no CSP violation",
	SMOKE,
	async ({ page }) => {
		await watchViolations(page);
		await stubApi(page, apiFixtures({ user: null }));
		for (const url of ["/", "/login", "/privacy", "/terms"]) {
			await page.goto(url);
			await expect(page.locator("#app")).not.toBeEmpty();
			await fontsReady(page);
		}
		expect(await violations(page)).toEqual([]);
	},
);

test(
	"the shared header's app mark is a file this origin serves (img-src 'self')",
	SMOKE,
	async ({ page }) => {
		await watchViolations(page);
		await stubApi(page, apiFixtures({ user: null }));
		await page.goto("/");
		const mark = page.getByTestId("public-home").locator("img");
		await expect(mark).toBeVisible();
		await expect
			.poll(() => mark.evaluate((img: HTMLImageElement) => img.naturalWidth))
			.toBeGreaterThan(0);
		expect(await mark.getAttribute("src")).toMatch(/^\/assets\//);
		expect(await violations(page)).toEqual([]);
	},
);

test(
	"each signed-in page and the account menu load with no CSP violation",
	SMOKE,
	async ({ page }) => {
		await watchViolations(page);
		const api = await stubApi(page);
		for (const url of ["/record", "/split", "/transcripts", "/search"]) {
			await page.goto(url);
			await expect(page.getByTestId("usage")).toBeVisible();
			await fontsReady(page);
		}
		await accountTrigger(page).click();
		await expect(accountMenuItem(page, "Đăng xuất")).toBeVisible();
		expect(await violations(page)).toEqual([]);
		expect(api.unexpected).toEqual([]);
	},
);

/** A 1×1 PNG, served for the IdP avatar hosts. */
const AVATAR_PNG = Buffer.from(
	"iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
	"base64",
);

for (const picture of [
	"https://lh3.googleusercontent.com/a/scribe-e2e=s96-c",
	"https://avatars.githubusercontent.com/u/1?v=4",
]) {
	test(
		`the account menu shows the IdP picture from ${new URL(picture).host} (img-src)`,
		SMOKE,
		async ({ page }) => {
			await watchViolations(page);
			await stubApi(
				page,
				apiFixtures({
					user: { ...SIGNED_IN_USER, name: "Minh Anh", picture },
				}),
			);
			// After stubApi, whose catch-all aborts every other origin.
			await page.route(picture, (route) =>
				route.fulfill({ contentType: "image/png", body: AVATAR_PNG }),
			);
			await page.goto("/record");
			const avatar = accountTrigger(page).locator("img");
			await expect(avatar).toHaveAttribute("src", picture);
			await expect
				.poll(() =>
					avatar.evaluate((img: HTMLImageElement) => img.naturalWidth),
				)
				.toBeGreaterThan(0);
			expect(await violations(page)).toEqual([]);
		},
	);
}
