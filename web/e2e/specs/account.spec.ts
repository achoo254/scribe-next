import { expect, test } from "@playwright/test";
import { SIGNED_IN_USER, apiFixtures, stubApi } from "../support/api-stubs";
import { nativeCalls, stubNativeShell } from "../support/capacitor-stub";
import {
	COLOR_SCHEMES,
	SMOKE,
	accountMenuItem,
	accountTrigger,
	fontsReady,
	preparePage,
	seriousA11yViolations,
} from "../support/page-helpers";

/*
 * App Store readiness: the privacy policy opens without signing in, and
 * the account can be deleted from the account menu. Desktop runs the
 * browser UI, the iPhone project the iOS app's. Which pages open signed
 * in or out is locked by src/routes.test.tsx, the menu's entries by
 * src/layout/account-controls.test.tsx.
 */

const GUEST = apiFixtures({ user: null });

for (const scheme of COLOR_SCHEMES) {
	test(`privacy policy for a visitor in ${scheme}`, async ({ page }) => {
		await preparePage(page, scheme);
		const api = await stubApi(page, GUEST);
		await page.goto("/privacy");
		const policy = page.getByTestId("legal-privacy");
		await expect(
			policy.getByRole("heading", {
				level: 1,
				name: "Chính sách quyền riêng tư",
			}),
		).toBeVisible();
		await expect(policy).toContainText("Có hiệu lực từ 29/10/2026");
		await expect(
			policy.getByRole("heading", { level: 2, name: "Bên xử lý dữ liệu" }),
		).toBeVisible();
		await expect(
			policy.getByRole("link", { name: "Chính sách của ElevenLabs" }),
		).toHaveAttribute("href", "https://elevenlabs.io/privacy-policy");
		await expect(policy.locator("#english")).toHaveAttribute("lang", "en");
		await expect(page).toHaveURL(/\/privacy$/);
		await fontsReady(page);
		await expect(page).toHaveScreenshot(`privacy-top-${scheme}.png`, {
			clip: {
				x: 0,
				y: 0,
				width: page.viewportSize()?.width ?? 0,
				height: 600,
			},
		});
		expect(await seriousA11yViolations(page)).toEqual([]);
		expect(api.unexpected).toEqual([]);
	});
}

test(
	"deletes the account after typing XOÁ, then signs out",
	SMOKE,
	async ({ page, isMobile }) => {
		await preparePage(page, "light");
		if (isMobile) await stubNativeShell(page, SIGNED_IN_USER.email);
		const api = await stubApi(page);
		await page.goto("/record");
		await expect(page.getByTestId("usage")).toBeVisible();
		await accountTrigger(page).click();
		await accountMenuItem(page, "Xoá tài khoản").click();
		const dialog = page.getByRole("dialog", { name: "Xoá tài khoản Scribe" });
		await expect(dialog).toContainText(
			`Xoá vĩnh viễn tài khoản Scribe của ${SIGNED_IN_USER.email}?`,
		);
		await expect(
			dialog.getByRole("link", { name: "auth.hoandat.com/account" }),
		).toHaveAttribute("href", "https://auth.hoandat.com/account");
		const confirm = dialog.getByRole("button", { name: "Xoá tài khoản" });
		await expect(confirm).toBeDisabled();
		expect(
			await seriousA11yViolations(page, '[data-id="delete-account-dialog"]'),
		).toEqual([]);

		await dialog.getByLabel("Gõ XOÁ để xác nhận").fill("XOÁ");
		await confirm.click();
		await expect(page).toHaveURL(/\/login$/);
		await expect(page.getByTestId("login-account-deleted")).toContainText(
			"Đã xoá tài khoản Scribe",
		);
		expect(api.accountDeletes).toEqual(["XOÁ"]);
		expect(api.requests).toContain("POST /api/me/delete");
		// The web stays signed in at hoandat-auth: no logout call to Scribe.
		expect(api.requests).not.toContain("POST /api/auth/logout");
		if (isMobile) {
			const calls = await nativeCalls(page);
			expect(calls.map((call) => `${call.plugin}.${call.method}`)).toEqual(
				expect.arrayContaining(["Recorder.signOut", "HoandatAuth.signOut"]),
			);
			// The deleted account's segments and bookmarks leave the phone.
			const signOut = calls.find(
				(call) => call.plugin === "Recorder" && call.method === "signOut",
			);
			expect(signOut?.options).toEqual({
				discardUserKey: expect.stringMatching(/^[0-9a-f]{24}$/),
			});
		}
		expect(api.unexpected).toEqual([]);
	},
);
