import { render as renderNode, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactElement } from "react";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";
import { I18nProvider } from "../i18n/i18n-provider";
import { AppFooter } from "./app-footer";

function render(ui: ReactElement) {
	return renderNode(
		<I18nProvider>
			<MemoryRouter>{ui}</MemoryRouter>
		</I18nProvider>,
	);
}

describe("AppFooter", () => {
	it.each([
		["Tài khoản hoandat\\.com", "https://auth.hoandat.com/account"],
		["Ứng dụng", "https://auth.hoandat.com/account#account-apps"],
	])("links '%s' of the IdP in a new tab", (label, href) => {
		render(<AppFooter />);
		const link = screen.getByRole("link", {
			name: new RegExp(`^${label} ?, mở trang hoandat\\.com ở thẻ mới$`),
		});
		expect(link).toHaveAttribute("href", href);
		expect(link).toHaveAttribute("target", "_blank");
		expect(link).toHaveAttribute("rel", "noopener noreferrer");
	});

	it.each([
		["Quyền riêng tư", "/privacy"],
		["Điều khoản", "/terms"],
	])("links '%s' inside the app", (label, href) => {
		render(<AppFooter />);
		const legal = screen.getByRole("navigation", { name: "Văn bản pháp lý" });
		const link = within(legal).getByRole("link", { name: label });
		expect(link).toHaveAttribute("href", href);
		expect(link).not.toHaveAttribute("target");
	});

	it("links back to hoandat.com, like the public footer", () => {
		render(<AppFooter />);
		const link = screen.getByRole("link", { name: "Một app của hoandat.com" });
		expect(link).toHaveAttribute("href", "https://hoandat.com");
		expect(link).not.toHaveAttribute("target");
	});

	it("holds the theme switch", () => {
		render(<AppFooter />);
		const footer = screen.getByRole("contentinfo");
		const theme = within(footer).getByRole("group", { name: "Giao diện" });
		expect(within(theme).getByRole("button", { name: "Tối" })).toBeVisible();
	});

	it("holds the shared language switch next to the theme switch", async () => {
		render(<AppFooter />);
		const switches = screen
			.getByRole("contentinfo")
			.querySelector<HTMLElement>('[data-id="app-footer-switches"]');
		if (!switches) throw new Error("no footer switches");
		const language = within(switches).getByRole("group", { name: "Ngôn ngữ" });
		expect(
			within(language).getByRole("button", { name: "Tiếng Việt" }),
		).toHaveAttribute("aria-pressed", "true");
		expect(
			within(language)
				.getAllByRole("button")
				.map((button) => button.textContent),
		).toEqual(["Tiếng Việt", "English", "中文"]);
		expect(
			within(switches).getByRole("group", { name: "Giao diện" }),
		).toBeInTheDocument();

		await userEvent.click(
			within(language).getByRole("button", { name: "English" }),
		);
		expect(document.cookie).toContain("hd_lang=en");
		expect(document.documentElement.lang).toBe("en");
		expect(
			await screen.findByRole("link", { name: "An app from hoandat.com" }),
		).toHaveAttribute("href", "https://hoandat.com");
		expect(
			within(switches).getByRole("group", { name: "Language" }),
		).toBeInTheDocument();
		expect(
			within(switches).getByRole("group", { name: "Theme" }),
		).toBeVisible();
	});

	it("speaks the language chosen in another hoandat.com app", () => {
		document.cookie = "hd_lang=en; Path=/";
		render(<AppFooter />);
		expect(
			screen.getByRole("navigation", { name: "Legal" }),
		).toBeInTheDocument();
		expect(screen.getByRole("link", { name: "Privacy" })).toHaveAttribute(
			"href",
			"/privacy",
		);
	});

	it("speaks Chinese chosen here or in another hoandat.com app", async () => {
		document.cookie = "hd_lang=zh-CN; Path=/";
		render(<AppFooter />);
		expect(document.documentElement.lang).toBe("zh-CN");
		expect(screen.getByRole("button", { name: "中文" })).toHaveAttribute(
			"aria-pressed",
			"true",
		);
		expect(
			screen.getByRole("navigation", { name: "法律文件" }),
		).toBeInTheDocument();
		expect(screen.getByRole("link", { name: "隐私" })).toHaveAttribute(
			"href",
			"/privacy",
		);

		await userEvent.click(screen.getByRole("button", { name: "Tiếng Việt" }));
		expect(document.cookie).toContain("hd_lang=vi");
		expect(document.documentElement.lang).toBe("vi");
	});

	it("ignores a stored language Scribe cannot speak", () => {
		document.cookie = "hd_lang=fr; Path=/";
		render(<AppFooter />);
		expect(screen.getByRole("button", { name: "Tiếng Việt" })).toHaveAttribute(
			"aria-pressed",
			"true",
		);
	});
});
