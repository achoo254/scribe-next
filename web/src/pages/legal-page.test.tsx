import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { renderWithProviders } from "../test/render-with-providers";
import { LegalPage } from "./legal-page";

/*
 * The legal pages follow the language on screen. The Vietnamese text is the
 * one that applies: it ends with an English summary, and the English
 * translation opens by saying the Vietnamese version prevails.
 */
describe("LegalPage", () => {
	it("is Vietnamese with an English summary by default", async () => {
		renderWithProviders(<LegalPage doc="privacy" />);
		const article = await screen.findByRole("article");
		expect(
			within(article).getByRole("heading", {
				level: 1,
				name: "Chính sách quyền riêng tư",
			}),
		).toBeVisible();
		expect(
			within(article).getByRole("heading", { name: "English summary" }),
		).toBeVisible();
		expect(article.querySelector('[data-id="legal-authoritative"]')).toBeNull();
	});

	it("is translated in full on an English page, Vietnamese prevailing", async () => {
		document.cookie = "hd_lang=en; Path=/";
		renderWithProviders(<LegalPage doc="terms" />);
		const article = await screen.findByRole("article");
		expect(
			within(article).getByRole("heading", { level: 1, name: "Terms of Use" }),
		).toBeVisible();
		expect(
			within(article).getByRole("heading", { name: "About this translation" }),
		).toBeVisible();
		expect(article).toHaveTextContent(
			"The Vietnamese version is the one that applies",
		);
		expect(
			within(article).getByRole("heading", { name: "Governing law" }),
		).toBeVisible();
		expect(article).toHaveTextContent(
			"These terms are governed by the laws of Vietnam.",
		);
		expect(
			within(article).queryByRole("heading", { name: "English summary" }),
		).toBeNull();
		expect(
			within(article).getByRole("link", { name: "Privacy Policy" }),
		).toHaveAttribute("href", "/privacy");
		expect(document.documentElement.lang).toBe("en");
	});

	it("is translated in full on a Chinese page, Vietnamese prevailing", async () => {
		document.cookie = "hd_lang=zh-CN; Path=/";
		renderWithProviders(<LegalPage doc="privacy" />);
		const article = await screen.findByRole("article");
		expect(
			within(article).getByRole("heading", { level: 1, name: "隐私政策" }),
		).toBeVisible();
		expect(
			within(article).getByRole("heading", { name: "关于本译文" }),
		).toBeVisible();
		expect(article).toHaveTextContent("以越南语版本为准");
		expect(article).toHaveTextContent("自 2026年");
		expect(
			within(article).queryByRole("heading", { name: "英文摘要" }),
		).toBeNull();
		expect(document.documentElement.lang).toBe("zh-CN");
	});
});
