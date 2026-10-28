import { type Page, expect, test } from "@playwright/test";
import { STUB_MINUTES, apiFixtures, stubApi } from "../support/api-stubs";
import { E2E_ORIGIN } from "../support/e2e-origin";
import {
	fontsReady,
	preparePage,
	seriousA11yViolations,
} from "../support/page-helpers";
import {
	DOC_DETAIL,
	DOC_KEY,
	DOC_SUMMARY,
} from "../support/transcript-fixtures";

/*
 * The signed-in app in English (the shared `hd_lang` choice), on the
 * desktop and the iPhone viewport: the English texts are longer than the
 * Vietnamese ones, so every page must still fit (no sideways scroll), no
 * button or tab may clip its label, and axe must pass (a visible label
 * must be part of the accessible name). The wording is locked by the
 * unit tests; the Vietnamese pages by the other specs.
 */

const FIXTURES = apiFixtures({
	chatLinked: true,
	transcriptDocs: [DOC_SUMMARY],
	transcriptDetails: {
		[DOC_KEY]: {
			...DOC_DETAIL,
			summary: STUB_MINUTES,
			bookmarks: [{ id: "b-1", at: 3, label: "Khách hỏi giá" }],
		},
	},
});

async function speakEnglish(page: Page): Promise<void> {
	await page
		.context()
		.addCookies([{ name: "hd_lang", value: "en", url: E2E_ORIGIN }]);
}

/** Buttons, tabs, links and navs whose labels are wider than their box. */
async function clippedControls(page: Page): Promise<string[]> {
	return page.evaluate(() =>
		[
			...document.querySelectorAll<HTMLElement>(
				'button, [role="tab"], [role="tablist"], a[data-id], nav',
			),
		]
			// Visible ones with a text label; `sr-only` (the skip link) is
			// clipped on purpose, a switch's thumb is not a label.
			.filter(
				(element) =>
					element.offsetParent !== null &&
					!element.classList.contains("sr-only") &&
					element.getAttribute("role") !== "switch" &&
					(element.textContent ?? "").trim() !== "",
			)
			.filter((element) => element.scrollWidth > element.clientWidth + 1)
			.map((element) => element.dataset.id ?? element.textContent ?? ""),
	);
}

async function checkPage(page: Page): Promise<void> {
	await fontsReady(page);
	await expect(page.locator("html")).toHaveAttribute("lang", "en");
	expect(
		await page.evaluate(
			() => document.documentElement.scrollWidth <= window.innerWidth,
		),
	).toBe(true);
	expect(await clippedControls(page)).toEqual([]);
	expect(await seriousA11yViolations(page)).toEqual([]);
}

test.describe("the signed-in app in English", () => {
	test.beforeEach(async ({ page }) => {
		await preparePage(page, "light");
		await speakEnglish(page);
	});

	test("Record with the recording history", async ({ page }) => {
		const api = await stubApi(page, FIXTURES);
		await page.goto("/record");
		await expect(
			page.getByRole("heading", { level: 1, name: "Record" }),
		).toBeVisible();
		await expect(page.getByTestId("history")).toContainText(
			"Recording history",
		);
		await expect(page.getByTestId("usage")).toBeVisible();
		await checkPage(page);
		expect(api.unexpected).toEqual([]);
	});

	test("Split file", async ({ page }) => {
		const api = await stubApi(page, FIXTURES);
		await page.goto("/split");
		await expect(
			page.getByRole("heading", { level: 1, name: "Split file" }),
		).toBeVisible();
		await checkPage(page);
		expect(api.unexpected).toEqual([]);
	});

	test("Transcripts and a transcript with its minutes", async ({ page }) => {
		const api = await stubApi(page, FIXTURES);
		await page.goto("/transcripts");
		await expect(
			page.getByRole("heading", { level: 1, name: "Transcripts" }),
		).toBeVisible();
		await expect(page.getByTestId(`transcripts-item-${DOC_KEY}`)).toBeVisible();
		await checkPage(page);

		await page.goto(`/transcripts/${encodeURIComponent(DOC_KEY)}`);
		await expect(page.getByTestId("minutes-send")).toHaveText("Send to chat");
		await expect(page.getByTestId("reader-mark-b-1")).toHaveText(
			"Bookmark 0:03 · Khách hỏi giá",
		);
		await checkPage(page);
		expect(api.unexpected).toEqual([]);
	});

	test("Search and Ask Scribe", async ({ page }) => {
		const api = await stubApi(page, FIXTURES);
		await page.goto("/search");
		await expect(
			page.getByRole("heading", { level: 1, name: "Search recordings" }),
		).toBeVisible();
		await expect(
			page.getByRole("heading", { name: "Ask Scribe" }),
		).toBeVisible();
		await checkPage(page);
		expect(api.unexpected).toEqual([]);
	});

	test("the privacy policy, translated", async ({ page }) => {
		const api = await stubApi(page, FIXTURES);
		await page.goto("/privacy");
		await expect(
			page.getByRole("heading", { level: 1, name: "Privacy Policy" }),
		).toBeVisible();
		await expect(page.getByTestId("legal-authoritative")).toContainText(
			"The Vietnamese version is the one that applies",
		);
		await checkPage(page);
		expect(api.unexpected).toEqual([]);
	});
});
