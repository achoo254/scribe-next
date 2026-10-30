import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { activeTranslator } from "./active-translator";
import { useT } from "./i18n-context";
import { I18nProvider } from "./i18n-provider";

function CloseLabel() {
	const t = useT();
	return <span>{t("common.actions.close")}</span>;
}

/** The browser's language for one test; the setup's Vietnamese after it. */
function setDeviceLanguage(tag: string): void {
	Object.defineProperty(window.navigator, "language", {
		value: tag,
		configurable: true,
	});
}

afterEach(() => setDeviceLanguage("vi-VN"));

describe("I18nProvider", () => {
	it("gives components the Vietnamese texts", () => {
		render(
			<I18nProvider>
				<CloseLabel />
			</I18nProvider>,
		);
		expect(screen.getByText("Đóng")).toBeInTheDocument();
		expect(activeTranslator().t("ui.toast.region")).toBe("Thông báo");
	});

	it("speaks Chinese on a Chinese device with no stored choice", () => {
		setDeviceLanguage("zh-TW");
		render(
			<I18nProvider>
				<CloseLabel />
			</I18nProvider>,
		);
		expect(screen.getByText("关闭")).toBeInTheDocument();
		expect(document.documentElement.lang).toBe("zh-CN");
		expect(activeTranslator().formatNumber(10_000)).toBe("10,000");
	});

	it("keeps a stored choice over the device's language", () => {
		setDeviceLanguage("zh-CN");
		document.cookie = "hd_lang=en; Path=/";
		render(
			<I18nProvider>
				<CloseLabel />
			</I18nProvider>,
		);
		expect(screen.getByText("Close")).toBeInTheDocument();
	});

	it("falls back to Vietnamese outside the provider (single-component tests)", () => {
		render(<CloseLabel />);
		expect(screen.getByText("Đóng")).toBeInTheDocument();
	});
});
