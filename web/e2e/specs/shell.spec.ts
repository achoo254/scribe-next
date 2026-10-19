import { type Page, expect, test } from "@playwright/test";
import { SIGNED_IN_USER, stubApi } from "../support/api-stubs";
import {
	IDLE_RECORDER,
	nativeCalls,
	stubNativeShell,
} from "../support/capacitor-stub";
import {
	COLOR_SCHEMES,
	fontsReady,
	preparePage,
	seriousA11yViolations,
} from "../support/page-helpers";

/*
 * The app shell: top bar, account menu and footer. Desktop runs the
 * browser UI; the iPhone project runs the iOS app's UI (Capacitor stub),
 * which is what iPhone users get.
 */

async function openApp(page: Page, native: boolean): Promise<void> {
	if (native) await stubNativeShell(page, SIGNED_IN_USER.email);
	await page.goto("/");
	await expect(page.getByTestId("history")).toContainText(
		"Họp nhóm sáng thứ Năm.webm",
	);
	await expect(page.getByTestId("usage")).toBeVisible();
	await fontsReady(page);
}

function accountTrigger(page: Page) {
	return page.getByRole("button", {
		name: `${SIGNED_IN_USER.email} · Tài khoản`,
	});
}

for (const scheme of COLOR_SCHEMES) {
	test.describe(`app shell in ${scheme}`, () => {
		test("top bar: wordmark, nav, account menu", async ({ page, isMobile }) => {
			await preparePage(page, scheme);
			const api = await stubApi(page);
			await openApp(page, isMobile);

			const topbar = page.getByTestId("topbar");
			await expect(
				topbar.getByRole("link", { name: "Scribe, về trang Ghi âm" }),
			).toBeVisible();
			const nav = topbar.getByRole("navigation", { name: "Chức năng" });
			await expect(nav.getByRole("link")).toHaveText([
				"Ghi âm",
				"Cắt file",
				"Lời đã chép",
			]);
			// The phone gets the compact row under the bar, not the wide nav.
			await expect(
				page.getByTestId(isMobile ? "topbar-nav" : "topbar-nav-compact"),
			).toBeHidden();
			await expect(accountTrigger(page)).toBeVisible();
			await expect(topbar).toHaveScreenshot(`topbar-${scheme}.png`);
			expect(await seriousA11yViolations(page)).toEqual([]);
			expect(api.unexpected).toEqual([]);
		});

		test("open account menu", async ({ page, isMobile }) => {
			await preparePage(page, scheme);
			const api = await stubApi(page);
			await openApp(page, isMobile);

			await accountTrigger(page).click();
			const menu = page.getByRole("menu");
			await expect(menu).toBeVisible();
			await expect(menu).toContainText(SIGNED_IN_USER.email);
			await expect(menu.getByRole("menuitem")).toHaveText([
				"Tài khoản hoandat, mở trang hoandat ở thẻ mới",
				"Ứng dụng, mở trang hoandat ở thẻ mới",
				"Key riêng",
				"Liên kết chat.hoandat.com",
				"Đăng xuất",
			]);
			await expect(page).toHaveScreenshot(`account-menu-${scheme}.png`);
			// The page under the menu is checked by the top bar test.
			expect(
				await seriousA11yViolations(page, '[data-id="account-menu"]'),
			).toEqual([]);
			expect(api.unexpected).toEqual([]);
		});

		test("footer: account links and theme", async ({ page, isMobile }) => {
			await preparePage(page, scheme);
			const api = await stubApi(page);
			await openApp(page, isMobile);

			const footer = page.getByTestId("app-footer");
			await footer.scrollIntoViewIfNeeded();
			await expect(
				footer.getByRole("link", { name: /^Tài khoản hoandat/ }),
			).toHaveAttribute("href", "https://auth.hoandat.com/account");
			await expect(
				footer.getByRole("link", { name: /^Ứng dụng/ }),
			).toHaveAttribute(
				"href",
				"https://auth.hoandat.com/account#account-apps",
			);
			// Phones get the one-button theme switch so the footer stays one row.
			if (isMobile) {
				await expect(
					footer.getByRole("button", { name: /— bấm để đổi$/ }),
				).toBeVisible();
				await expect(
					footer.getByRole("group", { name: "Giao diện" }),
				).toBeHidden();
			} else {
				await expect(
					footer.getByRole("group", { name: "Giao diện" }),
				).toBeVisible();
			}
			await expect(footer).toHaveScreenshot(`footer-${scheme}.png`);
			expect(await seriousA11yViolations(page)).toEqual([]);
			expect(api.unexpected).toEqual([]);
		});
	});
}

test("the nav opens each page at its URL", async ({ page, isMobile }) => {
	await preparePage(page, "light");
	await stubApi(page);
	await openApp(page, isMobile);
	await expect(page).toHaveURL(/\/record$/);
	const nav = page
		.getByTestId("topbar")
		.getByRole("navigation", { name: "Chức năng" });

	await nav.getByRole("link", { name: "Cắt file" }).click();
	await expect(page).toHaveURL(/\/split$/);
	await expect(nav.getByRole("link", { name: "Cắt file" })).toHaveAttribute(
		"aria-current",
		"page",
	);
	await expect(page.getByTestId("split")).toBeVisible();
	// The page left is unmounted; its state lives above the routes.
	await expect(page.getByTestId("record")).toHaveCount(0);

	await page.getByRole("link", { name: "Scribe, về trang Ghi âm" }).click();
	await expect(page).toHaveURL(/\/record$/);
	await expect(page.getByTestId("record")).toBeVisible();
});

test.describe("sign-out from the iOS app while recording", () => {
	test.skip(
		({ isMobile }) => !isMobile,
		"The iOS app shell only exists on the phone viewport",
	);

	test("asks first, then stops the recorder and signs out", async ({
		page,
	}) => {
		await preparePage(page, "light");
		await stubNativeShell(page, SIGNED_IN_USER.email, {
			...IDLE_RECORDER,
			state: "recording",
			elapsedS: 125,
			uploadsWaiting: 2,
		});
		const api = await stubApi(page);
		await page.goto("/");
		await expect(page.getByTestId("native-record-timer")).toBeVisible();

		await accountTrigger(page).click();
		await page.getByRole("menuitem", { name: "Đăng xuất" }).click();
		const dialog = page.getByRole("dialog", { name: "Đăng xuất" });
		await expect(dialog).toContainText(
			"Bản ghi đang chạy sẽ dừng; phần đã ghi vẫn được lưu.",
		);
		await expect(dialog).toContainText("2 đoạn chưa tải lên");
		expect(await seriousA11yViolations(page)).toEqual([]);

		await dialog.getByRole("button", { name: "Vẫn đăng xuất" }).click();
		await expect(page).toHaveURL(/\/login$/);
		const calls = (await nativeCalls(page)).map(
			(call) => `${call.plugin}.${call.method}`,
		);
		expect(calls).toEqual(
			expect.arrayContaining(["Recorder.signOut", "HoandatAuth.signOut"]),
		);
		expect(api.unexpected).toEqual([]);
	});
});
