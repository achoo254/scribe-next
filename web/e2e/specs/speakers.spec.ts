import { expect, test } from "@playwright/test";
import { apiFixtures, stubApi } from "../support/api-stubs";
import {
	COLOR_SCHEMES,
	fontsReady,
	preparePage,
	seriousA11yViolations,
} from "../support/page-helpers";
import { SPEAKERS_DETAIL, SPEAKERS_KEY } from "../support/transcript-fixtures";

/*
 * Speakers in the reader: a label where the speaker changes, and "Người nói"
 * renames a speaker everywhere in the transcript.
 */

const READER = `/transcripts/${encodeURIComponent(SPEAKERS_KEY)}`;

for (const scheme of COLOR_SCHEMES) {
	test(`labels speakers and renames one in ${scheme}`, async ({ page }) => {
		await preparePage(page, scheme);
		const api = await stubApi(
			page,
			apiFixtures({ transcriptDetails: { [SPEAKERS_KEY]: SPEAKERS_DETAIL } }),
		);
		await page.goto(READER);
		await expect(
			page.getByRole("heading", { level: 1, name: "Họp giao việc" }),
		).toBeVisible();

		const label = (line: number) =>
			page.getByTestId(`reader-segment-0-speaker-${line}`);
		await expect(label(0)).toHaveText("Người 1");
		await expect(label(1)).toHaveCount(0);
		await expect(label(2)).toHaveText("Người 2");
		await expect(label(3)).toHaveText("Người 1");
		await fontsReady(page);
		await page
			.getByTestId("reader-segment-0")
			.evaluate((el) => el.scrollIntoView({ block: "center" }));
		await expect(page.getByTestId("reader-segment-0")).toHaveScreenshot(
			`speakers-reader-${scheme}.png`,
		);

		await page.getByTestId("reader-speakers").click();
		const dialog = page.getByRole("dialog", { name: "Tên người nói" });
		await expect(dialog).toBeVisible();
		await page.getByTestId("speakers-dialog-s0").fill("Chị Lan");
		await page.getByTestId("speakers-dialog-s1").fill("Minh");
		await expect(dialog).toHaveScreenshot(`speakers-dialog-${scheme}.png`);
		expect(await seriousA11yViolations(page)).toEqual([]);

		await page.getByTestId("speakers-dialog-save").click();
		await expect(dialog).toBeHidden();
		expect(api.speakerRenames).toEqual([{ s0: "Chị Lan", s1: "Minh" }]);
		await expect(label(0)).toHaveText("Chị Lan");
		await expect(label(2)).toHaveText("Minh");
		await expect(label(3)).toHaveText("Chị Lan");
		expect(api.unexpected).toEqual([]);
	});
}
