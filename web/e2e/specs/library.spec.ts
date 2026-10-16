import { type Page, expect, test } from "@playwright/test";
import type {
	AskReply,
	SearchReply,
} from "../../src/features/search/search-api";
import { apiFixtures, countOf, stubApi } from "../support/api-stubs";
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
 * "Kho bản ghi": searching every transcript (typed with or without
 * Vietnamese marks) and "Hỏi Scribe", whose sources play the transcript
 * from their second.
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

test("search from Lời đã chép without marks, then open the sentence at its second", async ({
	page,
}) => {
	const api = await stubApi(page, FIXTURES);
	await page.goto("/transcripts");
	await page.getByTestId("search-entry-input").fill("cau dau");
	await page.getByTestId("search-entry-submit").click();
	await expect(page).toHaveURL(/\/search\?q=cau\+dau$/);
	const result = page.getByTestId(`search-result-${DOC_KEY}-62`);
	await expect(result).toBeVisible();
	// The part typed without marks is marked in the sentence with marks.
	await expect(result.locator("mark")).toHaveText("Câu đầu");
	// React's development double mount may cancel and repeat the first request.
	expect(api.searches.length).toBeGreaterThan(0);
	expect(api.searches.every((q) => q === "cau dau")).toBe(true);

	await result.click();
	await expectSecondSegmentPlaying(page);
	expect(api.unexpected).toEqual([]);
});

test("Hỏi Scribe on one transcript: a source jumps to its sentence", async ({
	page,
}) => {
	const api = await stubApi(page, FIXTURES);
	await page.goto(`/transcripts/${encodeURIComponent(DOC_KEY)}`);
	await askScribe(page, "Đoạn 2 mở đầu thế nào?");
	const answer = page.getByTestId("ask-entry-1-answer");
	await expect(answer).toContainText("Đoạn 2 mở đầu bằng câu chào của khách");
	expect(api.questions).toEqual([
		{ question: "Đoạn 2 mở đầu thế nào?", key: DOC_KEY },
	]);
	await page.getByTestId("ask-entry-1-cite-1").click();
	await expect(page.getByText("Đang ở đoạn 2")).toBeVisible();
	await expect(page.getByTestId("reader-segment-1-line-0")).toBeFocused();
	expect(api.unexpected).toEqual([]);
});

test("a used-up daily allowance says when Hỏi Scribe opens again", async ({
	page,
}) => {
	const api = await stubApi(page, {
		...FIXTURES,
		replies: {
			"POST /api/ask": {
				status: 403,
				body: {
					error: "Hôm nay đã dùng 30/30 lượt hỏi. Mở lại lúc 0 giờ.",
					code: "ASK_DAILY_LIMIT",
					usedMinutes: 30,
					limitMinutes: 30,
					retryAt: "2026-10-16T17:00:00+00:00",
				},
			},
		},
	});
	await page.goto("/search");
	await askScribe(page, "Ai gửi báo giá?");
	await expect(page.getByTestId("ask-entry-1-error")).toContainText(
		"Hôm nay đã dùng 30/30 lượt hỏi. Mở lại lúc 00:00 17/10.",
	);
	expect(countOf(api, "POST /api/ask")).toBe(1);
	expect(api.unexpected).toEqual([]);
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
