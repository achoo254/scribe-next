import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { AppFooter } from "./app-footer";

describe("AppFooter", () => {
	it.each([
		["Tài khoản hoandat", "https://auth.hoandat.com/account"],
		["Ứng dụng", "https://auth.hoandat.com/account#account-apps"],
	])("links '%s' of the IdP in a new tab", (label, href) => {
		render(<AppFooter />);
		const link = screen.getByRole("link", {
			name: new RegExp(`^${label} ?, mở trang hoandat ở thẻ mới$`),
		});
		expect(link).toHaveAttribute("href", href);
		expect(link).toHaveAttribute("target", "_blank");
		expect(link).toHaveAttribute("rel", "noopener noreferrer");
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
