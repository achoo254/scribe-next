import { type Page, expect, test } from "@playwright/test";
import { SIGNED_IN_USER, apiFixtures, stubApi } from "../support/api-stubs";
import { fontsReady } from "../support/page-helpers";
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

test("the page carries every production security header", async ({ page }) => {
	await stubApi(page, apiFixtures({ user: null }));
	const response = await page.goto("/login");
	const sent = response?.headers() ?? {};
	for (const [name, value] of Object.entries(HEADERS)) {
		expect(sent[name.toLowerCase()], name).toContain(value);
	}
});

test("landing, sign-in and public pages load with no CSP violation", async ({
	page,
}) => {
	await watchViolations(page);
	await stubApi(page, apiFixtures({ user: null }));
	for (const url of ["/", "/login", "/privacy", "/terms"]) {
		await page.goto(url);
		await expect(page.locator("#app")).not.toBeEmpty();
		await fontsReady(page);
	}
	expect(await violations(page)).toEqual([]);
});

test("the landing's hero art is a file this origin serves (img-src 'self')", async ({
	page,
}) => {
	await watchViolations(page);
	await stubApi(page, apiFixtures({ user: null }));
	await page.goto("/");
	const art = page.getByTestId("landing-hero-art");
	await expect(art).toBeVisible();
	await expect
		.poll(() => art.evaluate((img: HTMLImageElement) => img.naturalWidth))
		.toBeGreaterThan(0);
	expect(await art.getAttribute("src")).toMatch(/^\/assets\//);
	expect(await violations(page)).toEqual([]);
});

test("each signed-in page and the account menu load with no CSP violation", async ({
	page,
}) => {
	await watchViolations(page);
	const api = await stubApi(page);
	for (const url of ["/record", "/split", "/transcripts", "/search"]) {
		await page.goto(url);
		await expect(page.getByTestId("usage")).toBeVisible();
		await fontsReady(page);
	}
	// Radix menus insert a <style> element (style-src).
	await page
		.getByRole("button", { name: `${SIGNED_IN_USER.email} · Tài khoản` })
		.click();
	await expect(page.getByRole("menu")).toBeVisible();
	expect(await violations(page)).toEqual([]);
	expect(api.unexpected).toEqual([]);
});
