import { render as renderNode, screen, within } from "@testing-library/react";
import type { ReactElement } from "react";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";
import { AppFooter } from "./app-footer";

function render(ui: ReactElement) {
	return renderNode(<MemoryRouter>{ui}</MemoryRouter>);
}

describe("AppFooter", () => {
	it.each([
		["Tài khoản Auth Next", "https://auth.hoandat.com/account"],
		["Ứng dụng", "https://auth.hoandat.com/account#account-apps"],
	])("links '%s' of the IdP in a new tab", (label, href) => {
		render(<AppFooter />);
		const link = screen.getByRole("link", {
			name: new RegExp(`^${label} ?, mở trang Auth Next ở thẻ mới$`),
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

	it("holds the theme switch", () => {
		render(<AppFooter />);
		const footer = screen.getByRole("contentinfo");
		const theme = within(footer).getByRole("group", { name: "Giao diện" });
		expect(
			within(theme).getByRole("button", { name: "Giao diện tối" }),
		).toBeVisible();
	});
});
