import { expect, test } from "@playwright/test";
import { SIGNED_IN_USER, stubApi } from "../support/api-stubs";
import { IDLE_RECORDER, stubNativeShell } from "../support/capacitor-stub";
import {
	COLOR_SCHEMES,
	fontsReady,
	preparePage,
	seriousA11yViolations,
} from "../support/page-helpers";

/*
 * The iPhone recorder card with "Đánh dấu", against its screenshot. The
 * after-meeting logic is locked by unit tests: minutes, their allowance and
 * sending to chat in src/pages/transcript/transcript-reader.test.tsx,
 * linking chat.hoandat.com in src/layout/account-controls.test.tsx,
 * bookmarks in src/pages/record/record-state.test.tsx and
 * src/features/transcript-docs/bookmark-queue.test.ts.
 */

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
