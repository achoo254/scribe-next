import { type Page, expect, test } from "@playwright/test";
import type {
	AskReply,
	SearchReply,
} from "../../src/features/search/search-api";
import { apiFixtures, stubApi } from "../support/api-stubs";
import {
	COLOR_SCHEMES,
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
 * "Kho bản ghi" against its screenshots: search results and a "Hỏi Scribe"
 * answer, whose source opens the reader at its second. Search without
 * Vietnamese marks, the marked match, the answer's sources and the daily
 * allowance are locked by src/features/search/normalize.test.ts,
 * src/pages/search/search-page.test.tsx and src/pages/search/ask-card.test.tsx.
 */

const SEARCH: SearchReply = {
	results: [
		{
			key: DOC_KEY,
			title: DOC_SUMMARY.title,
			createdAt: DOC_SUMMARY.createdAt ?? null,
			fileId: "f-iphone-1",
			at: 62,
			text: "Câu đầu của đoạn 2.",
			before: "Câu đầu của đoạn 1.",
			after: null,
		},
	],
	total: 1,
};

const ANSWER: AskReply = {
	answer: "Đoạn 2 mở đầu bằng câu chào của khách [1].",
	citations: [
		{
			n: 1,
			key: DOC_KEY,
			title: DOC_SUMMARY.title,
			createdAt: DOC_SUMMARY.createdAt ?? null,
			at: 62,
			text: "Câu đầu của đoạn 2.",
		},
	],
};

const FIXTURES = apiFixtures({
	transcriptDocs: [DOC_SUMMARY],
	transcriptDetails: { [DOC_KEY]: DOC_DETAIL },
	search: SEARCH,
	answer: ANSWER,
});

async function askScribe(page: Page, question: string): Promise<void> {
	await page.getByTestId("ask-input").fill(question);
	await page.getByTestId("ask-submit").click();
}

async function expectSecondSegmentPlaying(page: Page): Promise<void> {
	await expect(page).toHaveURL(/\/transcripts\/s%3Aa1b2c3d4-e5f6\?t=62$/);
	await expect(page.getByText("Đang ở đoạn 2")).toBeVisible();
	await expect(page.getByTestId("reader-segment-1-line-0")).toBeFocused();
}

test.beforeEach(async ({ page }) => {
	await preparePage(page, "light");
});

for (const scheme of COLOR_SCHEMES) {
	test(`the search page with results and an answer in ${scheme}`, async ({
		page,
	}) => {
		await preparePage(page, scheme);
		const api = await stubApi(page, FIXTURES);
		await page.goto("/search?q=c%C3%A2u%20%C4%91%E1%BA%A7u");
		await expect(page.getByTestId(`search-result-${DOC_KEY}-62`)).toBeVisible();
		await askScribe(page, "Đoạn 2 mở đầu thế nào?");
		await expect(page.getByTestId("ask-entry-1-source-1")).toBeVisible();
		// Every transcript: no key in the question.
		expect(api.questions).toEqual([
			{ question: "Đoạn 2 mở đầu thế nào?", key: null },
		]);
		// Asking scrolled the field into view: shoot the page from its top.
		await page.getByTestId("ask-input").blur();
		await page.evaluate(() => window.scrollTo(0, 0));
		await fontsReady(page);
		expect(
			await page.evaluate(
				() => document.documentElement.scrollWidth <= window.innerWidth,
			),
		).toBe(true);
		await expect(page).toHaveScreenshot(`search-${scheme}.png`, {
			fullPage: true,
		});
		expect(await seriousA11yViolations(page)).toEqual([]);

		// A source opens the reader at its second.
		await page.getByTestId("ask-entry-1-source-1").click();
		await expectSecondSegmentPlaying(page);
		expect(api.unexpected).toEqual([]);
	});
}
