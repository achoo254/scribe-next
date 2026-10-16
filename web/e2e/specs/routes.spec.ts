import { type Page, expect, test } from "@playwright/test";
import {
	SIGNED_IN_USER,
	STUB_IDS,
	apiFixtures,
	countOf,
	stubApi,
} from "../support/api-stubs";
import {
	DOC_DETAIL,
	DOC_KEY,
	DOC_PATH,
	DOC_SUMMARY,
} from "../support/transcript-fixtures";

/*
 * Pages have URLs and unmount when left; the recording, the "Cắt file"
 * upload and its job live above the routes and must not notice.
 */

function nav(page: Page) {
	// Role queries skip the nav hidden at this width (wide or compact row).
	return page.getByRole("navigation", { name: "Chức năng" });
}

async function goTo(page: Page, label: string, path: RegExp): Promise<void> {
	await nav(page).getByRole("link", { name: label }).click();
	await expect(page).toHaveURL(path);
}

test.describe("a browser recording across route changes", () => {
	test.skip(
		({ browserName }) => browserName !== "chromium",
		"Only Chromium has the synthetic microphone (--use-fake-device-for-media-stream)",
	);

	test("one session, contiguous chunks, one finalize", async ({ page }) => {
		await page.emulateMedia({ reducedMotion: "reduce" });
		const api = await stubApi(page);
		await page.goto("/record");
		await page.getByTestId("record-toggle").click();
		await expect(page.getByTestId("record-timer")).toBeVisible();
		await expect.poll(() => api.recordingChunks.length).toBeGreaterThan(1);

		await goTo(page, "Cắt file", /\/split$/);
		await expect(page.getByTestId("split")).toBeVisible();
		await goTo(page, "Lời đã chép", /\/transcripts$/);
		await expect(page.getByTestId("transcripts")).toBeVisible();
		await goTo(page, "Ghi âm", /\/record$/);

		// Same recording, still running and still uploading.
		await expect(page.getByTestId("record-timer")).toBeVisible();
		await expect(page.getByTestId("record-toggle")).toHaveText("Dừng ghi");
		const sent = api.recordingChunks.length;
		await expect.poll(() => api.recordingChunks.length).toBeGreaterThan(sent);

		await page.getByTestId("record-toggle").click();
		await expect(page.getByTestId("record-toggle")).toHaveText("Bắt đầu ghi");
		await expect
			.poll(() => countOf(api, "POST /api/recordings/finalize"))
			.toBe(1);

		expect(countOf(api, "POST /api/recordings/init")).toBe(1);
		const chunks = api.recordingChunks;
		expect(new Set(chunks.map((chunk) => chunk.session))).toEqual(
			new Set([STUB_IDS.recordingSession]),
		);
		// Each chunk starts where the previous one ended: none lost, none twice.
		let expected = 0;
		for (const chunk of chunks) {
			expect(chunk.size).toBeGreaterThan(0);
			expect(chunk.offset).toBe(expected);
			expected += chunk.size;
		}
		await expect(page.getByTestId("record-error")).toHaveCount(0);
		expect(api.unexpected).toEqual([]);
	});
});

const MIB = 1024 * 1024;

