import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Button, buttonClassName } from "./button";
import { IconButton } from "./icon-button";

describe("Button", () => {
	it("renders a non-submitting button with the requested variant and size", () => {
		render(
			<Button variant="primary" size="sm">
				Lưu
			</Button>,
		);
		const button = screen.getByRole("button", { name: "Lưu" });
		expect(button).toHaveAttribute("type", "button");
		expect(button).toHaveAttribute("data-variant", "primary");
		expect(button).toHaveAttribute("data-size", "sm");
		expect(button).toHaveClass(
			"bg-primary-solid",
			"text-on-primary",
			"rounded-sm",
		);
	});

	it.each([
		["secondary", "bg-surface"],
		["ghost", "bg-transparent"],
		["danger", "bg-error"],
	] as const)("applies %s variant classes", (variant, expected) => {
		expect(buttonClassName({ variant })).toContain(expected);
	});

	it("uses 12px radius (rounded-md) and press/hover scale at default size", () => {
		const classes = buttonClassName();
		expect(classes).toContain("rounded-md");
		expect(classes).toContain(
			"motion-safe:enabled:not-aria-disabled:hover:scale-103",
		);
		expect(classes).toContain(
			"motion-safe:enabled:not-aria-disabled:active:scale-97",
		);
	});

	it("calls onClick when enabled", async () => {
		const onClick = vi.fn();
		render(<Button onClick={onClick}>Gửi</Button>);
		await userEvent.click(screen.getByRole("button", { name: "Gửi" }));
		expect(onClick).toHaveBeenCalledTimes(1);
	});

	it("does not call onClick when disabled", async () => {
		const onClick = vi.fn();
		render(
			<Button disabled onClick={onClick}>
				Gửi
			</Button>,
		);
		const button = screen.getByRole("button", { name: "Gửi" });
		expect(button).toBeDisabled();
		await userEvent.click(button);
		expect(onClick).not.toHaveBeenCalled();
	});

	it("keeps type=submit when asked and merges custom classes", () => {
		render(
			<Button type="submit" className="mt-4" data-id="form-submit">
				OK
			</Button>,
		);
		const button = screen.getByRole("button", { name: "OK" });
		expect(button).toHaveAttribute("type", "submit");
		expect(button).toHaveClass("mt-4");
		expect(button).toHaveAttribute("data-id", "form-submit");
	});
});

describe("Button loading", () => {
	it("keeps the label for width and name, adds a spinner, swallows clicks and marks busy", async () => {
		const onClick = vi.fn();
		render(
			<Button
				loading
				variant="primary"
				leadingIcon={<svg data-id="lead" aria-hidden="true" />}
				onClick={onClick}
			>
				Create project
			</Button>,
		);
		const button = screen.getByRole("button", { name: "Create project" });
		expect(button).toBeEnabled();
		expect(button).toHaveAttribute("aria-disabled", "true");
		expect(button).toHaveAttribute("aria-busy", "true");
		expect(button).toHaveClass("aria-busy:cursor-progress");
		expect(button).toHaveClass("relative");
		const label = screen.getByText("Create project");
		expect(label).toHaveClass("opacity-0", "inline-flex", "gap-2");
		expect(label.querySelector('[data-id="lead"]')).not.toBeNull();
		const spinner = button.querySelector('[data-id="button-spinner"]');
		expect(spinner).toHaveAttribute("aria-hidden", "true");
		expect(spinner).toHaveClass("absolute", "inset-0");
		expect(spinner?.querySelector("svg")).toHaveClass(
			"motion-safe:animate-spin",
		);
		await userEvent.click(button);
		expect(onClick).not.toHaveBeenCalled();
	});

	it("renders as before when not loading", () => {
		render(<Button disabled={false}>Save</Button>);
		const button = screen.getByRole("button", { name: "Save" });
		expect(button).toBeEnabled();
		expect(button).not.toHaveAttribute("aria-busy");
		expect(button.querySelector('[data-id="button-spinner"]')).toBeNull();
	});
});

describe("IconButton", () => {
	it("swaps the icon for a spinner while loading", () => {
		render(
			<IconButton
				loading
				label="Refresh"
				icon={<svg data-id="icon" aria-hidden="true" />}
			/>,
		);
		const button = screen.getByRole("button", { name: "Refresh" });
		expect(button).toBeEnabled();
		expect(button).toHaveAttribute("aria-disabled", "true");
		expect(button).toHaveAttribute("aria-busy", "true");
		expect(button.querySelector('[data-id="icon"]')).toBeNull();
		expect(button.querySelector('[data-id="button-spinner"]')).not.toBeNull();
	});

	it("exposes its label as the accessible name", () => {
		render(<IconButton label="Đóng" icon={<svg aria-hidden="true" />} />);
		expect(screen.getByRole("button", { name: "Đóng" })).toHaveClass("size-9");
	});

	it("stays a 36px square: no text-button min-height or padding", () => {
		render(<IconButton label="Thêm" icon={<svg aria-hidden="true" />} />);
		const button = screen.getByRole("button", { name: "Thêm" });
		expect(button).toHaveClass(
			"size-9",
			"pointer-coarse:size-11",
			"rounded-md",
		);
		for (const cls of ["min-h-10", "px-4", "pointer-coarse:min-h-11"]) {
			expect(button).not.toHaveClass(cls);
		}
	});
});
