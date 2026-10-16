import { type Page, expect, test } from "@playwright/test";
import {
	LINKED_CHAT_NAME,
	SIGNED_IN_USER,
	STUB_IDS,
	STUB_MINUTES,
	apiFixtures,
	countOf,
	stubApi,
} from "../support/api-stubs";
import { IDLE_RECORDER, stubNativeShell } from "../support/capacitor-stub";
import {
	COLOR_SCHEMES,
	fontsReady,
	preparePage,
	seriousA11yViolations,
} from "../support/page-helpers";
import { DOC_DETAIL, DOC_KEY } from "../support/transcript-fixtures";

/*
 * After the meeting: minutes ("Tóm tắt"), bookmarks pressed while
 * recording, and sending the minutes to chat.hoandat.com once linked.
 */

const READER = `/transcripts/${encodeURIComponent(DOC_KEY)}`;
const SHARE = `POST /api/transcript-docs/${encodeURIComponent(DOC_KEY)}/share`;
const SUMMARY = `POST /api/transcript-docs/${encodeURIComponent(DOC_KEY)}/summary`;

async function openReader(page: Page): Promise<void> {
	await page.goto(READER);
	await expect(
		page.getByRole("heading", { level: 1, name: "Phỏng vấn khách hàng" }),
	).toBeVisible();
}

test.beforeEach(async ({ page }) => {
	await preparePage(page, "light");
});

test("Tóm tắt makes the minutes, and a time plays the audio from there", async ({
	page,
}) => {
	const api = await stubApi(
		page,
		apiFixtures({ transcriptDetails: { [DOC_KEY]: DOC_DETAIL } }),
	);
	await openReader(page);
	await page.getByTestId("minutes-create").click();
	const minutes = page.getByTestId("minutes");
	await expect(minutes).toContainText(STUB_MINUTES.summary);
	await expect(minutes).toContainText("Người làm: Lan · Hạn: thứ Sáu");
	expect(countOf(api, SUMMARY)).toBe(1);

	// 1:02 of the session is 0:02 into the second segment.
	await minutes.getByRole("button", { name: "Nghe từ 1:02" }).click();
	await expect(page.getByText("Đang ở đoạn 2")).toBeVisible();
	await expect(page.getByTestId("reader-segment-1-line-0")).toBeFocused();
	expect(api.unexpected).toEqual([]);
});

test("a used-up daily allowance says when Tóm tắt opens again", async ({
	page,
}) => {
	const api = await stubApi(
		page,
		apiFixtures({
			transcriptDetails: { [DOC_KEY]: DOC_DETAIL },
			replies: {
				[SUMMARY]: {
					status: 403,
					body: {
						error: "Hôm nay đã dùng 10/10 lượt tóm tắt. Mở lại lúc 0 giờ.",
						code: "SUMMARY_DAILY_LIMIT",
						usedMinutes: 10,
						limitMinutes: 10,
						retryAt: "2026-10-16T17:00:00+00:00",
					},
				},
			},
		}),
	);
	await openReader(page);
	await page.getByTestId("minutes-create").click();
	await expect(page.getByTestId("minutes-error")).toContainText(
		"Hôm nay đã dùng 10/10 lượt tóm tắt. Mở lại lúc 00:00 17/10.",
	);
	expect(api.unexpected).toEqual([]);
});

