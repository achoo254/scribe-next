import { expect, test } from "@playwright/test";
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
	DOC_TRANSLATION,
} from "../support/transcript-fixtures";

/*
 * "Dịch" in the reader: pick a language, follow the progress, switch the
 * view between Gốc / Bản dịch / Song song, then download the translation.
 */

const READER = `/transcripts/${encodeURIComponent(DOC_KEY)}`;
const TRANSLATE = `POST /api/transcript-docs/${encodeURIComponent(DOC_KEY)}/translation`;

for (const scheme of COLOR_SCHEMES) {
	test(`translates, switches views and downloads the translation in ${scheme}`, async ({
		page,
	}) => {
		await preparePage(page, scheme);
		const api = await stubApi(
			page,
			apiFixtures({
				transcriptDetails: { [DOC_KEY]: DOC_DETAIL },
				translation: DOC_TRANSLATION,
			}),
		);
		await page.goto(READER);
		await expect(
			page.getByRole("heading", { level: 1, name: "Phỏng vấn khách hàng" }),
		).toBeVisible();

		await page.getByTestId("translate-menu").click();
		await page.getByRole("menuitemradio", { name: "Tiếng Anh" }).click();
		await expect(page.getByTestId("translate-running")).toContainText(
			"Đang dịch sang Tiếng Anh…",
		);
		expect(api.translations).toEqual(["en"]);
		expect(countOf(api, TRANSLATE)).toBe(1);

		// Song song: each sentence with its translation; the edited one is marked.
		const first = page.getByTestId("reader-segment-0-line-0");
		const second = page.getByTestId("reader-segment-1-line-0");
		await expect(first).toContainText("Câu đầu của đoạn 1.");
		await expect(first).toContainText("First sentence of segment 1.");
		await expect(first).toContainText("bản dịch cũ");
		await expect(second).toContainText("First sentence of segment 2.");
		await expect(second).not.toContainText("bản dịch cũ");
		await expect(page.getByTestId("translate-view-both")).toHaveAttribute(
			"aria-pressed",
			"true",
		);
		await expect(page.getByTestId("translate-outdated")).toContainText(
			"1 câu đã sửa sau khi dịch.",
		);
		await fontsReady(page);
		await expect(page.getByTestId("translate")).toHaveScreenshot(
			`translation-controls-${scheme}.png`,
		);
		// Clear of the sticky player at the bottom of the reader.
		await page
			.getByTestId("reader-segment-0")
			.evaluate((el) => el.scrollIntoView({ block: "center" }));
		await expect(page.getByTestId("reader-segment-0")).toHaveScreenshot(
			`translation-both-${scheme}.png`,
		);
		expect(await seriousA11yViolations(page)).toEqual([]);

		// Bản dịch: translations only; a tap still plays from that sentence.
		await page.getByTestId("translate-view-translation").click();
		await expect(first).toHaveText(/First sentence of segment 1\./);
		await expect(first).not.toContainText("Câu đầu của đoạn 1.");
		await second.click();
		await expect(page.getByText("Đang ở đoạn 2")).toBeVisible();
		await expect(second).toHaveAttribute("aria-current", "true");

		// The download menu offers the translation for every format.
		await page.getByTestId("reader-download").click();
		const menu = page.getByRole("menu");
		await expect(menu.getByRole("menuitemradio")).toHaveText([
			"Gốc",
			"Tiếng Anh",
		]);
		await menu.getByRole("menuitemradio", { name: "Tiếng Anh" }).click();
		await expect(
			menu.getByRole("menuitemradio", { name: "Tiếng Anh" }),
		).toHaveAttribute("aria-checked", "true");
		await expect(menu).toHaveScreenshot(`translation-download-${scheme}.png`);
		expect(
			await seriousA11yViolations(page, '[data-id="reader-download-content"]'),
		).toEqual([]);
		const exported = page.waitForRequest((request) =>
			request.url().includes("/api/transcripts/export"),
		);
		await menu.getByRole("menuitem", { name: "Markdown" }).click();
		const query = new URL((await exported).url()).searchParams;
		expect(Object.fromEntries(query)).toEqual({
			session: "a1b2c3d4-e5f6",
			format: "md",
			name: "Phỏng vấn khách hàng",
			lang: "en",
		});
		await expect(page.getByTestId("reader")).toBeVisible();
		expect(api.unexpected).toEqual([]);
	});
}
