import { expect, test } from "@playwright/test";
import {
	STUB_SENTENCE,
	STUB_UPLOADED,
	countOf,
	stubApi,
} from "../support/api-stubs";
import { preparePage } from "../support/page-helpers";

/*
 * "Lịch sử ghi" flows on the record page: a file uploaded from the device
 * joins the history and is transcribed like a recording; one-tap delete
 * with "Hoàn tác"; "Xoá tất cả" behind a confirmation.
 */

test("an uploaded audio file shows in the history and can be transcribed", async ({
	page,
}) => {
	await preparePage(page, "light");
	const api = await stubApi(page);
	await page.goto("/");

	await page.getByTestId("record-upload-input").setInputFiles({
		name: "phong-van.mp3",
		mimeType: "audio/mpeg",
		buffer: Buffer.alloc(64 * 1024, 1),
	});
	await expect(page.getByTestId("record-upload-done")).toContainText(
		STUB_UPLOADED.name,
	);
	const row = page.getByTestId(`history-item-${STUB_UPLOADED.id}`);
	await expect(row).toContainText(STUB_UPLOADED.name);
	await expect(row).toContainText("Tệp tải lên");

	await page
		.getByTestId(`history-item-${STUB_UPLOADED.id}-transcript-start`)
		.click();
	await expect(
		page.getByTestId(`history-item-${STUB_UPLOADED.id}-transcript-status`),
	).toContainText("Đã chép lời");
	expect(api.transcribed).toEqual([[STUB_UPLOADED.id]]);
	// The list re-reads after the request; open the text once it has settled.
	await expect(async () => {
		const toggle = page.getByTestId(
			`history-item-${STUB_UPLOADED.id}-transcript-toggle`,
		);
		if ((await toggle.getAttribute("aria-expanded")) !== "true") {
			await toggle.click();
		}
		await expect(row).toContainText(STUB_SENTENCE, { timeout: 1_000 });
	}).toPass();
	expect(countOf(api, "POST /api/upload/finalize")).toBe(1);
	expect(api.unexpected).toEqual([]);
});

test("one tap deletes a recording, “Hoàn tác” brings it back, the toast's end deletes it", async ({
	page,
}) => {
	await preparePage(page, "light");
	const api = await stubApi(page);
	await page.goto("/");
	const row = page.getByTestId("history-item-f-browser-1");
	await expect(row).toBeVisible();

	await page.getByTestId("history-item-f-browser-1-delete").click();
	await expect(row).toBeHidden();
	await page.getByRole("button", { name: "Hoàn tác" }).click();
	await expect(row).toBeVisible();
	expect(api.deletes).toEqual([]);

	await page.getByTestId("history-item-f-browser-1-delete").click();
	await expect(row).toBeHidden();
	await expect(page.getByRole("button", { name: "Hoàn tác" })).toBeVisible();
	expect(api.deletes).toEqual([]);
	// Nothing is sent before the 5 seconds of the toast are over.
	await expect
		.poll(() => api.deletes, { timeout: 10_000 })
		.toEqual(["/api/files/f-browser-1"]);
	await expect(row).toBeHidden();
	expect(api.unexpected).toEqual([]);
});

test("“Xoá tất cả” states the count and size, then empties the history", async ({
	page,
}) => {
	await preparePage(page, "light");
	const api = await stubApi(page);
	await page.goto("/");
	await expect(page.getByTestId("history")).toContainText(
		"Họp nhóm sáng thứ Năm.webm",
	);

	await page.getByTestId("history-delete-all").click();
	const dialog = page.getByRole("dialog", { name: "Xoá toàn bộ lịch sử ghi" });
	await expect(dialog).toContainText("Xoá 3 tệp, tổng 15.9 MB?");
	await dialog.getByRole("button", { name: "Xoá tất cả" }).click();

	await expect(page.getByText("Đã xoá 3 tệp").first()).toBeVisible();
	await expect(page.getByTestId("history-empty")).toBeVisible();
	expect(api.deletes).toEqual(["/api/files"]);
	expect(api.unexpected).toEqual([]);
});