test("links chat.hoandat.com from the account menu, then sends the minutes", async ({
	page,
}) => {
	const api = await stubApi(
		page,
		apiFixtures({
			transcriptDetails: {
				[DOC_KEY]: { ...DOC_DETAIL, summary: STUB_MINUTES },
			},
		}),
	);
	await openReader(page);
	// Not linked yet: the send button offers the link instead.
	await expect(page.getByTestId("minutes-send")).toHaveText("Liên kết chat");

	await page
		.getByRole("button", { name: `${SIGNED_IN_USER.email} · Tài khoản` })
		.click();
	await page
		.getByRole("menuitem", { name: "Liên kết chat.hoandat.com" })
		.click();
	// The title turns into "Đã liên kết chat" once linked.
	const dialog = page.getByTestId("chat-link-dialog");
	await expect(
		page.getByRole("dialog", { name: "Liên kết chat.hoandat.com" }),
	).toContainText("Gửi /start");
	await expect(dialog.getByTestId("chat-link-expires")).toContainText(
		"hết hạn lúc 10:10 16/10",
	);
	expect(
		await seriousA11yViolations(page, '[data-id="chat-link-dialog"]'),
	).toEqual([]);
	const popup = page.waitForEvent("popup");
	await dialog.getByTestId("chat-link-open").click();
	expect((await popup).url()).toBe(
		"https://chat.hoandat.com/b/bot_scribe_bot?start=e2e-token",
	);

	// The user sends /start and presses "Xác nhận liên kết" in the chat;
	// the dialog polls meanwhile, no focus change needed.
	await expect(dialog).toContainText("Xác nhận liên kết");
	api.linkChat();
	await expect(dialog.getByTestId("chat-link-linked")).toBeVisible();
	await dialog.getByTestId("chat-link-done").click();

	await expect(page.getByTestId("minutes-send")).toHaveText("Gửi sang chat");
	await page.getByTestId("minutes-send").click();
	await expect(page.getByText("Đã gửi biên bản sang chat")).toBeVisible();
	expect(countOf(api, SHARE)).toBe(1);

	// Unlinking turns the button back into "Liên kết chat".
	await page
		.getByRole("button", { name: `${SIGNED_IN_USER.email} · Tài khoản` })
		.click();
	await expect(page.getByTestId("account-menu-chat-name")).toHaveText(
		`Đã liên kết với ${LINKED_CHAT_NAME}`,
	);
	await expect(page.getByTestId("account-menu-chat-since")).toHaveText(
		"Đã liên kết chat lúc 09:55 16/10",
	);
	await page.getByRole("menuitem", { name: "Gỡ liên kết chat" }).click();
	await expect(page.getByTestId("minutes-send")).toHaveText("Liên kết chat");
	expect(countOf(api, "DELETE /api/chat/link")).toBe(1);
	expect(api.unexpected).toEqual([]);
});

test.describe("bookmarks while recording in the browser", () => {
	test.skip(
		({ browserName }) => browserName !== "chromium",
		"Only Chromium has the synthetic microphone (--use-fake-device-for-media-stream)",
	);

	test("Đánh dấu saves the second under the recording's file id", async ({
		page,
	}) => {
		const api = await stubApi(page);
		await page.goto("/record");
		await page.getByTestId("record-toggle").click();
		await expect(page.getByTestId("record-timer")).toBeVisible();
		await page.getByTestId("record-bookmark").click();
		await expect(page.getByTestId("record-bookmarks")).toHaveText(
			"Đã đánh 1 mốc",
		);
		expect(api.bookmarks).toHaveLength(1);
		expect(api.bookmarks[0]?.key).toBe(`f:${STUB_IDS.recordingFile}`);
		expect(api.bookmarks[0]?.at).toBeGreaterThanOrEqual(0);

		await page.getByTestId("record-toggle").click();
		await expect(page.getByTestId("record-toggle")).toHaveText("Bắt đầu ghi");
		expect(api.unexpected).toEqual([]);
	});
});

test.describe("bookmarks while recording on the iPhone", () => {
	test.skip(
		({ isMobile }) => !isMobile,
		"The iOS app shell only exists on the phone viewport",
	);

	for (const scheme of COLOR_SCHEMES) {
		test(`the recorder card with Đánh dấu in ${scheme}`, async ({ page }) => {
			await preparePage(page, scheme);
			await stubNativeShell(page, SIGNED_IN_USER.email, {
				...IDLE_RECORDER,
				state: "recording",
				elapsedS: 125,
				// Swift's position of the kept audio, which bookmarks use.
				positionS: 124.6,
				sessionId: "e2e-session",
			});
			const api = await stubApi(page);
			await page.goto("/");
			await expect(page.getByTestId("native-record-timer")).toBeVisible();
			await page.getByTestId("native-record-bookmark").click();
			await expect(page.getByTestId("native-record-bookmarks")).toHaveText(
				"Đã đánh 1 mốc",
			);
			expect(api.bookmarks).toEqual([
				{
					key: "s:e2e-session",
					at: 124.6,
					id: expect.stringMatching(/^[0-9a-f]{24}$/),
				},
			]);
			await fontsReady(page);
			await expect(page.getByTestId("native-record")).toHaveScreenshot(
				`native-recording-${scheme}.png`,
			);
			expect(await seriousA11yViolations(page)).toEqual([]);
			expect(api.unexpected).toEqual([]);
		});
	}
});