test("the Cắt file upload and its job survive leaving the page", async ({
	page,
}) => {
	const api = await stubApi(page);
	// The second of three 5 MB chunks waits until the spec lets it through.
	let release = (): void => undefined;
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	let chunks = 0;
	await page.route(
		(url) => url.pathname === "/api/upload/chunk",
		async (route) => {
			chunks += 1;
			if (chunks === 2) await held;
			await route.fallback();
		},
	);

	await page.goto("/split");
	await page.getByTestId("split-file-input").setInputFiles({
		name: "phong-van.m4a",
		mimeType: "audio/mp4",
		buffer: Buffer.alloc(12 * MIB),
	});
	await expect(page.getByText("Đang tải lên… 41%")).toBeVisible();

	await goTo(page, "Ghi âm", /\/record$/);
	await expect(page.getByTestId("record")).toBeVisible();
	await goTo(page, "Cắt file", /\/split$/);
	await expect(page.getByText("Đang tải lên… 41%")).toBeVisible();
	await expect(page.getByText("phong-van.m4a")).toBeVisible();

	// The rest arrives while the user is elsewhere.
	await goTo(page, "Ghi âm", /\/record$/);
	release();
	await expect.poll(() => countOf(api, "POST /api/upload/finalize")).toBe(1);
	await goTo(page, "Cắt file", /\/split$/);
	await expect(page.getByTestId("split-uploaded")).toContainText("12:34");

	await page.getByTestId("split-start").click();
	await goTo(page, "Ghi âm", /\/record$/);
	await expect
		.poll(() => countOf(api, `GET /api/jobs/${STUB_IDS.job}`))
		.toBeGreaterThan(0);
	await goTo(page, "Cắt file", /\/split$/);
	await expect(page.getByTestId("split-result")).toContainText(
		"Kết quả (2 đoạn)",
	);
	expect(countOf(api, "POST /api/upload/init")).toBe(1);
	expect(countOf(api, "POST /api/upload/chunk")).toBe(3);
	expect(countOf(api, "POST /api/jobs")).toBe(1);
	expect(api.unexpected).toEqual([]);
});

test("nav, browser history and the reader's back button follow the URL", async ({
	page,
}) => {
	const api = await stubApi(
		page,
		apiFixtures({
			transcriptDocs: [DOC_SUMMARY],
			transcriptDetails: { [DOC_KEY]: DOC_DETAIL },
		}),
	);
	await page.goto("/");
	await expect(page).toHaveURL(/\/record$/);

	await goTo(page, "Cắt file", /\/split$/);
	await goTo(page, "Lời đã chép", /\/transcripts$/);
	await page.goBack();
	await expect(page).toHaveURL(/\/split$/);
	await expect(page.getByTestId("split")).toBeVisible();
	await expect(
		nav(page).getByRole("link", { name: "Cắt file" }),
	).toHaveAttribute("aria-current", "page");
	await page.goForward();
	await expect(page).toHaveURL(/\/transcripts$/);

	await page.getByTestId(`transcripts-item-${DOC_KEY}`).click();
	await expect(page).toHaveURL(DOC_PATH);
	const title = page.getByRole("heading", { name: "Phỏng vấn khách hàng" });
	await expect(title).toBeVisible();
	await page.getByTestId("reader-back").click();
	await expect(page).toHaveURL(/\/transcripts$/);
	await page.goBack();
	await expect(page).toHaveURL(DOC_PATH);
	await expect(title).toBeVisible();
	// A deep link (reload, shared URL) opens the same transcript.
	await page.reload();
	await expect(title).toBeVisible();

	// Unsaved input: leaving asks first, by nav and by history alike.
	await page.getByTestId("reader-rename").click();
	await goTo(page, "Ghi âm", DOC_PATH);
	const leave = page.getByRole("dialog", { name: "Rời trang" });
	await expect(leave).toContainText("Phần đang sửa chưa được lưu sẽ mất.");
	await leave.getByRole("button", { name: "Huỷ" }).click();
	await expect(leave).toBeHidden();
	await expect(page).toHaveURL(DOC_PATH);
	await expect(page.getByTestId("reader-title-input")).toBeVisible();
	await page.goBack();
	await leave.getByRole("button", { name: "Vẫn rời trang" }).click();
	await expect(page).toHaveURL(/\/transcripts$/);
	await expect(page.getByTestId("transcripts")).toBeVisible();
	expect(api.unexpected).toEqual([]);
});

test("no Transcript page for users who may not transcribe", async ({
	page,
}) => {
	const api = await stubApi(
		page,
		apiFixtures({ user: { ...SIGNED_IN_USER, canTranscribe: false } }),
	);
	await page.goto(`/transcripts/${encodeURIComponent(DOC_KEY)}`);
	await expect(page).toHaveURL(/\/record$/);
	await expect(nav(page).getByRole("link")).toHaveText(["Ghi âm", "Cắt file"]);
	await page.goto("/transcripts");
	await expect(page).toHaveURL(/\/record$/);
	expect(api.unexpected).toEqual([]);
});
