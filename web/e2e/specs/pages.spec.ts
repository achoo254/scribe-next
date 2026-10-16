import { type Page, expect, test } from "@playwright/test";
import { type ApiFixtures, apiFixtures, stubApi } from "../support/api-stubs";
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
 * Every signed-in page in light and dark, on the desktop and the iPhone
 * viewport: page header, cards, list rows and row menus, compared with
 * the localman-next page each one follows.
 */

const FIXTURES: ApiFixtures = apiFixtures({
	managedFiles: [
		{
			id: "f-upload-1",
			name: "phong-van-khach-hang.m4a",
			kind: "upload",
			size: 52_428_800,
			createdAt: "2026-10-16T09:30:00+07:00",
		},
		{
			id: "f-seg-1",
			name: "phong-van-phan-1.m4a",
			kind: "output",
			size: 26_214_400,
			jobId: "job-1",
			createdAt: "2026-10-16T09:40:00+07:00",
		},
		{
			id: "f-seg-2",
			name: "phong-van-phan-2.m4a",
			kind: "output",
			size: 26_214_400,
			jobId: "job-1",
			createdAt: "2026-10-16T09:40:00+07:00",
		},
	],
	transcriptDocs: [DOC_SUMMARY],
	transcriptDetails: { [DOC_KEY]: DOC_DETAIL },
});

/** Nothing wider than the viewport: long names truncate on the iPhone. */
async function noSideScroll(page: Page): Promise<void> {
	expect(
		await page.evaluate(
			() => document.documentElement.scrollWidth <= window.innerWidth,
		),
	).toBe(true);
}

async function snapshot(page: Page, name: string): Promise<void> {
	await fontsReady(page);
	await noSideScroll(page);
	await expect(page).toHaveScreenshot(name, { fullPage: true });
}

for (const scheme of COLOR_SCHEMES) {
	test.describe(`pages in ${scheme}`, () => {
		test.beforeEach(async ({ page }) => {
			await preparePage(page, scheme);
		});

		test("Cắt file after an upload, with the files kept on the server", async ({
			page,
		}) => {
			const api = await stubApi(page, FIXTURES);
			await page.goto("/split");
			await expect(
				page.getByRole("heading", { level: 1, name: "Cắt file" }),
			).toBeVisible();
			await page.getByTestId("split-file-input").setInputFiles({
				name: "phong-van.m4a",
				mimeType: "audio/mp4",
				buffer: Buffer.alloc(1024),
			});
			await expect(page.getByTestId("split-uploaded")).toBeVisible();
			await expect(
				page.getByRole("combobox", { name: "Cắt theo" }),
			).toHaveValue("minutes");
			await expect(page.getByTestId("managed-files")).toContainText(
				"phong-van-khach-hang.m4a",
			);
			await snapshot(page, `split-${scheme}.png`);
			expect(await seriousA11yViolations(page)).toEqual([]);
			expect(api.unexpected).toEqual([]);
		});

		test("Transcript list", async ({ page }) => {
			const api = await stubApi(page, FIXTURES);
			await page.goto("/transcripts");
			await expect(
				page.getByRole("heading", { level: 1, name: "Lời đã chép" }),
			).toBeVisible();
			await expect(
				page.getByTestId(`transcripts-item-${DOC_KEY}`),
			).toBeVisible();
			await expect(page.getByTestId("transcription-quota")).toBeVisible();
			await snapshot(page, `transcripts-${scheme}.png`);
			expect(await seriousA11yViolations(page)).toEqual([]);
			expect(api.unexpected).toEqual([]);
		});

		test("Transcript reader with its actions menu", async ({ page }) => {
			const api = await stubApi(page, FIXTURES);
			await page.goto(`/transcripts/${encodeURIComponent(DOC_KEY)}`);
			await expect(
				page.getByRole("heading", { level: 1, name: "Phỏng vấn khách hàng" }),
			).toBeVisible();
			await expect(page.getByText("Câu đầu của đoạn 2.")).toBeVisible();
			await snapshot(page, `reader-${scheme}.png`);
			expect(await seriousA11yViolations(page)).toEqual([]);

			await page.getByTestId("reader-menu").click();
			const menu = page.getByRole("menu");
			await expect(
				menu.getByRole("menuitem", { name: "Xoá bản chép lời" }),
			).toBeVisible();
			await expect(menu).toHaveScreenshot(`reader-menu-${scheme}.png`);
			expect(
				await seriousA11yViolations(page, '[data-id="reader-menu-content"]'),
			).toEqual([]);
			expect(api.unexpected).toEqual([]);
		});

		test("a recording row's ⋮ menu", async ({ page }) => {
			const api = await stubApi(page, FIXTURES);
			await page.goto("/record");
			const row = page.getByTestId("history-item-f-browser-1");
			await expect(row).toBeVisible();
			await noSideScroll(page);
			await row.getByTestId("history-item-f-browser-1-menu").click();
			const menu = page.getByTestId("history-item-f-browser-1-menu-content");
			await expect(
				menu.getByRole("menuitem", { name: "Tải về" }),
			).toBeVisible();
			await expect(page).toHaveScreenshot(`record-row-menu-${scheme}.png`);
			expect(
				await seriousA11yViolations(
					page,
					'[data-id="history-item-f-browser-1-menu-content"]',
				),
			).toEqual([]);
			expect(api.unexpected).toEqual([]);
		});
	});
}
