import { expect, test } from "@playwright/test";
import { SIGNED_IN_USER, stubApi } from "../support/api-stubs";
import { nativeCalls, stubNativeShell } from "../support/capacitor-stub";
import {
	COLOR_SCHEMES,
	fontsReady,
	preparePage,
	seriousA11yViolations,
} from "../support/page-helpers";

test.describe("record page (browser)", () => {
	for (const scheme of COLOR_SCHEMES) {
		test(`renders in ${scheme} with no serious a11y violation`, async ({
			page,
		}) => {
			await preparePage(page, scheme);
			const api = await stubApi(page);
			await page.goto("/");

			await expect(page.getByTestId("app-shell")).toBeVisible();
			await expect(page.getByRole("tab", { name: "Ghi âm" })).toHaveAttribute(
				"aria-selected",
				"true",
			);
			const history = page.getByTestId("history");
			await expect(history).toContainText("Họp nhóm sáng thứ Năm.webm");
			await expect(page.getByTestId("quota")).toBeVisible();
			await fontsReady(page);
			await expect(page).toHaveScreenshot(`record-${scheme}.png`, {
				fullPage: true,
			});
			expect(await seriousA11yViolations(page)).toEqual([]);
			expect(api.unexpected).toEqual([]);
		});
	}
});

test.describe("record page (iOS app shell)", () => {
	test.skip(
		({ isMobile }) => !isMobile,
		"The iOS app shell only exists on the phone viewport",
	);

	for (const scheme of COLOR_SCHEMES) {
		test(`renders the native recorder in ${scheme}`, async ({ page }) => {
			await preparePage(page, scheme);
			await stubNativeShell(page, SIGNED_IN_USER.email);
			const api = await stubApi(page);
			await page.goto("/");

			await expect(page.getByTestId("native-record")).toBeVisible();
			await expect(page.getByTestId("history")).toContainText(
				"Họp nhóm sáng thứ Năm.webm",
			);
			// On mount the app resumes this account's uploads on the phone.
			await expect
				.poll(async () =>
					(await nativeCalls(page)).map(
						(call) => `${call.plugin}.${call.method}`,
					),
				)
				.toEqual(
					expect.arrayContaining([
						"Recorder.getStatus",
						"Recorder.syncUploads",
					]),
				);
			await fontsReady(page);
			await expect(page).toHaveScreenshot(`record-native-${scheme}.png`, {
				fullPage: true,
			});
			expect(await seriousA11yViolations(page)).toEqual([]);
			expect(api.unexpected).toEqual([]);
		});
	}
});
