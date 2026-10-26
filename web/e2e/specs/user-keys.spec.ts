import { type Page, expect, test } from "@playwright/test";
import { REJECTED_KEY, SIGNED_IN_USER, stubApi } from "../support/api-stubs";
import { stubNativeShell } from "../support/capacitor-stub";
import {
	COLOR_SCHEMES,
	fontsReady,
	preparePage,
	seriousA11yViolations,
	smokeIn,
} from "../support/page-helpers";

/*
 * "Key riêng" from the account menu: add keys (checked by the stubbed
 * provider), see them only as "••••1234", pick which use runs on which,
 * delete. Desktop runs the browser UI, the iPhone project the iOS app's.
 */

async function openKeys(page: Page, native: boolean): Promise<void> {
	if (native) await stubNativeShell(page, SIGNED_IN_USER.email);
	await page.goto("/");
	await expect(page.getByTestId("usage")).toBeVisible();
	await fontsReady(page);
	await page
		.getByRole("button", { name: `${SIGNED_IN_USER.email} · Tài khoản` })
		.click();
	await page.getByRole("menuitem", { name: "Key riêng" }).click();
	await expect(page.getByRole("dialog", { name: "Key riêng" })).toBeVisible();
}

for (const scheme of COLOR_SCHEMES) {
	test.describe(`Key riêng in ${scheme}`, smokeIn(scheme), () => {
		test("add keys, pick their uses, delete", async ({ page, isMobile }) => {
			await preparePage(page, scheme);
			const api = await stubApi(page);
			await openKeys(page, isMobile);
			const dialog = page.getByRole("dialog", { name: "Key riêng" });
			const elevenlabs = dialog.getByRole("region", { name: "ElevenLabs" });
			const openai = dialog.getByRole("region", { name: "OpenAI" });
			await expect(elevenlabs).toContainText("Chưa có key");
			await expect(
				dialog.getByRole("group", { name: "Chép lời tệp" }),
			).toContainText("Đang dùng key của Scribe");
			await expect(page).toHaveScreenshot(`user-keys-empty-${scheme}.png`);

			const elevenField = elevenlabs.getByLabel("Key ElevenLabs");
			await elevenField.fill("sk_test_fake_e2e_key_wxyz");
			await elevenlabs.getByRole("button", { name: "Lưu và kiểm" }).click();
			await expect(elevenlabs).toContainText("••••wxyz · đã kiểm 16/10");
			await expect(elevenField).toHaveValue("");

			const openaiField = openai.getByLabel("Key OpenAI");
			await openaiField.fill(REJECTED_KEY);
			await openai.getByRole("button", { name: "Lưu và kiểm" }).click();
			await expect(openai.getByRole("alert")).toContainText(
				"OpenAI từ chối key này",
			);
			await openaiField.fill("sk-test-fake-e2e-key-1234");
			await openai.getByRole("button", { name: "Lưu và kiểm" }).click();
			await expect(openai).toContainText("••••1234 · đã kiểm 16/10");

			const transcribe = dialog.getByRole("group", { name: "Chép lời tệp" });
			await expect(
				transcribe.getByRole("radio", { name: "ElevenLabs" }),
			).toBeChecked();
			await expect(
				transcribe.getByRole("radio", { name: "Google Gemini (chưa có key)" }),
			).toBeDisabled();
			const text = dialog.getByRole("group", {
				name: "Biên bản, hỏi đáp, dịch",
			});
			await expect(text.getByRole("radio", { name: "OpenAI" })).toBeChecked();
			await transcribe.getByRole("radio", { name: "OpenAI" }).click();
			await expect(
				transcribe.getByRole("radio", { name: "OpenAI" }),
			).toBeChecked();
			await text
				.getByRole("combobox", { name: "Model" })
				.selectOption("gpt-4.1-nano");
			await expect(text.getByRole("combobox", { name: "Model" })).toHaveValue(
				"gpt-4.1-nano",
			);
			// Toasts of the saves would cover the dialog: wait until they expire.
			await expect(
				page.locator("li", { hasText: "Đã kiểm và lưu key" }),
			).toHaveCount(0, { timeout: 10_000 });
			// Scroll the dialog body (only it) to the uses section.
			await dialog.getByTestId("user-keys-uses").evaluate((section) => {
				let box = section.parentElement;
				while (box && getComputedStyle(box).overflowY !== "auto") {
					box = box.parentElement;
				}
				if (box) box.scrollTop = box.scrollHeight;
			});
			await expect(page).toHaveScreenshot(`user-keys-saved-${scheme}.png`);
			expect(
				await seriousA11yViolations(page, '[data-id="user-keys-dialog"]'),
			).toEqual([]);

			await elevenlabs.getByRole("button", { name: "Xoá key" }).click();
			await expect(elevenlabs).toContainText("Chưa có key");
			await dialog.getByRole("button", { name: "Đóng" }).first().click();
			await expect(dialog).toBeHidden();
			const usage = page.getByTestId("usage");
			await expect(usage).toContainText("Dùng key của bạn (OpenAI)");
			expect(api.keySaves).toEqual(["elevenlabs", "openai", "openai"]);
			expect(api.unexpected).toEqual([]);
		});
	});
}
