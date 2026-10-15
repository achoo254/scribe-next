import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { THEME_STORAGE_KEY } from "../lib/theme";
import { ThemeToggle } from "./theme-toggle";

afterEach(() => {
	delete document.documentElement.dataset.theme;
	window.localStorage.clear();
});

describe("ThemeToggle", () => {
	it("starts on 'system' and switches data-theme on <html>", async () => {
		render(<ThemeToggle data-id="theme" />);
		expect(
			screen.getByRole("button", { name: "Theo hệ điều hành" }),
		).toHaveAttribute("aria-pressed", "true");

		await userEvent.click(
			screen.getByRole("button", { name: "Giao diện tối" }),
		);
		expect(document.documentElement.dataset.theme).toBe("dark");
		expect(window.localStorage.getItem(THEME_STORAGE_KEY)).toBe("dark");
		expect(
			screen.getByRole("button", { name: "Giao diện tối" }),
		).toHaveAttribute("aria-pressed", "true");

		await userEvent.click(
			screen.getByRole("button", { name: "Theo hệ điều hành" }),
		);
		expect(document.documentElement.dataset.theme).toBeUndefined();
	});
});
