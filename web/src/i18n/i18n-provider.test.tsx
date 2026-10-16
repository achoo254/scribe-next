import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { activeTranslator } from "./active-translator";
import { useT } from "./i18n-context";
import { I18nProvider } from "./i18n-provider";

function CloseLabel() {
	const t = useT();
	return <span>{t("common.actions.close")}</span>;
}

describe("I18nProvider", () => {
	it("gives components the Vietnamese texts and marks the page language", () => {
		document.documentElement.lang = "";
		render(
			<I18nProvider>
				<CloseLabel />
			</I18nProvider>,
		);
		expect(screen.getByText("Đóng")).toBeInTheDocument();
		expect(document.documentElement.lang).toBe("vi");
		expect(activeTranslator().t("ui.toast.region")).toBe("Thông báo");
	});

	it("falls back to Vietnamese outside the provider (single-component tests)", () => {
		render(<CloseLabel />);
		expect(screen.getByText("Đóng")).toBeInTheDocument();
	});
});
