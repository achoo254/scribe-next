import { afterEach, describe, expect, it } from "vitest";
import { setActiveTranslator } from "../../i18n/active-translator";
import { VIETNAMESE } from "../../i18n/i18n-context";
import { zhCN } from "../../i18n/messages/zh-CN/dictionary";
import { createTranslator } from "../../i18n/translate";
import { transcriptExportPath } from "./transcript-api";

afterEach(() => {
	setActiveTranslator(VIETNAMESE);
});

describe("transcript export path", () => {
	it("leaves the language out on a Vietnamese page", () => {
		expect(transcriptExportPath({ session: "abc" }, "md", "Họp")).toBe(
			"/api/transcripts/export?session=abc&format=md&name=H%E1%BB%8Dp",
		);
	});

	it("asks for headings in the language on screen and keeps the translation", () => {
		setActiveTranslator(createTranslator("zh-CN", zhCN));
		const original = new URL(
			transcriptExportPath({ fileIds: ["a", "b"] }, "docx", "周会"),
			"https://x",
		).searchParams;
		expect(original.get("ui")).toBe("zh-CN");
		expect(original.get("lang")).toBeNull();
		const translated = new URL(
			transcriptExportPath({ session: "abc" }, "md", "周会", "zh-CN"),
			"https://x",
		).searchParams;
		expect(translated.get("lang")).toBe("zh-CN");
	});
});
